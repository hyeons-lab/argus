use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow, ensure};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSize {
    pub rows: usize,
    pub cols: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub dpi: usize,
}

impl Default for SessionSize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 480,
            dpi: 96,
        }
    }
}

impl SessionSize {
    fn validate(&self) -> Result<()> {
        ensure!(self.rows > 0, "session PTY rows must be greater than zero");
        ensure!(self.cols > 0, "session PTY cols must be greater than zero");
        ensure!(
            self.rows <= u16::MAX as usize,
            "session PTY rows must fit in u16"
        );
        ensure!(
            self.cols <= u16::MAX as usize,
            "session PTY cols must fit in u16"
        );
        ensure!(
            self.pixel_width <= u16::MAX as usize,
            "session PTY pixel width must fit in u16"
        );
        ensure!(
            self.pixel_height <= u16::MAX as usize,
            "session PTY pixel height must fit in u16"
        );
        ensure!(
            self.dpi <= u32::MAX as usize,
            "session terminal DPI must fit in u32"
        );
        Ok(())
    }

    fn pty_size(&self) -> PtySize {
        PtySize {
            rows: self.rows as u16,
            cols: self.cols as u16,
            pixel_width: self.pixel_width as u16,
            pixel_height: self.pixel_height as u16,
        }
    }

    fn terminal_size(&self) -> TerminalSize {
        TerminalSize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
            dpi: self.dpi as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub size: SessionSize,
    pub log_path: PathBuf,
}

impl SessionConfig {
    pub fn new(command: impl Into<String>, log_path: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            size: SessionSize::default(),
            log_path: log_path.into(),
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.command.trim().is_empty(),
            "session command must be set"
        );
        self.size.validate()
    }
}

#[derive(Debug)]
struct ArgusTerminalConfig;

impl TerminalConfiguration for ArgusTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

pub struct PtySession {
    _master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader: Box<dyn Read + Send>,
    log: File,
    bytes_logged: u64,
    terminal: Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedSession {
    pub bytes_logged: u64,
    pub output_seq: u64,
    pub visible_rows: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub output_seq: u64,
    pub bytes_logged: u64,
    pub size: SessionSize,
    pub visible_rows: Vec<String>,
    pub exited: bool,
}

pub struct SessionActor {
    tx: Sender<ActorMessage>,
    worker: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
}

impl PtySession {
    pub fn spawn(config: SessionConfig) -> Result<Self> {
        config.validate()?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(config.size.pty_size())
            .context("opening PTY")?;

        let mut command = CommandBuilder::new(&config.command);
        for arg in &config.args {
            command.arg(arg);
        }
        if let Some(cwd) = &config.cwd {
            command.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("spawning PTY child")?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .context("cloning PTY reader")?;
        let log = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&config.log_path)
            .with_context(|| format!("opening session log {}", config.log_path.display()))?;

        let terminal = Terminal::new(
            config.size.terminal_size(),
            Arc::new(ArgusTerminalConfig),
            "Argus",
            env!("CARGO_PKG_VERSION"),
            Box::new(TerminalInputSink),
        );

        Ok(Self {
            _master: pair.master,
            child,
            reader,
            log,
            bytes_logged: 0,
            terminal,
        })
    }

    pub fn drain_until_exit(mut self) -> Result<CompletedSession> {
        let mut chunk = [0; 8192];

        loop {
            match self.reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read_len) => self.ingest_output(&chunk[..read_len])?,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::BrokenPipe
                            | ErrorKind::ConnectionReset
                            | ErrorKind::UnexpectedEof
                    ) =>
                {
                    break;
                }
                Err(error) if is_closed_pty_error(&error) => break,
                Err(error) => return Err(error).context("reading PTY output"),
            }
        }

        self.child.wait().context("waiting for PTY child")?;
        self.log.flush().context("flushing session log")?;

        Ok(CompletedSession {
            bytes_logged: self.bytes_logged,
            output_seq: u64::from(self.bytes_logged > 0),
            visible_rows: visible_rows(&self.terminal),
        })
    }

    fn ingest_output(&mut self, bytes: &[u8]) -> Result<()> {
        self.log
            .write_all(bytes)
            .context("writing PTY output log")?;
        self.bytes_logged += bytes.len() as u64;
        self.terminal.advance_bytes(bytes);
        Ok(())
    }
}

