use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
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
    pub visible_rows: Vec<String>,
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
}
