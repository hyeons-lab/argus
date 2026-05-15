use std::ffi::{OsStr, OsString};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use argus_core::session::{
    InputControllerKind, SessionSize, StyledRow, TerminalColor, TerminalCursor, TerminalStyle,
};
use argus_tui::{LocalSessionApp, SessionView, session_size_from_terminal};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

fn main() -> Result<()> {
    let startup_mode = StartupMode::from_args(std::env::args_os().skip(1))?;
    if startup_mode == StartupMode::Help {
        println!("{}", StartupMode::HELP);
        return Ok(());
    }

    let size = initial_session_size()?;
    let mut app = start_app(size, startup_mode).context("starting local session")?;
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
fn start_app(size: SessionSize, startup_mode: StartupMode) -> Result<LocalSessionApp> {
    match startup_mode {
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
        StartupMode::Help => unreachable!("help mode exits before startup"),
    }
}

#[cfg(not(unix))]
fn start_app(size: SessionSize, startup_mode: StartupMode) -> Result<LocalSessionApp> {
    match startup_mode {
        StartupMode::Daemon { .. } => {
            bail!(
                "daemon socket mode is only available on Unix; pass --embedded for development mode"
            )
        }
        StartupMode::Embedded => LocalSessionApp::start(size),
        StartupMode::Help => unreachable!("help mode exits before startup"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartupMode {
    Daemon { socket_path: PathBuf },
    Embedded,
    Help,
}

impl StartupMode {
    const HELP: &'static str = "usage: argus-tui [--socket <path>] [--embedded]\n\nBy default argus-tui connects to the daemon Unix socket on Unix and uses embedded development mode on non-Unix platforms. Use --embedded on Unix only for isolated development.";

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
                Some("--help") | Some("-h") => return Ok(StartupMode::Help),
                Some(flag) if flag.starts_with('-') => bail!("unknown option {flag}"),
                Some(value) => bail!("unexpected argument {value}"),
                None => bail!(
                    "unexpected non-UTF-8 argument {}; pass socket paths with --socket",
                    display_os_str(&arg)
                ),
            }
        }

        if mode == Some(StartupMode::Embedded) && socket_path.is_some() {
            bail!("--embedded cannot be combined with --socket");
        }

        if let Some(mode) = mode {
            return Ok(mode);
        }

        if let Some(socket_path) = socket_path {
            return Ok(StartupMode::Daemon { socket_path });
        }

        Ok(default_startup_mode())
    }
}

#[cfg(unix)]
fn default_startup_mode() -> StartupMode {
    StartupMode::Daemon {
        socket_path: argus_daemon::ipc::default_socket_path(),
    }
}

#[cfg(not(unix))]
fn default_startup_mode() -> StartupMode {
    StartupMode::Embedded
}

fn display_os_str(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut LocalSessionApp) -> Result<()> {
    let mut sidebar_visible = true;
    let mut pending_confirmation = None;

    loop {
        app.drain_events();
        terminal.draw(|frame| draw(frame, app, sidebar_visible, pending_confirmation))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Press => {}
            Event::Key(key) if should_exit(key) => {
                if app.exits_by_shutting_down_sessions() {
                    if pending_confirmation == Some(Confirmation::Exit) {
                        return Ok(());
                    }
                    pending_confirmation = Some(Confirmation::Exit);
                    continue;
                }
                return Ok(());
            }
            Event::Key(key) => match key_to_command(key) {
                Some(AppCommand::New) => {
                    pending_confirmation = None;
                    app.create_session(current_session_size(sidebar_visible)?);
                }
                Some(AppCommand::Close) => {
                    if pending_confirmation != Some(Confirmation::CloseSession) {
                        pending_confirmation = Some(Confirmation::CloseSession);
                        continue;
                    }
                    pending_confirmation = None;
                    app.close_selected_session();
                }
                Some(AppCommand::Next) => {
                    pending_confirmation = None;
                    app.select_next_session();
                }
                Some(AppCommand::Previous) => {
                    pending_confirmation = None;
                    app.select_previous_session();
                }
                Some(AppCommand::ToggleSidebar) => {
                    pending_confirmation = None;
                    sidebar_visible = !sidebar_visible;
                    app.resize(current_session_size(sidebar_visible)?);
                }
                None => {
                    pending_confirmation = None;
                    if let Some(bytes) = key_to_input(key) {
                        app.write_input(bytes);
                    }
                }
            },
            Event::Resize(cols, rows) => {
                pending_confirmation = None;
                app.resize(session_size_from_app_terminal(rows, cols, sidebar_visible));
            }
            _ => {}
        }
    }
}

