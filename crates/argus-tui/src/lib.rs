use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

use anyhow::{Context, Result};
use argus_core::session::{
    AttachMode, AttachSessionRequest, ClientId, CompletedSession, InputLeaseState,
    ResizeSessionRequest, SessionApi, SessionEvent, SessionEventReceiver, SessionId, SessionSize,
    SessionSnapshot, StartSessionRequest, WriteInputRequest,
};
use argus_daemon::session::{SessionManager, SessionManagerConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    pub session_id: SessionId,
    pub snapshot: SessionSnapshot,
    pub lease: InputLeaseState,
    pub last_completed: Option<CompletedSession>,
    pub input_bytes_sent: u64,
    pub output_events_seen: u64,
}

pub struct LocalSessionApp {
    manager: SessionManager,
    client_id: ClientId,
    events: SessionEventReceiver,
    view: SessionView,
    last_error: Option<String>,
}

impl LocalSessionApp {
    pub fn start(size: SessionSize) -> Result<Self> {
        Self::start_with_log_dir(size, default_tui_log_dir())
    }

    fn start_with_log_dir(size: SessionSize, log_dir: PathBuf) -> Result<Self> {
        let manager = SessionManager::new(SessionManagerConfig::new(log_dir));
        let mut request = default_shell_request();
        request.size = size;

        let session_id = manager.start_session(request)?;
        let client_id = ClientId::new("local-tui")?;
        let events = manager.subscribe_session_events(session_id.clone())?;
        let attached = manager.attach_session(AttachSessionRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
            mode: AttachMode::InteractiveController,
        })?;

        Ok(Self {
            manager,
            client_id,
            events,
            view: SessionView {
                session_id,
                snapshot: attached.snapshot,
                lease: attached.lease,
                last_completed: None,
                input_bytes_sent: 0,
                output_events_seen: 0,
            },
            last_error: None,
        })
    }

    pub fn view(&self) -> &SessionView {
        &self.view
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn drain_events(&mut self) -> Result<()> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.apply_event(event)?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    self.last_error = Some("session event stream closed".to_string());
                    return Ok(());
                }
            }
        }
    }

    pub fn write_input(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() || self.view.snapshot.exited {
            return;
        }

        if let Err(error) = self.manager.write_input(WriteInputRequest {
            session_id: self.view.session_id.clone(),
            client_id: self.client_id.clone(),
            bytes: bytes.clone(),
        }) {
            self.last_error = Some(error.to_string());
        } else {
            self.view.input_bytes_sent += bytes.len() as u64;
            self.last_error = None;
        }
    }

    pub fn resize(&mut self, size: SessionSize) {
        if self.view.snapshot.size == size || self.view.snapshot.exited {
            return;
        }

        match self.manager.resize_session(ResizeSessionRequest {
            session_id: self.view.session_id.clone(),
            size,
        }) {
            Ok(snapshot) => {
                self.view.snapshot = snapshot;
                self.last_error = None;
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }

    pub fn shutdown(self) -> Result<CompletedSession> {
        self.manager.shutdown_session(self.view.session_id)
    }

    fn apply_event(&mut self, event: SessionEvent) -> Result<()> {
        apply_event_to_view(&mut self.view, event);

        if !self.view.snapshot.exited {
            self.view.snapshot = self
                .manager
                .snapshot_session(self.view.session_id.clone())
                .context("refreshing session snapshot after event")?;
        }

        Ok(())
    }
}

pub fn apply_event_to_view(view: &mut SessionView, event: SessionEvent) {
    match event {
        SessionEvent::Output { session_id, .. } if session_id == view.session_id => {
            view.output_events_seen += 1;
        }
        SessionEvent::Snapshot {
            session_id,
            snapshot,
        } if session_id == view.session_id => {
            view.snapshot = snapshot;
        }
        SessionEvent::LeaseChanged { session_id, change } if session_id == view.session_id => {
            view.lease = InputLeaseState {
                holder: change.current,
                generation: change.generation,
            };
        }
        SessionEvent::Exited {
            session_id,
            completed,
        } if session_id == view.session_id => {
            view.snapshot.output_seq = completed.output_seq;
            view.snapshot.bytes_logged = completed.bytes_logged;
            view.snapshot.visible_rows = completed.visible_rows.clone();
            view.snapshot.exited = true;
            view.last_completed = Some(completed);
        }
        _ => {}
    }
}

pub fn session_size_from_terminal(rows: u16, cols: u16) -> SessionSize {
    SessionSize {
        rows: usize::from(rows.max(1)),
        cols: usize::from(cols.max(1)),
        pixel_width: usize::from(cols.max(1)) * 10,
        pixel_height: usize::from(rows.max(1)) * 20,
        dpi: 96,
    }
}

fn default_tui_log_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            home.join(".local/state")
        })
        .join("argus/tui-sessions")
}

