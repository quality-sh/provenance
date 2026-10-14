use super::frames::Frame;
use provenance_macros::rule;
use provenance_store::review_changes::ReviewChanges;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

#[derive(Clone)]
pub(super) struct Subscribers {
    inner: Arc<Mutex<State>>,
    stop: watch::Sender<bool>,
}

struct State {
    epoch: String,
    scope: String,
    sequence: u64,
    available: bool,
    sender: broadcast::Sender<Option<Frame>>,
}

pub(super) struct Subscription {
    pub ready: Frame,
    pub receiver: broadcast::Receiver<Option<Frame>>,
    pub stopped: watch::Receiver<bool>,
}

impl Subscribers {
    pub fn new(epoch: String, scope: String) -> Self {
        let (sender, _) = broadcast::channel(64);
        let (stop, _) = watch::channel(false);
        Self {
            inner: Arc::new(Mutex::new(State {
                epoch,
                scope,
                sequence: 0,
                available: false,
                sender,
            })),
            stop,
        }
    }

    pub fn subscribe(&self) -> Option<Subscription> {
        let state = self.inner.lock().unwrap();
        if !state.available || *self.stop.borrow() {
            return None;
        }
        // Capture ready and attach under one mutex, so later frames stay queued.
        Some(Subscription {
            ready: Frame::Ready {
                epoch: state.epoch.clone(),
                sequence: state.sequence,
            },
            receiver: state.sender.subscribe(),
            stopped: self.stop.subscribe(),
        })
    }

    pub fn available(&self) {
        self.inner.lock().unwrap().available = true;
    }

    /// Sends observed canonical record changes to connected review readers through WebSocket.
    #[rule("rule_review_host_pushes_graph_changes")]
    pub fn changed(&self, mut changes: ReviewChanges) {
        let mut state = self.inner.lock().unwrap();
        changes.scopes.retain(|scope| scope.scope == state.scope);
        if changes.configuration_changed {
            Self::reset_locked(&mut state, "configuration-changed");
        } else if !changes.scopes.is_empty() {
            state.sequence += 1;
            let _ = state.sender.send(Some(Frame::Changed {
                epoch: state.epoch.clone(),
                sequence: state.sequence,
                scopes: changes.scopes,
            }));
        }
    }

    pub fn reset(&self, reason: &str) -> Frame {
        Self::reset_locked(&mut self.inner.lock().unwrap(), reason)
    }

    fn reset_locked(state: &mut State, reason: &str) -> Frame {
        state.sequence += 1;
        let frame = Frame::Reset {
            epoch: state.epoch.clone(),
            sequence: state.sequence,
            reason: reason.to_owned(),
        };
        let _ = state.sender.send(Some(frame.clone()));
        frame
    }

    pub fn unavailable(&self, reason: &str, error: &str) {
        let mut state = self.inner.lock().unwrap();
        if state.available {
            Self::reset_locked(&mut state, reason);
            eprintln!("Live review updates are unavailable: {error}");
            state.available = false;
            let _ = state.sender.send(None);
        }
    }

    pub fn stop(&self) {
        self.stop.send_replace(true);
    }

    pub fn stopped(&self) -> watch::Receiver<bool> {
        self.stop.subscribe()
    }
}
