// src/ui.rs

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
};
use textwrap::wrap;

// Nous avons besoin de l'App struct pour accéder à l'état de l'application
use crate::{ActivePanel, App};
use chrono::Local;
use throbber_widgets_tui::Throbber;

/// Dessine l'interface utilisateur de l'application.
/// Prend un Frame de Ratatui et une référence mutable à l'état de l'application.
pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[0]);

    // Left Panel (Table)
    draw_left_panel(f, app, main_chunks[0]);

    // Right Panel (Gemini Response)
    draw_right_panel(f, app, main_chunks[1]);

    // Bottom Status Panel (for spinner and time)
    draw_bottom_panel(f, app, chunks[1]);
}

/// Dessine le panneau inférieur pour le spinner et le temps.
fn draw_bottom_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let time_str = Local::now().format("%H:%M:%S").to_string();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Min(0),
                Constraint::Length(time_str.len() as u16),
            ]
            .as_ref(),
        )
        .split(area);

    // Left sub-panel: spinner with throbber and label styled separately
    if app.spinner_enabled {
        let spinner = Throbber::default()
            .throbber_style(Style::default().fg(Color::Magenta))
            .label("Chargement")
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(spinner, chunks[0]);
    }

    // Right sub-panel with current time at bottom-right
    let time_paragraph = Paragraph::new(time_str).alignment(Alignment::Right);
    f.render_widget(time_paragraph, chunks[1]);
}

fn draw_left_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let table_title = "Mock Bugs (↑↓ pour naviguer)".to_string();
    let header_cells = ["Bug ID", "Date", "Title"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells)
        .style(Style::default())
        .height(1)
        .bottom_margin(1);

    let rows = app.table_items.iter().map(|item| {
        let height = 1;
        let cells = vec![
            Cell::from(item.bug_id.to_string()),
            Cell::from(item.date.clone()),
            Cell::from(item.title.clone()),
        ];
        Row::new(cells).height(height as u16).bottom_margin(1)
    });

    let widths = &[
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(70),
    ];
    let table_border_style = if let ActivePanel::Left = app.active_panel {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };

    let table_widget = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(table_title)
                .border_style(table_border_style),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::LightCyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(table_widget, area, &mut app.table_state);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        scrollbar,
        area.inner(Default::default()),
        &mut app.scrollbar_state,
    );
}

fn draw_right_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let current_display_text = app.gemini_response.lock().unwrap().clone();
    let editor_instruction = " (Appuyez sur 'e' pour éditer, 'a' pour une generation ai)";

    let mut gemini_title = if let Some(index) = app.selected_bug_index {
        if let Some(bug) = app.table_items.get(index) {
            let truncated_title = if bug.title.chars().count() > 40 {
                format!("{}...", bug.title.chars().take(40).collect::<String>())
            } else {
                bug.title.clone()
            };
            format!("{}-{}", bug.bug_id, truncated_title)
        } else {
            "Réponse de Gemini".to_string()
        }
    } else {
        "Réponse de Gemini".to_string()
    };

    if !current_display_text.starts_with("Chargement") {
        gemini_title.push_str(editor_instruction);
    }

    let right_panel_border_style = if let ActivePanel::Right = app.active_panel {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };

    let wrapped_text = wrap(&current_display_text, (area.width - 2) as usize);
    let content_length = wrapped_text.len();
    let viewport_height = (area.height.saturating_sub(2)) as usize;

    if app.scroll_to_end {
        app.right_panel_scroll = content_length.saturating_sub(viewport_height) as u16;
        app.scroll_to_end = false;
    }

    let max_scroll = content_length.saturating_sub(viewport_height) as u16;
    app.right_panel_scroll = app.right_panel_scroll.min(max_scroll);

    let gemini_paragraph = Paragraph::new(current_display_text.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(gemini_title)
                .border_style(right_panel_border_style),
        )
        .wrap(ratatui::widgets::Wrap { trim: true })
        .scroll((app.right_panel_scroll, 0));

    f.render_widget(gemini_paragraph, area);

    let mut right_scrollbar_state =
        ScrollbarState::new(content_length).position(app.right_panel_scroll as usize);

    let right_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        right_scrollbar,
        area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 1,
        }),
        &mut right_scrollbar_state,
    );
}
