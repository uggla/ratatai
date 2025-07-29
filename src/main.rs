use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{
    env,
    io::{Read, Write, stdout},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::NamedTempFile;
use tokio::time::sleep;

use dotenv::dotenv;
// Commenter ces lignes si tu ne veux pas compiler avec la dépendance google_ai_rs pour le moment.
// use google_ai_rs::{Client, GenerativeModel, genai::Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let _api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY not set in .env file or environment variables");

    enable_raw_mode()?;
    execute!(stdout(), Clear(ClearType::All))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;

    let gemini_response_text_arc = Arc::new(Mutex::new(
        "Chargement de la réponse de Gemini...".to_string(),
    ));
    let gemini_response_text_for_spawn = Arc::clone(&gemini_response_text_arc);

    let items = &[
        String::from("Qu'est-ce que Rust ?"),
        String::from("Explique React Hooks."),
        String::from("C'est quoi un blockchain ?"),
        String::from("Comment fonctionne Git ?"),
        String::from("Donne une recette de lasagnes."),
    ];
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    tokio::spawn(async move {
        match get_gemini_response_static().await {
            Ok(response) => {
                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                *response_guard = response;
            }
            Err(e) => {
                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                *response_guard = format!("Erreur lors de la récupération de la réponse: {e}");
            }
        }
    });

    loop {
        terminal.draw(|f| {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                .split(f.area());

            let list_title = "Suggestions de Prompts (↑↓ pour naviguer)".to_string();
            let list_items: Vec<ListItem> =
                items.iter().map(|i| ListItem::new(i.as_str())).collect();

            let list_widget = List::new(list_items)
                .block(Block::default().borders(Borders::ALL).title(list_title))
                .highlight_style(
                    Style::default()
                        .fg(ratatui::style::Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            f.render_stateful_widget(list_widget, main_chunks[0], &mut list_state);

            let current_display_text = gemini_response_text_arc.lock().unwrap().clone();
            let editor_instruction = " (Appuyez sur 'e' pour éditer)";
            let mut gemini_title = "Réponse de Gemini".to_string();
            if !current_display_text.starts_with("Chargement") {
                gemini_title.push_str(editor_instruction);
            }
            let gemini_paragraph = Paragraph::new(current_display_text)
                .block(Block::default().borders(Borders::ALL).title(gemini_title))
                .wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(gemini_paragraph, main_chunks[1]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Up => {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        items.len() - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        KeyCode::Down => {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i >= items.len() - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        KeyCode::Char('e') => {
                            let content_to_edit = gemini_response_text_arc.lock().unwrap().clone();

                            let mut temp_file = NamedTempFile::new()?;
                            temp_file.write_all(content_to_edit.as_bytes())?;
                            let file_path = temp_file.path().to_path_buf();

                            // 1. Quitter le mode Ratatui :
                            terminal.show_cursor()?;
                            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                            disable_raw_mode()?;

                            // 2. Lancer l'éditeur externe
                            let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                            let status = Command::new(&editor).arg(&file_path).status()?;

                            if !status.success() {
                                eprintln!(
                                    "L'éditeur a quitté avec une erreur : {:?}",
                                    status.code()
                                );
                            }

                            // 3. Réactiver le mode Ratatui :
                            enable_raw_mode()?;
                            execute!(stdout(), Clear(ClearType::All), EnterAlternateScreen)?;

                            // Lire le contenu mis à jour du fichier temporaire
                            // --- NOUVEAU CODE ICI ---
                            let mut updated_content = String::new();
                            std::fs::File::open(&file_path)?
                                .read_to_string(&mut updated_content)?;
                            {
                                let mut response_guard = gemini_response_text_arc.lock().unwrap();
                                *response_guard = updated_content;
                            }
                            // --- FIN NOUVEAU CODE ---

                            terminal.clear()?;
                            terminal.draw(|f| {
                                let main_chunks = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .margin(1)
                                    .constraints(
                                        [Constraint::Percentage(30), Constraint::Percentage(70)]
                                            .as_ref(),
                                    )
                                    .split(f.area());

                                let list_title =
                                    "Suggestions de Prompts (↑↓ pour naviguer)".to_string();
                                let list_items: Vec<ListItem> =
                                    items.iter().map(|i| ListItem::new(i.as_str())).collect();

                                let list_widget = List::new(list_items)
                                    .block(Block::default().borders(Borders::ALL).title(list_title))
                                    .highlight_style(
                                        Style::default()
                                            .fg(ratatui::style::Color::LightCyan)
                                            .add_modifier(Modifier::BOLD),
                                    )
                                    .highlight_symbol(">> ");

                                f.render_stateful_widget(
                                    list_widget,
                                    main_chunks[0],
                                    &mut list_state,
                                );

                                // Utilise directement la valeur mise à jour de gemini_response_text_arc
                                let current_display_text =
                                    gemini_response_text_arc.lock().unwrap().clone();
                                let editor_instruction = " (Appuyez sur 'e' pour éditer)";
                                let mut gemini_title = "Réponse de Gemini".to_string();
                                if !current_display_text.starts_with("Chargement") {
                                    gemini_title.push_str(editor_instruction);
                                }
                                let gemini_paragraph = Paragraph::new(current_display_text)
                                    .block(
                                        Block::default().borders(Borders::ALL).title(gemini_title),
                                    )
                                    .wrap(ratatui::widgets::Wrap { trim: false });
                                f.render_widget(gemini_paragraph, main_chunks[1]);
                            })?;
                            terminal.backend_mut().flush()?;
                            terminal.hide_cursor()?;
                        }
                        _ => {}
                    }
                }
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;

    Ok(())
}

// async fn get_gemini_response<'a>(
//     model: GenerativeModel<'a>,
// ) -> Result<Response, Box<dyn std::error::Error>> {
//     let response = model
//         .generate_content("Qu'est-ce que Rust et pourquoi est-il populaire ?")
//         .await?;
//     Ok(response)
// }

async fn get_gemini_response_static() -> Result<String, Box<dyn std::error::Error>> {
    sleep(Duration::from_secs(2)).await;
    Ok("Ceci est une réponse statique de Gemini pour l'instant. L'appel réel à l'API est commenté pour faciliter le développement de l'interface utilisateur. Vous pouvez éditer ce texte si vous le souhaitez.".to_string())
}
