// src/lib.rs

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{
    env,
    io::{Read, Write, stdout},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::NamedTempFile;
use tokio::time::sleep;

// Importe les modules que nous allons créer
mod ai;
mod ui; // Déclare le module ui // Déclare le module ai

use ai::get_gemini_response_static;
use ui::draw_ui; // Importe la fonction draw_ui du module ui // Importe la fonction get_gemini_response_static du module ai

/// Représente l'état de l'application TUI.
pub struct App {
    pub list_items: Vec<String>,
    pub list_state: ratatui::widgets::ListState,
    pub gemini_response: Arc<Mutex<String>>,
}

impl App {
    /// Crée une nouvelle instance de l'application avec l'état initial.
    pub fn new() -> App {
        let items = vec![
            String::from("Qu'est-ce que Rust ?"),
            String::from("Explique React Hooks."),
            String::from("C'est quoi un blockchain ?"),
            String::from("Comment fonctionne Git ?"),
            String::from("Donne une recette de lasagnes."),
        ];
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(0)); // Sélectionne le premier élément par défaut

        App {
            list_items: items,
            list_state,
            gemini_response: Arc::new(Mutex::new(
                "Chargement de la réponse de Gemini...".to_string(),
            )),
        }
    }

    /// Déplace la sélection vers le haut dans la liste.
    pub fn previous_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.list_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Déplace la sélection vers le bas dans la liste.
    pub fn next_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.list_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

/// Fonction principale de l'application TUI.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    // La clé API n'est plus strictement nécessaire pour le moment, mais on la garde pour plus tard.
    let _api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY not set in .env file or environment variables");

    // Initialisation du terminal Crossterm et Ratatui
    enable_raw_mode()?;
    execute!(stdout(), Clear(ClearType::All))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;

    // Crée une nouvelle instance de notre application
    let mut app = App::new();
    let gemini_response_text_for_spawn = Arc::clone(&app.gemini_response);

    // Lance la tâche asynchrone pour la "réponse Gemini" (statique pour l'instant)
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

    // Boucle principale de l'application
    loop {
        // Dessine l'interface utilisateur en passant la référence à l'objet app
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // Gère les événements d'entrée
        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,        // Quitter l'application
                        KeyCode::Up => app.previous_item(), // Déplacer la sélection vers le haut
                        KeyCode::Down => app.next_item(),   // Déplacer la sélection vers le bas
                        KeyCode::Char('e') => {
                            // Éditer le contenu
                            let content_to_edit = app.gemini_response.lock().unwrap().clone();

                            let mut temp_file = NamedTempFile::new()?;
                            temp_file.write_all(content_to_edit.as_bytes())?;
                            let file_path = temp_file.path().to_path_buf();

                            // 1. Quitter le mode Ratatui
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

                            // 3. Réactiver le mode Ratatui
                            enable_raw_mode()?;
                            execute!(stdout(), Clear(ClearType::All), EnterAlternateScreen)?;

                            // Lire le contenu mis à jour du fichier temporaire
                            let mut updated_content = String::new();
                            std::fs::File::open(&file_path)?
                                .read_to_string(&mut updated_content)?;
                            {
                                // Met à jour l'état de l'application avec le nouveau contenu
                                let mut response_guard = app.gemini_response.lock().unwrap();
                                *response_guard = updated_content;
                            }

                            // Forcer un nettoyage et un redessin complet de la TUI
                            terminal.clear()?;
                            terminal.draw(|f| draw_ui(f, &mut app))?; // Redessine avec le nouveau contenu
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

    // Nettoyage final avant de quitter
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;

    Ok(())
}
