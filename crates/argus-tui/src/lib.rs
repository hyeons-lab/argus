use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

use anyhow::Result;
use argus_core::session::{
    AttachMode, AttachSessionRequest, ClientId, CompletedSession, InputLeaseState,
    ResizeSessionRequest, SessionApi, SessionEvent, SessionEventReceiver, SessionId, SessionSize,
    SessionSnapshot, StartSessionRequest, WriteInputRequest,
};
use argus_daemon::session::{SessionManager, SessionManagerConfig};

const MAX_EVENTS_PER_SESSION_TICK: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    pub session_id: SessionId,
    pub snapshot: SessionSnapshot,
    pub lease: InputLeaseState,
    pub last_completed: Option<CompletedSession>,
}

pub struct LocalSessionApp {
    manager: SessionManager,
    client_id: ClientId,
    sessions: Vec<SessionRuntime>,
    selected: usize,
    last_error: Option<String>,
}

struct SessionRuntime {
    events: SessionEventReceiver,
    view: SessionView,
}

impl LocalSessionApp {
    pub fn start(size: SessionSize) -> Result<Self> {
        Self::start_with_log_dir(size, default_tui_log_dir())
    }

    fn start_with_log_dir(size: SessionSize, log_dir: PathBuf) -> Result<Self> {
        let manager = SessionManager::new(SessionManagerConfig::new(log_dir));
        let client_id = ClientId::new("local-tui")?;
        let session = start_local_session(&manager, &client_id, size)?;

        Ok(Self {
            manager,
            client_id,
            sessions: vec![session],
            selected: 0,
            last_error: None,
        })
    }

    pub fn create_session(&mut self, size: SessionSize) {
        match start_local_session(&self.manager, &self.client_id, size) {
            Ok(session) => {
                self.sessions.push(session);
                self.selected = self.sessions.len() - 1;
                self.last_error = None;
            }
            Err(error) => {
                self.last_error = Some(format!("creating local session: {error}"));
            }
        }
    }

    pub fn close_selected_session(&mut self) {
        if self.sessions.len() <= 1 {
            self.last_error = Some("cannot close the last local session".to_string());
            return;
        }

        let session_id = self.sessions[self.selected].view.session_id.clone();
        if let Err(error) = self.manager.shutdown_session(session_id) {
            self.last_error = Some(format!("closing selected session: {error}"));
            return;
        }

        self.sessions.remove(self.selected);

        if self.selected >= self.sessions.len() {
            self.selected = self.sessions.len() - 1;
        }
        self.last_error = None;
    }

