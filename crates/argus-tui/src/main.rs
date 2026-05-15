use std::ffi::{OsStr, OsString};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use argus_core::session::{
    InputControllerKind, SessionSize, StyledRow, TerminalColor, TerminalCursor, TerminalStyle,
};
use argus_tui::{LocalSessionApp, SessionView, session_size_from_terminal};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
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
    let mut terminal_area = Rect::default();
    let mut selection = None;

    loop {
        app.drain_events();
        terminal.draw(|frame| {
            terminal_area = draw(frame, app, sidebar_visible, pending_confirmation, selection);
        })?;

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
                    selection = None;
                    app.create_session(current_session_size(sidebar_visible)?);
                }
                Some(AppCommand::Close) => {
                    if pending_confirmation != Some(Confirmation::CloseSession) {
                        pending_confirmation = Some(Confirmation::CloseSession);
                        continue;
                    }
                    pending_confirmation = None;
                    selection = None;
                    app.close_selected_session();
                }
                Some(AppCommand::Next) => {
                    pending_confirmation = None;
                    selection = None;
                    app.select_next_session();
                }
                Some(AppCommand::Previous) => {
                    pending_confirmation = None;
                    selection = None;
                    app.select_previous_session();
                }
                Some(AppCommand::ToggleSidebar) => {
                    pending_confirmation = None;
                    selection = None;
                    sidebar_visible = !sidebar_visible;
                    app.resize(current_session_size(sidebar_visible)?);
                }
                Some(AppCommand::ScrollUp) => {
                    pending_confirmation = None;
                    selection = None;
                    app.scroll_selected(terminal_page_scroll_lines(terminal_area));
                }
                Some(AppCommand::ScrollDown) => {
                    pending_confirmation = None;
                    selection = None;
                    app.scroll_selected(-terminal_page_scroll_lines(terminal_area));
                }
                None => {
                    pending_confirmation = None;
                    if copy_selection_on_control_c(
                        key,
                        app.view(),
                        terminal_area,
                        &mut selection,
                        terminal.backend_mut(),
                    )? {
                        continue;
                    }
                    selection = None;
                    if let Some(bytes) = key_to_input(key) {
                        app.write_input(bytes);
                    }
                }
            },
            Event::Resize(cols, rows) => {
                pending_confirmation = None;
                selection = None;
                app.resize(session_size_from_app_terminal(rows, cols, sidebar_visible));
            }
            Event::Paste(text) => {
                pending_confirmation = None;
                selection = None;
                app.write_input(text.into_bytes());
            }
            Event::Mouse(mouse) => {
                if handle_mouse_event(
                    mouse,
                    terminal_area,
                    app,
                    &mut selection,
                    terminal.backend_mut(),
                )? {
                    pending_confirmation = None;
                }
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
    selection: Option<TerminalSelection>,
) -> Rect {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let terminal_area = if sidebar_visible {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(20)])
            .split(root[0]);
        draw_sidebar(frame, body[0], app);
        draw_terminal(frame, body[1], app.view(), selection);
        body[1]
    } else {
        draw_terminal(frame, root[0], app.view(), selection);
        root[0]
    };

    draw_status(
        frame,
        root[1],
        app.view(),
        app.last_error(),
        pending_confirmation,
    );
    terminal_area
}