impl SessionActor {
    pub fn spawn(config: SessionConfig) -> Result<Self> {
        config.validate()?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(config.size.pty_size())
            .context("opening PTY")?;

        let mut command = CommandBuilder::new(&config.command);
        for arg in &config.args {
            command.arg(arg);
        }
        if let Some(cwd) = &config.cwd {
            command.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("spawning PTY child")?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .context("cloning PTY reader")?;
        let writer = pair.master.take_writer().context("taking PTY writer")?;
        let log = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&config.log_path)
            .with_context(|| format!("opening session log {}", config.log_path.display()))?;
        let terminal = Terminal::new(
            config.size.terminal_size(),
            Arc::new(ArgusTerminalConfig),
            "Argus",
            env!("CARGO_PKG_VERSION"),
            Box::new(TerminalInputSink),
        );

        let (tx, rx) = mpsc::channel();
        let reader_tx = tx.clone();
        let reader = thread::spawn(move || read_pty_output(reader, reader_tx));
        let worker = thread::spawn(move || {
            let state = ActorState {
                master: pair.master,
                child,
                writer,
                log,
                terminal,
                size: config.size,
                bytes_logged: 0,
                output_seq: 0,
                exited: false,
            };
            run_actor(state, rx);
        });

        Ok(Self {
            tx,
            worker: Some(worker),
            reader: Some(reader),
        })
    }

    pub fn write_input(&self, bytes: impl Into<Vec<u8>>) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ActorMessage::Command(ActorCommand::WriteInput {
                bytes: bytes.into(),
                response: tx,
            }))
            .context("sending session input command")?;
        recv_actor_result(rx, "writing session input")
    }

    pub fn resize(&self, size: SessionSize) -> Result<SessionSnapshot> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ActorMessage::Command(ActorCommand::Resize {
                size,
                response: tx,
            }))
            .context("sending session resize command")?;
        recv_actor_result(rx, "resizing session")
    }

    pub fn snapshot(&self) -> Result<SessionSnapshot> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ActorMessage::Command(ActorCommand::Snapshot {
                response: tx,
            }))
            .context("sending session snapshot command")?;
        recv_actor_result(rx, "reading session snapshot")
    }

    pub fn shutdown(mut self) -> Result<CompletedSession> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ActorMessage::Command(ActorCommand::Shutdown {
                response: tx,
            }))
            .context("sending session shutdown command")?;
        let completed = recv_actor_result(rx, "shutting down session")?;
        self.join_threads();
        Ok(completed)
    }

    fn join_threads(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for SessionActor {
    fn drop(&mut self) {
        if self.worker.is_none() {
            return;
        }

        let (tx, rx) = mpsc::channel();
        let _ = self.tx.send(ActorMessage::Command(ActorCommand::Shutdown {
            response: tx,
        }));
        let _ = rx.recv();
        self.join_threads();
    }
}

struct ActorState {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    log: File,
    terminal: Terminal,
    size: SessionSize,
    bytes_logged: u64,
    output_seq: u64,
    exited: bool,
}

enum ActorMessage {
    Output(Vec<u8>),
    OutputClosed(Result<(), String>),
    Command(ActorCommand),
}

enum ActorCommand {
    WriteInput {
        bytes: Vec<u8>,
        response: Sender<ActorResult<()>>,
    },
    Resize {
        size: SessionSize,
        response: Sender<ActorResult<SessionSnapshot>>,
    },
    Snapshot {
        response: Sender<ActorResult<SessionSnapshot>>,
    },
    Shutdown {
        response: Sender<ActorResult<CompletedSession>>,
    },
}

type ActorResult<T> = std::result::Result<T, String>;

fn run_actor(mut state: ActorState, rx: Receiver<ActorMessage>) {
    while let Ok(message) = rx.recv() {
        match message {
            ActorMessage::Output(bytes) => {
                if let Err(error) = state.ingest_output(&bytes) {
                    tracing::warn!(error = ?error, "failed to ingest PTY output");
                }
            }
            ActorMessage::OutputClosed(result) => {
                if let Err(error) = result {
                    tracing::warn!(error, "PTY output reader closed with error");
                }
                state.mark_exited();
            }
            ActorMessage::Command(command) => {
                if state.handle_command(command) {
                    break;
                }
            }
        }
    }
}

impl ActorState {
    fn ingest_output(&mut self, bytes: &[u8]) -> Result<()> {
        self.log
            .write_all(bytes)
            .context("writing PTY output log")?;
        self.bytes_logged += bytes.len() as u64;
        self.output_seq += 1;
        self.terminal.advance_bytes(bytes);
        Ok(())
    }

    fn handle_command(&mut self, command: ActorCommand) -> bool {
        match command {
            ActorCommand::WriteInput { bytes, response } => {
                let result = self.write_input(&bytes).map_err(|error| error.to_string());
                let _ = response.send(result);
                false
            }
            ActorCommand::Resize { size, response } => {
                let result = self.resize(size).map_err(|error| error.to_string());
                let _ = response.send(result);
                false
            }
            ActorCommand::Snapshot { response } => {
                let _ = response.send(Ok(self.snapshot()));
                false
            }
            ActorCommand::Shutdown { response } => {
                let result = self.shutdown().map_err(|error| error.to_string());
                let _ = response.send(result);
                true
            }
        }
    }

    fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(!self.exited, "session has already exited");
        self.writer
            .write_all(bytes)
            .context("writing input to PTY")?;
        self.writer.flush().context("flushing PTY input")?;
        Ok(())
    }

    fn resize(&mut self, size: SessionSize) -> Result<SessionSnapshot> {
        size.validate()?;
        self.master
            .resize(size.pty_size())
            .context("resizing PTY")?;
        self.terminal.resize(size.terminal_size());
        self.size = size;
        Ok(self.snapshot())
    }

    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            output_seq: self.output_seq,
            bytes_logged: self.bytes_logged,
            size: self.size.clone(),
            visible_rows: visible_rows(&self.terminal),
            exited: self.exited,
        }
    }

    fn shutdown(&mut self) -> Result<CompletedSession> {
        if !self.exited {
            self.child.kill().context("terminating PTY child")?;
            self.mark_exited();
        }
        self.log.flush().context("flushing session log")?;

        Ok(CompletedSession {
            bytes_logged: self.bytes_logged,
            output_seq: self.output_seq,
            visible_rows: visible_rows(&self.terminal),
        })
    }

    fn mark_exited(&mut self) {
        if self.exited {
            return;
        }

        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Err(error) = self.child.wait() {
                    tracing::warn!(error = ?error, "failed waiting for PTY child");
                }
            }
            Err(error) => tracing::warn!(error = ?error, "failed polling PTY child"),
        }
        self.exited = true;
    }
}

