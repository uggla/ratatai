// src/ui.rs

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

// Nous avons besoin de l'App struct pour accéder à l'état de l'application
use crate::App;

/// Dessine l'interface utilisateur de l'application.
/// Prend un Frame de Ratatui et une référence mutable à l'état de l'application.
pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(f.area());

    let list_title = "Suggestions de Prompts (↑↓ pour naviguer)".to_string();
    let list_items: Vec<ListItem> = app
        .list_items // Utilise les items de l'application
        .iter()
        .map(|i| ListItem::new(i.as_str()))
        .collect();

    let list_widget = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .fg(ratatui::style::Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list_widget, main_chunks[0], &mut app.list_state); // Utilise list_state de l'application

    let current_display_text = app.gemini_response.lock().unwrap().clone(); // Accède à la réponse Gemini via l'application
    let editor_instruction = " (Appuyez sur 'e' pour éditer)";
    let mut gemini_title = "Réponse de Gemini".to_string();
    if !current_display_text.starts_with("Chargement") {
        gemini_title.push_str(editor_instruction);
    }
    let gemini_paragraph = Paragraph::new(current_display_text)
        .block(Block::default().borders(Borders::ALL).title(gemini_title))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(gemini_paragraph, main_chunks[1]);
}