fn draw_sidebar(frame: &mut ratatui::Frame<'_>, area: Rect, app: &LocalSessionApp) {
    let rows = app
        .sessions()
        .enumerate()
        .flat_map(|(index, view)| session_sidebar_rows(index, app.selected_index(), view))
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(rows)
            .block(Block::default().borders(Borders::RIGHT))
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

fn draw_terminal(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    view: &SessionView,
    selection: Option<TerminalSelection>,
) {
    let visible_height = usize::from(area.height);
    let start = view
        .snapshot
        .visible_rows
        .len()
        .saturating_sub(visible_height)
        .saturating_sub(view.scroll_offset);
    let end = start
        .saturating_add(visible_height)
        .min(view.snapshot.visible_rows.len());
    let render_cols = terminal_render_cols(area, view.snapshot.size.cols);
    let selection = selection.map(|selection| selection.to_visible_range(start));
    let rows = if let Some(selection) = selection {
        selected_rows_to_lines(
            &view.snapshot.visible_rows[start..end],
            render_cols,
            start,
            selection,
        )
    } else if let Some(styled_rows) = styled_rows_for_visible_range(&view.snapshot, start, end) {
        styled_rows_to_lines(styled_rows, render_cols)
    } else {
        view.snapshot.visible_rows[start..end]
            .iter()
            .map(|row| Line::from(row.as_str()))
            .collect::<Vec<_>>()
    };

    frame.render_widget(Paragraph::new(rows), area);
    if let Some(position) = terminal_cursor_position(area, start, &view.snapshot.cursor) {
        frame.set_cursor_position(position);
    }
}

fn terminal_render_cols(area: Rect, _snapshot_cols: usize) -> usize {
    usize::from(area.width)
}

fn terminal_cursor_position(
    area: Rect,
    start_row: usize,
    cursor: &TerminalCursor,
) -> Option<Position> {
    if !cursor.visible || cursor.row < start_row {
        return None;
    }

    let inner_width = usize::from(area.width);
    let inner_height = usize::from(area.height);
    let row = cursor.row - start_row;
    if cursor.col >= inner_width || row >= inner_height {
        return None;
    }

    Some(Position::new(
        area.x + u16::try_from(cursor.col).ok()?,
        area.y + u16::try_from(row).ok()?,
    ))
}

fn terminal_page_scroll_lines(area: Rect) -> isize {
    isize::try_from(area.height.saturating_sub(1).max(1)).unwrap_or(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSelection {
    start: TerminalPoint,
    end: TerminalPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalPoint {
    col: usize,
    row: usize,
}

impl TerminalSelection {
    fn new(point: TerminalPoint) -> Self {
        Self {
            start: point,
            end: point,
        }
    }

    fn update(&mut self, point: TerminalPoint) {
        self.end = point;
    }

    fn to_visible_range(self, first_visible_row: usize) -> VisibleSelection {
        let (start, end) = ordered_terminal_points(self.start, self.end);
        VisibleSelection {
            start: TerminalPoint {
                col: start.col,
                row: first_visible_row + start.row,
            },
            end: TerminalPoint {
                col: end.col,
                row: first_visible_row + end.row,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleSelection {
    start: TerminalPoint,
    end: TerminalPoint,
}

fn handle_mouse_event(
    mouse: MouseEvent,
    terminal_area: Rect,
    app: &mut LocalSessionApp,
    selection: &mut Option<TerminalSelection>,
    output: &mut CrosstermBackend<Stdout>,
) -> Result<bool> {
    match mouse.kind {
        MouseEventKind::ScrollUp
            if terminal_area_contains(terminal_area, mouse.column, mouse.row) =>
        {
            *selection = None;
            app.scroll_selected(3);
            Ok(true)
        }
        MouseEventKind::ScrollDown
            if terminal_area_contains(terminal_area, mouse.column, mouse.row) =>
        {
            *selection = None;
            app.scroll_selected(-3);
            Ok(true)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if !terminal_area_contains(terminal_area, mouse.column, mouse.row) {
                return Ok(false);
            }
            let Some(point) = terminal_point_from_mouse(terminal_area, mouse) else {
                return Ok(false);
            };
            *selection = Some(TerminalSelection::new(point));
            Ok(true)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if selection.is_none()
                && !terminal_area_contains(terminal_area, mouse.column, mouse.row)
            {
                return Ok(false);
            }
            let Some(point) = terminal_point_from_mouse(terminal_area, mouse) else {
                return Ok(false);
            };
            if let Some(selection) = selection {
                selection.update(point);
            } else {
                *selection = Some(TerminalSelection::new(point));
            }
            Ok(true)
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let Some(mut selected) = *selection else {
                return Ok(false);
            };
            if let Some(point) = terminal_point_from_mouse(terminal_area, mouse) {
                selected.update(point);
                *selection = Some(selected);
            }
            let text = selected_text(app.view(), terminal_area, selected);
            if !text.trim().is_empty() {
                write_osc52_clipboard(output, &text)?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn terminal_area_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && row >= area.y
        && col < area.x.saturating_add(area.width)
        && row < area.y.saturating_add(area.height)
}

fn terminal_point_from_mouse(area: Rect, mouse: MouseEvent) -> Option<TerminalPoint> {
    if area.width == 0 || area.height == 0 {
        return None;
    }

    let max_col = area.x.saturating_add(area.width.saturating_sub(1));
    let max_row = area.y.saturating_add(area.height.saturating_sub(1));
    let col = mouse.column.clamp(area.x, max_col).saturating_sub(area.x);
    let row = mouse.row.clamp(area.y, max_row).saturating_sub(area.y);

    Some(TerminalPoint {
        col: usize::from(col),
        row: usize::from(row),
    })
}

fn selected_text(view: &SessionView, area: Rect, selection: TerminalSelection) -> String {
    let visible_height = usize::from(area.height);
    let first_visible_row = view
        .snapshot
        .visible_rows
        .len()
        .saturating_sub(visible_height)
        .saturating_sub(view.scroll_offset);
    let selection = selection.to_visible_range(first_visible_row);

    view.snapshot
        .visible_rows
        .iter()
        .enumerate()
        .skip(selection.start.row)
        .take(selection.end.row.saturating_sub(selection.start.row) + 1)
        .filter_map(|(row_index, row)| {
            selected_row_text(row, row_index, selection).map(|line| line.trim_end().to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn copy_selection_on_control_c(
    key: KeyEvent,
    view: &SessionView,
    area: Rect,
    selection: &mut Option<TerminalSelection>,
    output: &mut CrosstermBackend<Stdout>,
) -> Result<bool> {
    if !is_control_c(key) {
        return Ok(false);
    }

    let Some(selected) = *selection else {
        return Ok(false);
    };

    let text = selected_text(view, area, selected);
    if !text.trim().is_empty() {
        write_osc52_clipboard(output, &text)?;
    }
    *selection = None;
    Ok(true)
}

fn styled_rows_for_visible_range(
    snapshot: &argus_core::session::SessionSnapshot,
    start: usize,
    end: usize,
) -> Option<&[StyledRow]> {
    let styled_start = snapshot.styled_rows_start;
    let styled_end = styled_start.checked_add(snapshot.styled_rows.len())?;
    if start < styled_start || end > styled_end {
        return None;
    }

    Some(&snapshot.styled_rows[start - styled_start..end - styled_start])
}

fn selected_rows_to_lines(
    rows: &[String],
    cols: usize,
    first_visible_row: usize,
    selection: VisibleSelection,
) -> Vec<Line<'static>> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let visible_row_index = first_visible_row + index;
            selected_row_to_line(row, cols, visible_row_index, selection)
        })
        .collect()
}

fn selected_row_to_line(
    row: &str,
    cols: usize,
    row_index: usize,
    selection: VisibleSelection,
) -> Line<'static> {
    let Some((start, end)) = selected_row_bounds(row, row_index, selection) else {
        return Line::from(row.to_string());
    };

    let padded = pad_to_cols(row, cols);
    let before = slice_chars(&padded, 0, start);
    let selected = slice_chars(&padded, start, end);
    let after = slice_chars(&padded, end, padded.chars().count());
    Line::from(vec![
        Span::raw(before),
        Span::styled(selected, Style::default().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
    ])
}

fn selected_row_text(row: &str, row_index: usize, selection: VisibleSelection) -> Option<String> {
    let (start, end) = selected_row_bounds(row, row_index, selection)?;
    Some(slice_chars(row, start, end.min(row.chars().count())))
}

fn selected_row_bounds(
    row: &str,
    row_index: usize,
    selection: VisibleSelection,
) -> Option<(usize, usize)> {
    if row_index < selection.start.row || row_index > selection.end.row {
        return None;
    }

    let row_len = row.chars().count();
    let end_of_row = row_len.max(selection.end.col.saturating_add(1));
    let start = if row_index == selection.start.row {
        selection.start.col
    } else {
        0
    };
    let end = if row_index == selection.end.row {
        selection.end.col.saturating_add(1)
    } else {
        end_of_row
    };

    if start >= end {
        None
    } else {
        Some((start, end))
    }
}

fn ordered_terminal_points(
    start: TerminalPoint,
    end: TerminalPoint,
) -> (TerminalPoint, TerminalPoint) {
    if (end.row, end.col) < (start.row, start.col) {
        (end, start)
    } else {
        (start, end)
    }
}

fn pad_to_cols(row: &str, cols: usize) -> String {
    let width = row.chars().count();
    if width >= cols {
        row.to_string()
    } else {
        format!("{row}{}", " ".repeat(cols - width))
    }
}

fn slice_chars(value: &str, start: usize, end: usize) -> String {
    value
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn write_osc52_clipboard(output: &mut CrosstermBackend<Stdout>, text: &str) -> Result<()> {
    use std::io::Write;

    write!(output, "\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))
        .context("writing terminal clipboard selection")?;
    output
        .flush()
        .context("flushing terminal clipboard selection")
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let value = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        output.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(value & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
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
    if style.dim {
        output = output.add_modifier(Modifier::DIM);
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
            "seq {}  bytes {}  PgUp/PgDn scroll  Ctrl-N new  Ctrl-W close  F2 tabs  Alt-arrows switch  Ctrl-Q exit",
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
    ScrollUp,
    ScrollDown,
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
        KeyCode::PageUp => Some(AppCommand::ScrollUp),
        KeyCode::PageDown => Some(AppCommand::ScrollDown),
        _ => None,
    }
}

fn should_exit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn is_control_c(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
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
    let horizontal_chrome = if sidebar_visible { 28 } else { 0 };
    session_size_from_terminal(
        rows.saturating_sub(1),
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
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
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
        execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
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
    fn control_c_is_detected_for_selection_copy() {
        assert!(is_control_c(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(is_control_c(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL,
        )));
        assert!(!is_control_c(KeyEvent::new(
            KeyCode::Char('c'),
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
            key_to_command(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(AppCommand::ScrollUp)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            Some(AppCommand::ScrollDown)
        );
        assert_eq!(
            key_to_command(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn session_size_matches_terminal_pane_inner_area() {
        let size = session_size_from_app_terminal(24, 100, true);

        assert_eq!(size.rows, 23);
        assert_eq!(size.cols, 72);
    }

    #[test]
    fn session_size_expands_when_sidebar_is_hidden() {
        let size = session_size_from_app_terminal(24, 100, false);

        assert_eq!(size.rows, 23);
        assert_eq!(size.cols, 100);
    }

    #[test]
    fn terminal_selection_extracts_only_visible_terminal_text() {
        let view = SessionView {
            session_id: argus_core::session::SessionId::new("session-1").expect("session id"),
            snapshot: argus_core::session::SessionSnapshot {
                output_seq: 0,
                bytes_logged: 0,
                size: SessionSize::default(),
                visible_rows: vec![
                    "ignored".to_string(),
                    "alpha beta".to_string(),
                    "gamma delta".to_string(),
                ],
                styled_rows_start: 0,
                styled_rows: Vec::new(),
                cursor: argus_core::session::TerminalCursor {
                    row: 0,
                    col: 0,
                    visible: false,
                },
                current_working_directory: None,
                exited: false,
            },
            lease: argus_core::session::InputLeaseState::default(),
            last_completed: None,
            scroll_offset: 0,
        };
        let area = Rect {
            x: 28,
            y: 0,
            width: 20,
            height: 2,
        };
        let selection = TerminalSelection {
            start: TerminalPoint { col: 2, row: 0 },
            end: TerminalPoint { col: 4, row: 1 },
        };

        assert_eq!(selected_text(&view, area, selection), "pha beta\ngamma");
    }

    #[test]
    fn terminal_point_clamps_to_terminal_pane() {
        let area = Rect {
            x: 28,
            y: 1,
            width: 10,
            height: 4,
        };
        let point = terminal_point_from_mouse(
            area,
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 3,
                row: 99,
                modifiers: KeyModifiers::NONE,
            },
        )
        .expect("terminal point");

        assert_eq!(point, TerminalPoint { col: 0, row: 3 });
    }

    #[test]
    fn base64_encoder_pads_terminal_clipboard_payloads() {
        assert_eq!(base64_encode(b"Argus"), "QXJndXM=");
        assert_eq!(base64_encode(b"copy"), "Y29weQ==");
    }

    #[test]
    fn terminal_render_cols_expands_to_terminal_pane_width() {
        let area = Rect::new(0, 0, 102, 24);

        assert_eq!(terminal_render_cols(area, 80), 102);
    }

    #[test]
    fn terminal_render_cols_uses_terminal_pane_width_when_snapshot_is_wider() {
        let area = Rect::new(0, 0, 72, 24);

        assert_eq!(terminal_render_cols(area, 100), 72);
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
    fn styled_rows_are_selected_from_full_snapshot_range() {
        let snapshot = argus_core::session::SessionSnapshot {
            output_seq: 0,
            bytes_logged: 0,
            size: SessionSize::default(),
            visible_rows: vec![
                "history".to_string(),
                "visible-1".to_string(),
                "visible-2".to_string(),
            ],
            styled_rows_start: 0,
            styled_rows: vec![
                StyledRow {
                    spans: vec![StyledSpan {
                        text: "history".to_string(),
                        style: TerminalStyle::default(),
                    }],
                },
                StyledRow {
                    spans: vec![StyledSpan {
                        text: "visible-1".to_string(),
                        style: TerminalStyle::default(),
                    }],
                },
                StyledRow {
                    spans: vec![StyledSpan {
                        text: "visible-2".to_string(),
                        style: TerminalStyle::default(),
                    }],
                },
            ],
            cursor: TerminalCursor {
                row: 2,
                col: 0,
                visible: true,
            },
            current_working_directory: None,
            exited: false,
        };

        assert!(styled_rows_for_visible_range(&snapshot, 0, 2).is_some());
        assert!(styled_rows_for_visible_range(&snapshot, 1, 3).is_some());
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
            Some(Position::new(17, 22))
        );
    }

    #[test]
    fn dim_terminal_style_maps_to_ratatui_modifier() {
        let style = ratatui_style(&TerminalStyle {
            dim: true,
            ..TerminalStyle::default()
        });

        assert!(style.add_modifier.contains(Modifier::DIM));
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