fn draw(
    frame: &mut ratatui::Frame<'_>,
    app: &LocalSessionApp,
    sidebar_visible: bool,
    pending_confirmation: Option<Confirmation>,
) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    if sidebar_visible {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(20)])
            .split(root[0]);
        draw_sidebar(frame, body[0], app);
        draw_terminal(frame, body[1], app.view());
    } else {
        draw_terminal(frame, root[0], app.view());
    }

    draw_status(
        frame,
        root[1],
        app.view(),
        app.last_error(),
        pending_confirmation,
    );
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
    let render_cols = terminal_render_cols(area, view.snapshot.size.cols);
    let rows = if view.snapshot.styled_rows.len() == view.snapshot.visible_rows.len() {
        styled_rows_to_lines(&view.snapshot.styled_rows[start..], render_cols)
    } else {
        view.snapshot.visible_rows[start..]
            .iter()
            .map(|row| Line::from(row.as_str()))
            .collect::<Vec<_>>()
    };

    frame.render_widget(
        Paragraph::new(rows).block(Block::default().title("Terminal").borders(Borders::ALL)),
        area,
    );
    if let Some(position) = terminal_cursor_position(area, start, &view.snapshot.cursor) {
        frame.set_cursor_position(position);
    }
}

fn terminal_render_cols(area: Rect, _snapshot_cols: usize) -> usize {
    usize::from(area.width.saturating_sub(2))
}

fn terminal_cursor_position(
    area: Rect,
    start_row: usize,
    cursor: &TerminalCursor,
) -> Option<Position> {
    if !cursor.visible || cursor.row < start_row {
        return None;
    }

    let inner_width = usize::from(area.width.saturating_sub(2));
    let inner_height = usize::from(area.height.saturating_sub(2));
    let row = cursor.row - start_row;
    if cursor.col >= inner_width || row >= inner_height {
        return None;
    }

    Some(Position::new(
        area.x + 1 + u16::try_from(cursor.col).ok()?,
        area.y + 1 + u16::try_from(row).ok()?,
    ))
}

fn styled_rows_to_lines(rows: &[StyledRow], cols: usize) -> Vec<Line<'static>> {
    rows.iter()
        .map(|row| styled_row_to_line(row, cols))
        .collect()
}

fn row_background_style(row: &StyledRow) -> Option<TerminalStyle> {
    row.spans
        .iter()
        .rev()
        .find(|span| span.style.background.is_some() || span.style.reverse)
        .map(|span| span.style.clone())
}

fn styled_row_to_line(row: &StyledRow, cols: usize) -> Line<'static> {
    let mut width = 0;
    let line_style = row_background_style(row)
        .as_ref()
        .map(ratatui_style)
        .unwrap_or_default();
    let mut spans = row
        .spans
        .iter()
        .map(|span| {
            width += span.text.chars().count();
            Span::styled(span.text.clone(), ratatui_style(&span.style))
        })
        .collect::<Vec<_>>();
    if let Some(last_span) = row.spans.last()
        && (last_span.style.background.is_some() || last_span.style.reverse)
        && width < cols
    {
        spans.push(Span::styled(
            " ".repeat(cols - width),
            ratatui_style(&last_span.style),
        ));
    }
    let mut line = Line::from(spans);
    line.style = line_style;
    line
}

fn ratatui_style(style: &TerminalStyle) -> Style {
    let mut output = Style::default();
    if let Some(color) = style.foreground {
        output = output.fg(ratatui_color(color));
    }
    if let Some(color) = style.background {
        output = output.bg(ratatui_color(color));
    }
    if style.bold {
        output = output.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        output = output.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        output = output.add_modifier(Modifier::UNDERLINED);
    }
    if style.reverse {
        output = output.add_modifier(Modifier::REVERSED);
    }
    output
}

fn ratatui_color(color: TerminalColor) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
}