    pub fn select_next_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        self.selected = (self.selected + 1) % self.sessions.len();
        self.last_error = None;
    }

    pub fn select_previous_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        self.selected = (self.selected + self.sessions.len() - 1) % self.sessions.len();
        self.last_error = None;
    }

    pub fn sessions(&self) -> impl ExactSizeIterator<Item = &SessionView> {
        self.sessions.iter().map(|session| &session.view)
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn view(&self) -> &SessionView {
        &self.sessions[self.selected].view
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn drain_events(&mut self) {
        for session in &mut self.sessions {
            let mut refresh_snapshot = false;

            for _ in 0..MAX_EVENTS_PER_SESSION_TICK {
                match session.events.try_recv() {
                    Ok(event) => refresh_snapshot |= apply_event_to_view(&mut session.view, event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if !session.view.snapshot.exited {
                            self.last_error = Some(format!(
                                "session {} event stream closed",
                                session.view.session_id
                            ));
                        }
                        break;
                    }
                }
            }

            if refresh_snapshot && !session.view.snapshot.exited {
                let session_id = session.view.session_id.clone();
                match self.manager.snapshot_session(session_id) {
                    Ok(snapshot) => {
                        session.view.snapshot = snapshot;
                    }
                    Err(error) => {
                        self.last_error = Some(format!(
                            "refreshing session snapshot after output events: {error}"
                        ));
                    }
                }
            }
        }
    }

    pub fn write_input(&mut self, bytes: Vec<u8>) {
        let Some(session) = self.sessions.get_mut(self.selected) else {
            return;
        };

        if bytes.is_empty() || session.view.snapshot.exited {
            return;
        }

        if let Err(error) = self.manager.write_input(WriteInputRequest {
            session_id: session.view.session_id.clone(),
            client_id: self.client_id.clone(),
            bytes,
        }) {
            self.last_error = Some(error.to_string());
        } else {
            self.last_error = None;
        }
    }

    pub fn resize(&mut self, size: SessionSize) {
        let mut resize_error = None;
        let mut resized_any = false;

        for session in &mut self.sessions {
            if session.view.snapshot.size == size || session.view.snapshot.exited {
                continue;
            }

            resized_any = true;
            match self.manager.resize_session(ResizeSessionRequest {
                session_id: session.view.session_id.clone(),
                size: size.clone(),
            }) {
                Ok(snapshot) => {
                    session.view.snapshot = snapshot;
                }
                Err(error) if resize_error.is_none() => resize_error = Some(error.to_string()),
                Err(_) => {}
            }
        }

        if resize_error.is_some() || resized_any {
            self.last_error = resize_error;
        }
    }

    pub fn shutdown(self) -> Result<Vec<CompletedSession>> {
        let mut completed_sessions = Vec::new();
        let mut result = Ok(());

        for session in self.sessions {
            match self.manager.shutdown_session(session.view.session_id) {
                Ok(completed) => completed_sessions.push(completed),
                Err(error) if result.is_ok() => result = Err(error),
                Err(_) => {}
            }
        }

        result?;
        Ok(completed_sessions)
    }
}

fn start_local_session(
    manager: &SessionManager,
    client_id: &ClientId,
    size: SessionSize,
) -> Result<SessionRuntime> {
    let mut request = default_shell_request();
    request.size = size;

    let session_id = manager.start_session(request)?;
    let events = manager.subscribe_session_events(session_id.clone())?;
    let attached = manager.attach_session(AttachSessionRequest {
        session_id: session_id.clone(),
        client_id: client_id.clone(),
        mode: AttachMode::InteractiveController,
    })?;

    Ok(SessionRuntime {
        events,
        view: SessionView {
            session_id,
            snapshot: attached.snapshot,
            lease: attached.lease,
            last_completed: None,
        },
    })
}

pub fn apply_event_to_view(view: &mut SessionView, event: SessionEvent) -> bool {
    match event {
        SessionEvent::Output { session_id, .. } if session_id == view.session_id => true,
        SessionEvent::Snapshot {
            session_id,
            snapshot,
        } if session_id == view.session_id => {
            view.snapshot = snapshot;
            false
        }
        SessionEvent::LeaseChanged { session_id, change } if session_id == view.session_id => {
            view.lease = InputLeaseState {
                holder: change.current,
                generation: change.generation,
            };
            false
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
            false
        }
        _ => false,
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
    fn output_events_request_snapshot_refresh() {
        let session_id = SessionId::new("session-1").expect("session id");
        let mut view = test_view(session_id.clone());

        assert!(apply_event_to_view(
            &mut view,
            SessionEvent::Output {
                session_id,
                output_seq: 1,
                bytes: b"hello".to_vec(),
            },
        ));

        let snapshot_session_id = view.session_id.clone();
        let snapshot = view.snapshot.clone();
        assert!(!apply_event_to_view(
            &mut view,
            SessionEvent::Snapshot {
                session_id: snapshot_session_id,
                snapshot,
            },
        ));
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
            app.drain_events();
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
            app.drain_events();
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

    #[test]
    #[cfg_attr(
        windows,
        ignore = "portable-pty ConPTY behavior needs a dedicated Windows lifecycle test"
    )]
    fn local_app_creates_selects_and_closes_sessions() {
        let log_dir = unique_log_dir();
        let mut app = LocalSessionApp::start_with_log_dir(SessionSize::default(), log_dir.clone())
            .expect("start local session app");
        let first_session = app.view().session_id.clone();

        app.create_session(SessionSize::default());

        assert_eq!(app.sessions().len(), 2);
        assert_eq!(app.selected_index(), 1);
        assert_ne!(app.view().session_id, first_session);

        app.select_previous_session();
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.view().session_id, first_session);

        app.select_next_session();
        app.close_selected_session();

        assert_eq!(app.sessions().len(), 1);
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.view().session_id, first_session);

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
