use std::ffi::{OsStr, OsString};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
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
    let mut app = start_app(size).context("starting local session")?;
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

#[cfg(unix)]
fn start_app(size: SessionSize) -> Result<LocalSessionApp> {
    match StartupMode::from_args(std::env::args_os().skip(1))? {
        StartupMode::Daemon { socket_path } => LocalSessionApp::connect(&socket_path, size)
            .with_context(|| {
                format!(
                    "connecting to Argus daemon socket at {}; start argus-daemon or pass --embedded for development mode",
                    socket_path.display()
                )
            }),
        StartupMode::Embedded => {
            tracing::warn!("starting embedded local session manager");
            LocalSessionApp::start(size)
        }
    }
}

#[cfg(not(unix))]
fn start_app(size: SessionSize) -> Result<LocalSessionApp> {
    match StartupMode::from_args(std::env::args_os().skip(1))? {
        StartupMode::Daemon { .. } => {
            bail!(
                "daemon socket mode is only available on Unix; pass --embedded for development mode"
            )
        }
        StartupMode::Embedded => LocalSessionApp::start(size),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartupMode {
    Daemon { socket_path: PathBuf },
    Embedded,
}

impl StartupMode {
    fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut mode = None;
        let mut socket_path = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--embedded") => {
                    if mode.replace(StartupMode::Embedded).is_some() {
                        bail!("--embedded can only be passed once");
                    }
                }
                Some("--socket") => {
                    if socket_path.is_some() {
                        bail!("--socket can only be passed once");
                    }
                    let Some(path) = args.next() else {
                        bail!("--socket requires a path");
                    };
                    socket_path = Some(PathBuf::from(path));
                }
                Some("--help") | Some("-h") => bail!(
                    "usage: argus-tui [--socket <path>] [--embedded]\n\nBy default argus-tui connects to the daemon Unix socket. Use --embedded only for isolated development."
                ),
                Some(flag) if flag.starts_with('-') => bail!("unknown option {flag}"),
                Some(value) => bail!("unexpected argument {value}"),
                None => bail!("argument is not valid UTF-8: {}", display_os_str(&arg)),
            }
        }

        if mode == Some(StartupMode::Embedded) && socket_path.is_some() {
            bail!("--embedded cannot be combined with --socket");
        }

        Ok(mode.unwrap_or_else(|| StartupMode::Daemon {
            socket_path: socket_path.unwrap_or_else(default_socket_path),
        }))
    }
}

#[cfg(unix)]
fn default_socket_path() -> PathBuf {
    argus_daemon::ipc::default_socket_path()
}

#[cfg(not(unix))]
fn default_socket_path() -> PathBuf {
    PathBuf::from("argus.sock")
}

fn display_os_str(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut LocalSessionApp) -> Result<()> {
    loop {
        app.drain_events();
        terminal.draw(|frame| draw(frame, app))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if should_exit(key) => return Ok(()),
            Event::Key(key) => match key_to_command(key) {
                Some(AppCommand::New) => {
                    app.create_session(current_session_size()?);
                }
                Some(AppCommand::Close) => {
                    app.close_selected_session();
                }
                Some(AppCommand::Next) => app.select_next_session(),
                Some(AppCommand::Previous) => app.select_previous_session(),
                None => {
                    if let Some(bytes) = key_to_input(key) {
                        app.write_input(bytes);
                    }
                }
            },
            Event::Resize(cols, rows) => {
                app.resize(session_size_from_app_terminal(rows, cols));
            }
            _ => {}
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &LocalSessionApp) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(root[0]);

    draw_sidebar(frame, body[0], app);
    draw_terminal(frame, body[1], app.view());
    draw_status(frame, root[1], app.view(), app.last_error());
}

fn draw_sidebar(frame: &mut ratatui::Frame<'_>, area: Rect, app: &LocalSessionApp) {
    let rows = app
        .sessions()
        .enumerate()
        .flat_map(|(index, view)| session_sidebar_rows(index, app.selected_index(), view))
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(rows)
            .block(Block::default().title("Sessions").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn session_sidebar_rows(index: usize, selected: usize, view: &SessionView) -> Vec<Line<'static>> {
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
    let marker = if index == selected { ">" } else { " " };
    let style = if index == selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    vec![
        Line::from(Span::styled(
            format!("{marker} {}  {state}", view.session_id),
            style,
        )),
        Line::from(format!(
            "  {holder}  {}x{}",
            view.snapshot.size.cols, view.snapshot.size.rows
        )),
        Line::from(""),
    ]
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
        .map(|row| Line::from(row.as_str()))
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
            Span::raw(error),
        ])
    } else {
        Line::from(format!(
            "seq {}  bytes {}  Ctrl-N new  Ctrl-W close  Alt-arrows switch  Esc/Ctrl-Q exit",
            view.snapshot.output_seq, view.snapshot.bytes_logged
        ))
    };
    frame.render_widget(Paragraph::new(status), area);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppCommand {
    New,
    Close,
    Next,
    Previous,
}

fn key_to_command(key: KeyEvent) -> Option<AppCommand> {
    match key.code {
        KeyCode::Char('n') | KeyCode::Char('N')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            Some(AppCommand::New)
        }
        KeyCode::Char('w') | KeyCode::Char('W')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            Some(AppCommand::Close)
        }
        KeyCode::Down | KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(AppCommand::Next)
        }
        KeyCode::Up | KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
            Some(AppCommand::Previous)
        }
        _ => None,
    }
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
    current_session_size()
}

fn current_session_size() -> Result<SessionSize> {
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
    fn reserved_control_keys_become_app_commands() {
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)),
            Some(AppCommand::New)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(AppCommand::Close)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT)),
            Some(AppCommand::Next)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)),
            Some(AppCommand::Previous)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn session_size_matches_terminal_pane_inner_area() {
        let size = session_size_from_app_terminal(24, 100);

        assert_eq!(size.rows, 21);
        assert_eq!(size.cols, 70);
    }

    #[test]
    fn startup_mode_defaults_to_daemon_socket() {
        assert!(matches!(
            StartupMode::from_args(Vec::<OsString>::new()).expect("startup mode"),
            StartupMode::Daemon { .. }
        ));
    }

    #[test]
    fn startup_mode_accepts_explicit_socket() {
        assert_eq!(
            StartupMode::from_args([
                OsString::from("--socket"),
                OsString::from("/tmp/argus.sock")
            ])
            .expect("startup mode"),
            StartupMode::Daemon {
                socket_path: PathBuf::from("/tmp/argus.sock")
            }
        );
    }

    #[test]
    fn startup_mode_accepts_explicit_embedded_mode() {
        assert_eq!(
            StartupMode::from_args([OsString::from("--embedded")]).expect("startup mode"),
            StartupMode::Embedded
        );
    }

    #[test]
    fn startup_mode_rejects_ambiguous_mode() {
        let error = StartupMode::from_args([
            OsString::from("--embedded"),
            OsString::from("--socket"),
            OsString::from("/tmp/argus.sock"),
        ])
        .expect_err("ambiguous mode should fail");

        assert!(error.to_string().contains("--embedded cannot be combined"));
    }
}