fn draw_status(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &SessionView,
    last_error: Option<&str>,
    pending_confirmation: Option<Confirmation>,
) {
    let status = if let Some(error) = last_error {
        Line::from(vec![
            Span::styled("error ", Style::default().fg(Color::Red)),
            Span::raw(error),
        ])
    } else if let Some(confirmation) = pending_confirmation {
        Line::from(Span::styled(
            confirmation.message(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(format!(
            "seq {}  bytes {}  Ctrl-N new  Ctrl-W close  F2 tabs  Alt-arrows switch  Ctrl-Q exit",
            view.snapshot.output_seq, view.snapshot.bytes_logged
        ))
    };
    frame.render_widget(Paragraph::new(status), area);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirmation {
    CloseSession,
    Exit,
}

impl Confirmation {
    fn message(self) -> &'static str {
        match self {
            Confirmation::CloseSession => "press Ctrl-W again to close this terminal",
            Confirmation::Exit => "press Ctrl-Q again to exit and close all embedded terminals",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppCommand {
    New,
    Close,
    Next,
    Previous,
    ToggleSidebar,
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
        KeyCode::F(2) => Some(AppCommand::ToggleSidebar),
        _ => None,
    }
}

fn should_exit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
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
        KeyCode::Esc => Some(b"\x1b".to_vec()),
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
    current_session_size(true)
}

fn current_session_size(sidebar_visible: bool) -> Result<SessionSize> {
    let (cols, rows) = crossterm::terminal::size()?;
    Ok(session_size_from_app_terminal(rows, cols, sidebar_visible))
}

fn session_size_from_app_terminal(rows: u16, cols: u16, sidebar_visible: bool) -> SessionSize {
    let horizontal_chrome = if sidebar_visible { 30 } else { 2 };
    session_size_from_terminal(
        rows.saturating_sub(3),
        cols.saturating_sub(horizontal_chrome),
    )
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
    use argus_core::session::{StyledSpan, TerminalStyle};

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
        assert_eq!(
            key_to_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(b"\x1b".to_vec())
        );
    }

    #[test]
    fn local_exit_uses_control_q() {
        assert!(!should_exit(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE
        )));
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
            key_to_command(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)),
            Some(AppCommand::ToggleSidebar)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn session_size_matches_terminal_pane_inner_area() {
        let size = session_size_from_app_terminal(24, 100, true);

        assert_eq!(size.rows, 21);
        assert_eq!(size.cols, 70);
    }

    #[test]
    fn session_size_expands_when_sidebar_is_hidden() {
        let size = session_size_from_app_terminal(24, 100, false);

        assert_eq!(size.rows, 21);
        assert_eq!(size.cols, 98);
    }

    #[test]
    fn terminal_render_cols_expands_to_terminal_pane_width() {
        let area = Rect::new(0, 0, 102, 24);

        assert_eq!(terminal_render_cols(area, 80), 100);
    }

    #[test]
    fn terminal_render_cols_uses_terminal_pane_width_when_snapshot_is_wider() {
        let area = Rect::new(0, 0, 72, 24);

        assert_eq!(terminal_render_cols(area, 100), 70);
    }

    #[test]
    fn styled_row_pads_trailing_background_to_session_width() {
        let row = StyledRow {
            spans: vec![StyledSpan {
                text: "input".to_string(),
                style: TerminalStyle {
                    background: Some(TerminalColor {
                        red: 10,
                        green: 20,
                        blue: 30,
                    }),
                    ..TerminalStyle::default()
                },
            }],
        };

        let line = styled_row_to_line(&row, 8);

        assert_eq!(line.width(), 8);
        assert!(line.style.bg.is_some());
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[1].content.as_ref(), "   ");
        assert!(line.spans[1].style.bg.is_some());
    }

    #[test]
    fn styled_rows_preserve_blank_rows() {
        let rows = vec![
            StyledRow {
                spans: vec![StyledSpan {
                    text: "top".to_string(),
                    style: TerminalStyle::default(),
                }],
            },
            StyledRow { spans: Vec::new() },
            StyledRow {
                spans: vec![StyledSpan {
                    text: "bottom".to_string(),
                    style: TerminalStyle::default(),
                }],
            },
        ];

        let lines = styled_rows_to_lines(&rows, 8);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].spans[0].content.as_ref(), "top");
        assert!(lines[1].spans.is_empty());
        assert_eq!(lines[2].spans[0].content.as_ref(), "bottom");
    }

    #[test]
    fn terminal_cursor_position_maps_visible_screen_to_terminal_area() {
        let area = Rect::new(10, 5, 82, 24);
        let cursor = TerminalCursor {
            row: 20,
            col: 7,
            visible: true,
        };

        assert_eq!(
            terminal_cursor_position(area, 3, &cursor),
            Some(Position::new(18, 23))
        );
    }

    #[test]
    fn terminal_cursor_position_hides_out_of_view_cursor() {
        let area = Rect::new(0, 0, 82, 24);
        let cursor = TerminalCursor {
            row: 2,
            col: 7,
            visible: true,
        };

        assert_eq!(terminal_cursor_position(area, 3, &cursor), None);
    }

    #[test]
    fn startup_mode_defaults_to_daemon_socket() {
        let startup_mode = StartupMode::from_args(Vec::<OsString>::new()).expect("startup mode");

        #[cfg(unix)]
        assert!(matches!(startup_mode, StartupMode::Daemon { .. }));
        #[cfg(not(unix))]
        assert_eq!(startup_mode, StartupMode::Embedded);
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

    #[test]
    fn startup_mode_rejects_reverse_ambiguous_mode() {
        let error = StartupMode::from_args([
            OsString::from("--socket"),
            OsString::from("/tmp/argus.sock"),
            OsString::from("--embedded"),
        ])
        .expect_err("ambiguous mode should fail");

        assert!(error.to_string().contains("--embedded cannot be combined"));
    }

    #[test]
    fn startup_mode_accepts_help() {
        assert_eq!(
            StartupMode::from_args([OsString::from("--help")]).expect("startup mode"),
            StartupMode::Help
        );
        assert_eq!(
            StartupMode::from_args([OsString::from("-h")]).expect("startup mode"),
            StartupMode::Help
        );
    }
}
