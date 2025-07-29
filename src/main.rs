use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};
use std::{
    io::stdout,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

use dotenv::dotenv;
use google_ai_rs::{Client, GenerativeModel, genai::Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY not set in .env file or environment variables");

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let client = Arc::new(Client::new(api_key.into()).await?);

    let gemini_response_text_arc = Arc::new(Mutex::new(
        "Chargement de la réponse de Gemini...".to_string(),
    ));
    let gemini_response_text_for_spawn = Arc::clone(&gemini_response_text_arc);
    let client_for_spawn = Arc::clone(&client);

    tokio::spawn(async move {
        let model = GenerativeModel::new(&client_for_spawn, "gemini-2.5-flash");

        match get_gemini_response(model).await {
            Ok(response) => {
                let text_content = response.text();

                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                if !text_content.is_empty() {
                    *response_guard = text_content;
                } else {
                    *response_guard =
                        "Réponse de Gemini vide. Vérifiez le prompt ou la configuration du modèle."
                            .to_string();
                }
            }
            Err(e) => {
                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                *response_guard = format!("Erreur API Gemini : {e}");
            }
        }
    });

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.area());

            let current_text = gemini_response_text_arc.lock().unwrap().clone();
            let paragraph = Paragraph::new(current_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Réponse de Gemini"),
                )
                .wrap(ratatui::widgets::Wrap { trim: false });

            f.render_widget(paragraph, chunks[0]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn get_gemini_response<'a>(
    model: GenerativeModel<'a>,
) -> Result<Response, Box<dyn std::error::Error>> {
    let response = model
        .generate_content("Qu'est-ce que Rust et pourquoi est-il populaire ?")
        .await?;
    Ok(response)
}