fn read_pty_output(mut reader: Box<dyn Read + Send>, tx: Sender<ActorMessage>) {
    let mut chunk = [0; 8192];

    loop {
        match reader.read(&mut chunk) {
            Ok(0) => {
                let _ = tx.send(ActorMessage::OutputClosed(Ok(())));
                break;
            }
            Ok(read_len) => {
                if tx
                    .send(ActorMessage::Output(chunk[..read_len].to_vec()))
                    .is_err()
                {
                    break;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof
                ) || is_closed_pty_error(&error) =>
            {
                let _ = tx.send(ActorMessage::OutputClosed(Ok(())));
                break;
            }
            Err(error) => {
                let _ = tx.send(ActorMessage::OutputClosed(Err(error.to_string())));
                break;
            }
        }
    }
}

fn recv_actor_result<T>(rx: Receiver<ActorResult<T>>, context: &'static str) -> Result<T> {
    rx.recv()
        .with_context(|| format!("{context}: actor stopped"))?
        .map_err(|error| anyhow!("{context}: {error}"))
}

struct TerminalInputSink;

impl Write for TerminalInputSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
fn is_closed_pty_error(error: &std::io::Error) -> bool {
    // Linux PTY masters report EIO after the slave side closes.
    error.raw_os_error() == Some(5)
}

#[cfg(not(unix))]
fn is_closed_pty_error(_: &std::io::Error) -> bool {
    false
}

fn visible_rows(terminal: &Terminal) -> Vec<String> {
    let screen = terminal.screen();
    let end = screen.scrollback_rows();
    let start = end.saturating_sub(screen.physical_rows);

    screen
        .lines_in_phys_range(start..end)
        .iter()
        .map(|line| line.as_str().trim_end().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    #[cfg_attr(
        windows,
        ignore = "portable-pty ConPTY EOF handling needs a dedicated Windows lifecycle test"
    )]
    fn pty_session_logs_raw_bytes_and_updates_visible_rows() {
        let log_path = unique_log_path();
        let mut config = command_that_prints_marker(log_path.clone());
        config.size = SessionSize {
            rows: 6,
            cols: 32,
            pixel_width: 640,
            pixel_height: 240,
            dpi: 96,
        };

        let completed = PtySession::spawn(config)
            .expect("spawn PTY session")
            .drain_until_exit()
            .expect("drain PTY output");

        let logged = std::fs::read(&log_path).expect("read raw PTY log");
        let _ = std::fs::remove_file(&log_path);

        assert_eq!(completed.bytes_logged, logged.len() as u64);
        assert!(completed.output_seq > 0);
        assert!(
            String::from_utf8_lossy(&logged).contains("argus-ready"),
            "raw PTY output did not contain marker: {:?}",
            logged
        );
        assert!(
            completed
                .visible_rows
                .iter()
                .any(|row| row.contains("argus-ready")),
            "visible rows did not contain marker: {:?}",
            completed.visible_rows
        );
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "portable-pty ConPTY EOF handling needs a dedicated Windows lifecycle test"
    )]
    fn session_actor_accepts_input_resizes_snapshots_and_shuts_down() {
        let log_path = unique_log_path();
        let mut config = long_running_shell(log_path.clone());
        config.size = SessionSize {
            rows: 6,
            cols: 40,
            pixel_width: 800,
            pixel_height: 240,
            dpi: 96,
        };

        let actor = SessionActor::spawn(config).expect("spawn session actor");
        actor
            .write_input(input_that_prints_marker())
            .expect("write PTY input");

        let first = wait_for_visible_marker(&actor, "actor-ready");
        assert!(
            first.output_seq > 0,
            "snapshot should include output sequence after PTY output"
        );
        assert!(
            first.bytes_logged > 0,
            "snapshot should include logged byte count"
        );

        let resized = actor
            .resize(SessionSize {
                rows: 8,
                cols: 48,
                pixel_width: 960,
                pixel_height: 320,
                dpi: 96,
            })
            .expect("resize session actor");
        assert_eq!(resized.size.rows, 8);
        assert_eq!(resized.size.cols, 48);
        assert!(resized.output_seq >= first.output_seq);

        let completed = actor.shutdown().expect("shutdown session actor");
        let logged = std::fs::read(&log_path).expect("read raw PTY log");
        let _ = std::fs::remove_file(&log_path);

        assert_eq!(completed.bytes_logged, logged.len() as u64);
        assert!(completed.output_seq >= first.output_seq);
        assert!(
            String::from_utf8_lossy(&logged).contains("actor-ready"),
            "raw PTY output did not contain actor marker: {:?}",
            logged
        );
        assert!(
            completed
                .visible_rows
                .iter()
                .any(|row| row.contains("actor-ready")),
            "final visible rows did not contain marker: {:?}",
            completed.visible_rows
        );
    }

    fn unique_log_path() -> PathBuf {
        let unique = format!(
            "argus-pty-session-{}-{:?}.log",
            std::process::id(),
            std::thread::current().id()
        );
        std::env::temp_dir().join(unique)
    }

    #[cfg(windows)]
    fn command_that_prints_marker(log_path: PathBuf) -> SessionConfig {
        let mut config = SessionConfig::new("cmd.exe", log_path);
        config.args = vec!["/C".to_string(), "echo argus-ready".to_string()];
        config
    }

    #[cfg(not(windows))]
    fn command_that_prints_marker(log_path: PathBuf) -> SessionConfig {
        let mut config = SessionConfig::new("/bin/sh", log_path);
        config.args = vec!["-c".to_string(), "printf 'argus-ready\\r\\n'".to_string()];
        config
    }

    #[cfg(windows)]
    fn long_running_shell(log_path: PathBuf) -> SessionConfig {
        SessionConfig::new("cmd.exe", log_path)
    }

    #[cfg(not(windows))]
    fn long_running_shell(log_path: PathBuf) -> SessionConfig {
        SessionConfig::new("/bin/sh", log_path)
    }

    #[cfg(windows)]
    fn input_that_prints_marker() -> Vec<u8> {
        b"echo actor-ready\r\n".to_vec()
    }

    #[cfg(not(windows))]
    fn input_that_prints_marker() -> Vec<u8> {
        b"printf 'actor-ready\\r\\n'\n".to_vec()
    }

    fn wait_for_visible_marker(actor: &SessionActor, marker: &str) -> SessionSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            let snapshot = actor.snapshot().expect("snapshot session actor");
            if snapshot.visible_rows.iter().any(|row| row.contains(marker)) {
                return snapshot;
            }

            assert!(
                Instant::now() < deadline,
                "timed out waiting for {marker}; latest snapshot: {:?}",
                snapshot
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