#[cfg(windows)]
fn default_shell_request() -> StartSessionRequest {
    StartSessionRequest::new("cmd.exe")
}

#[cfg(not(windows))]
fn default_shell_request() -> StartSessionRequest {
    let mut request = StartSessionRequest::new("/bin/sh");
    request.args = vec![
        "-lc".to_string(),
        "stty sane echo; exec \"${SHELL:-/bin/sh}\"".to_string(),
    ];
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use argus_core::session::{
        InputControllerKind, InputLeaseHolder, LeaseChange, LeaseChangeAction,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn lease_event_updates_view_holder() {
        let session_id = SessionId::new("session-1").expect("session id");
        let client_id = ClientId::new("local-tui").expect("client id");
        let mut view = test_view(session_id.clone());

        apply_event_to_view(
            &mut view,
            SessionEvent::LeaseChanged {
                session_id,
                change: LeaseChange {
                    generation: 7,
                    previous: None,
                    current: Some(InputLeaseHolder {
                        client_id: client_id.clone(),
                        kind: InputControllerKind::Interactive,
                    }),
                    action: LeaseChangeAction::Acquired,
                },
            },
        );

        assert_eq!(view.lease.generation, 7);
        assert_eq!(view.lease.holder.as_ref().unwrap().client_id, client_id);
    }

    #[test]
    fn exited_event_marks_snapshot_exited() {
        let session_id = SessionId::new("session-1").expect("session id");
        let mut view = test_view(session_id.clone());

        apply_event_to_view(
            &mut view,
            SessionEvent::Exited {
                session_id,
                completed: CompletedSession {
                    output_seq: 3,
                    bytes_logged: 21,
                    visible_rows: vec!["done".to_string()],
                },
            },
        );

        assert!(view.snapshot.exited);
        assert_eq!(view.snapshot.output_seq, 3);
        assert_eq!(view.snapshot.visible_rows, ["done"]);
        assert!(view.last_completed.is_some());
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "portable-pty ConPTY behavior needs a dedicated Windows lifecycle test"
    )]
    fn local_app_writes_input_and_refreshes_visible_rows() {
        let log_dir = unique_log_dir();
        let mut app = LocalSessionApp::start_with_log_dir(SessionSize::default(), log_dir.clone())
            .expect("start local session app");
        app.write_input(b"printf 'tui-ready\\r\\n'\n".to_vec());

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.drain_events().expect("drain session events");
            if app
                .view()
                .snapshot
                .visible_rows
                .iter()
                .any(|row| row.contains("tui-ready"))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for tui-ready; latest rows: {:?}; error: {:?}",
                app.view().snapshot.visible_rows,
                app.last_error()
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        app.shutdown().expect("shutdown local session app");
        let _ = std::fs::remove_dir_all(log_dir);
    }

    #[test]
    #[cfg_attr(
        windows,
        ignore = "portable-pty ConPTY behavior needs a dedicated Windows lifecycle test"
    )]
    fn local_app_shows_echo_before_enter() {
        let log_dir = unique_log_dir();
        let mut app = LocalSessionApp::start_with_log_dir(SessionSize::default(), log_dir.clone())
            .expect("start local session app");
        app.write_input(b"echo-before-enter".to_vec());

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.drain_events().expect("drain session events");
            if app
                .view()
                .snapshot
                .visible_rows
                .iter()
                .any(|row| row.contains("echo-before-enter"))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for input echo; latest rows: {:?}; error: {:?}",
                app.view().snapshot.visible_rows,
                app.last_error()
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        app.shutdown().expect("shutdown local session app");
        let _ = std::fs::remove_dir_all(log_dir);
    }

    fn test_view(session_id: SessionId) -> SessionView {
        SessionView {
            session_id,
            snapshot: SessionSnapshot {
                output_seq: 0,
                bytes_logged: 0,
                size: SessionSize::default(),
                visible_rows: Vec::new(),
                exited: false,
            },
            lease: InputLeaseState::default(),
            last_completed: None,
            input_bytes_sent: 0,
            output_events_seen: 0,
        }
    }

    fn unique_log_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "argus-tui-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }
}
