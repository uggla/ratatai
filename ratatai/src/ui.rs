// src/ui.rs

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
};
use regex::Regex;
use textwrap::wrap;

// We need the App struct to access the application state
use chrono::Local;
use throbber_widgets_tui::Throbber;

use crate::app::{ActivePanel, App, Screen};

/// Playful labels for the spinner, cycled with each 's' key press
pub const SPINNER_LABELS: [&str; 5] = [
    "Loading...",
    "Patience, young padawan...",
    "Don't blink",
    "Tinkering...",
    "Coffee time",
];

/// Draws the application's user interface.
/// Takes a Ratatui Frame and a mutable reference to the application state.
pub fn draw_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.area());

    match app.current_screen {
        Screen::BugList => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            // Left Panel (Table)
            draw_bug_list(f, app, main_chunks[0]);

            // Right Panel (Gemini Response)
            // draw_bug_description(f, app, main_chunks[1]);
        }
        Screen::BugEditing => {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            // draw_bug_description(f, app, main_chunks[0]);
            draw_bug_reply(f, app, main_chunks[1]);
        }
    }

    // Bottom Status Panel (for spinner and time)
    draw_bottom_panel(f, app, chunks[1]);
}

/// Draws the bottom panel for the spinner and time.
fn draw_bottom_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let time_str = Local::now().format("%H:%M:%S").to_string();
    let spinner_label_width = SPINNER_LABELS[app.spinner_label_index].len() as u16 + 2; // +2 for throbber
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(spinner_label_width),
                Constraint::Min(0),
                Constraint::Length(time_str.len() as u16),
            ]
            .as_ref(),
        )
        .split(area);

    // Left sub-panel: spinner with throbber and label styled separately
    if app.spinner_enabled {
        app.spinner_state.calc_next();
    }
    let spinner = Throbber::default()
        .throbber_style(Style::default().fg(Color::Magenta))
        .label(SPINNER_LABELS[app.spinner_label_index])
        .style(Style::default().fg(Color::Cyan));
    f.render_stateful_widget(spinner, chunks[0], &mut app.spinner_state);

    // Middle sub-panel: Command input
    let command_text = match app.current_screen {
        Screen::BugList => match app.active_panel {
            ActivePanel::Left => "Tab selection, ↑↓ PgUp/PgDown Home/End to navigate",
            ActivePanel::Right => {
                "Tab selection, ↑↓ PgUp/PgDown Home/End to navigate, 'e' to edit, 'a' for AI generation, 'r' to reply to this bug"
            }
        },
        Screen::BugEditing => match app.active_panel {
            ActivePanel::Left => {
                "Tab selection, ↑↓ PgUp/PgDown Home/End to navigate, 'e' to edit, 'a' for AI generation, 'r' to reply to this bug"
            }
            ActivePanel::Right => "tbd",
        },
    };
    let command_paragraph = Paragraph::new(command_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(command_paragraph, chunks[1]);

    // Right sub-panel with current time at bottom-right
    let time_paragraph = Paragraph::new(time_str).alignment(Alignment::Right);
    f.render_widget(time_paragraph, chunks[2]);
}

fn draw_bug_list(f: &mut Frame, app: &mut App, area: Rect) {
    let table_title = format!(
        "Bugs in status 'New' {}/{}",
        match app.bug_table_state.selected() {
            None => "-".to_string(),
            Some(n) => (n + 1).to_string(),
        },
        app.bug_table_items.len()
    );
    let header_cells = ["Bug ID", "Date", "Title"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells).style(Style::default()).height(1);

    let widths = &[
        Constraint::Length(9),
        Constraint::Length(12),
        Constraint::Percentage(100),
    ];
    let table_border_style = if let ActivePanel::Left = app.active_panel {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };

    let table_widget = Table::new(app.bug_table_rows.iter().cloned(), widths)
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

    f.render_stateful_widget(table_widget, area, &mut app.bug_table_state);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        scrollbar,
        area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut app.bug_table_scrollbar_state,
    );
}

// fn draw_bug_description(f: &mut Frame, app: &mut App, area: Rect) {
//     let current_display_text = app.gemini_response.lock().unwrap().clone();
//
//     let gemini_title = if let Some(index) = app.bug_table_selected_index {
//         if let Some(bug) = app.bug_table_items.get(index) {
//             let truncated_title = if bug.title.chars().count() > 40 {
//                 format!("{}...", bug.title.chars().take(40).collect::<String>())
//             } else {
//                 bug.title.clone()
//             };
//             format!("{}-{}", bug.bug_id, truncated_title)
//         } else {
//             "Gemini Response".to_string()
//         }
//     } else {
//         "Gemini Response".to_string()
//     };
//
//     let right_panel_border_style = match app.current_screen {
//         Screen::BugList => match app.active_panel {
//             ActivePanel::Right => Style::default().fg(Color::Green),
//             _ => Style::default().fg(Color::White),
//         },
//         Screen::BugEditing => match app.active_panel {
//             ActivePanel::Left => Style::default().fg(Color::Green),
//             _ => Style::default().fg(Color::White),
//         },
//     };
//
//     let scrollbar_area = area.inner(Margin {
//         vertical: 1,
//         horizontal: 1,
//     });
//     let scrollbar_height = scrollbar_area.height as usize;
//
//     let wrapped_text = wrap(&current_display_text, (scrollbar_area.width) as usize);
//     let wrapped_text: Vec<Line> = wrapped_text
//         .iter()
//         .map(|line| Line::from(line.to_string()))
//         .collect();
//
//     let content_length = wrapped_text.len();
//
//     if app.bug_desc_scroll_to_end {
//         app.bug_desc_scroll = content_length.saturating_sub(scrollbar_height) as u16;
//         app.bug_desc_scroll_to_end = false;
//     }
//
//     let max_scroll = content_length.saturating_sub(scrollbar_height) as u16;
//     app.bug_desc_scroll = app.bug_desc_scroll.min(max_scroll);
//
//     let gemini_paragraph = Paragraph::new(wrapped_text)
//         .block(
//             Block::default()
//                 .borders(Borders::ALL)
//                 .title(gemini_title)
//                 .border_style(right_panel_border_style),
//         )
//         .scroll((app.bug_desc_scroll, 0));
//
//     f.render_widget(gemini_paragraph, area);
//
//     let mut bug_desc_scrollbar_state = ScrollbarState::new(content_length)
//         .viewport_content_length(scrollbar_height)
//         .position(app.bug_desc_scroll as usize);
//
//     let bug_table_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
//         .begin_symbol(Some("↑"))
//         .end_symbol(Some("↓"));
//
//     f.render_stateful_widget(
//         bug_table_scrollbar,
//         scrollbar_area,
//         &mut bug_desc_scrollbar_state,
//     );
// }

fn draw_bug_reply(f: &mut Frame, app: &mut App, area: Rect) {
    let lorem_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

    let lorem_paragraph = Paragraph::new(lorem_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Lorem Ipsum")
                .border_style(match app.active_panel {
                    ActivePanel::Right => Style::default().fg(Color::Green),
                    _ => Style::default().fg(Color::White),
                }),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(lorem_paragraph, area);
}
