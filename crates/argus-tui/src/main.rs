use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::{Context, Result};
use argus_core::session::{InputControllerKind, SessionSize};
use argus_tui::{LocalSessionApp, SessionView, session_size_from_terminal};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

fn main() -> Result<()> {
    let size = initial_session_size()?;
    let mut app = LocalSessionApp::start(size).context("starting local session")?;
    let mut terminal = match TerminalSession::enter().context("starting terminal UI") {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = app.shutdown();
            return Err(error);
        }
    };

    let result = run(&mut terminal.terminal, &mut app);
    let shutdown_result = app.shutdown();
    terminal.restore()?;

    result?;
    shutdown_result.context("shutting down local session")?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut LocalSessionApp) -> Result<()> {
    loop {
        app.drain_events()?;
        terminal.draw(|frame| draw(frame, app.view(), app.last_error()))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if should_exit(key) => return Ok(()),
            Event::Key(key) => {
                if let Some(bytes) = key_to_input(key) {
                    app.write_input(bytes);
                }
            }
            Event::Resize(cols, rows) => {
                app.resize(session_size_from_app_terminal(rows, cols));
            }
            _ => {}
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, view: &SessionView, last_error: Option<&str>) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(root[0]);

    draw_sidebar(frame, body[0], view);
    draw_terminal(frame, body[1], view);
    draw_status(frame, root[1], view, last_error);
}

fn draw_sidebar(frame: &mut ratatui::Frame<'_>, area: Rect, view: &SessionView) {
    let holder = view
        .lease
        .holder
        .as_ref()
        .map(|holder| match holder.kind {
            InputControllerKind::Interactive => "interactive",
            InputControllerKind::Agent => "agent",
        })
        .unwrap_or("observer");
    let state = if view.snapshot.exited {
        "exited"
    } else {
        "running"
    };
    let rows = vec![
        Line::from(Span::styled(
            view.session_id.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("state  {state}")),
        Line::from(format!("lease  {holder}")),
        Line::from(format!(
            "size   {}x{}",
            view.snapshot.size.cols, view.snapshot.size.rows
        )),
    ];

    frame.render_widget(
        Paragraph::new(rows)
            .block(Block::default().title("Sessions").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_terminal(frame: &mut ratatui::Frame<'_>, area: Rect, view: &SessionView) {
    let visible_height = usize::from(area.height.saturating_sub(2));
    let start = view
        .snapshot
        .visible_rows
        .len()
        .saturating_sub(visible_height);
    let rows = view.snapshot.visible_rows[start..]
        .iter()
        .map(|row| {
            Line::from(Span::styled(
                row.as_str(),
                Style::default().fg(Color::White),
            ))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(rows)
            .block(Block::default().title("Terminal").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &SessionView,
    last_error: Option<&str>,
) {
    let status = if let Some(error) = last_error {
        Line::from(vec![
            Span::styled("error ", Style::default().fg(Color::Red)),
            Span::raw(error.to_string()),
        ])
    } else {
        Line::from(format!(
            "seq {}  bytes {}  in {}  out {}  Esc/Ctrl-Q exits",
            view.snapshot.output_seq,
            view.snapshot.bytes_logged,
            view.input_bytes_sent,
            view.output_events_seen
        ))
    };
    frame.render_widget(Paragraph::new(status), area);
}

fn should_exit(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')))
}

fn key_to_input(key: KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            control_byte(character).map(|byte| vec![byte])
        }
        KeyCode::Char(character) => Some(character.to_string().into_bytes()),
        KeyCode::Enter => Some(b"\r".to_vec()),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(b"\t".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        _ => None,
    }
}

fn control_byte(character: char) -> Option<u8> {
    let upper = character.to_ascii_uppercase();
    if upper.is_ascii_alphabetic() {
        Some((upper as u8) - b'A' + 1)
    } else {
        None
    }
}

fn initial_session_size() -> Result<SessionSize> {
    let (cols, rows) = crossterm::terminal::size()?;
    Ok(session_size_from_app_terminal(rows, cols))
}

fn session_size_from_app_terminal(rows: u16, cols: u16) -> SessionSize {
    session_size_from_terminal(rows.saturating_sub(3), cols.saturating_sub(30))
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    restored: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        Ok(Self {
            terminal,
            restored: false,
        })
    }

    fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_keys_are_forwarded_to_session() {
        assert_eq!(
            key_to_input(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(b"q".to_vec())
        );
        assert_eq!(
            key_to_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn local_exit_uses_escape_or_control_q() {
        assert!(should_exit(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(should_exit(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
        )));
        assert!(!should_exit(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn session_size_matches_terminal_pane_inner_area() {
        let size = session_size_from_app_terminal(24, 100);

        assert_eq!(size.rows, 21);
        assert_eq!(size.cols, 70);
    }
}
