//! A host policy that reports when repository preparation starts and holds it until the test
//! opens the gate. Preparation runs on the blocking pool, so a held call keeps its work slot.
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_transport::{fixture::FixtureAccess, HostAccess};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use tokio::sync::mpsc;

pub struct Gate {
    access: FixtureAccess,
    entered: mpsc::UnboundedSender<()>,
    state: Arc<GateState>,
}

struct GateState {
    open: Mutex<bool>,
    opened: Condvar,
    refuse: AtomicBool,
}

/// The test's side of the gate.
pub struct Control {
    entered: mpsc::UnboundedReceiver<()>,
    state: Arc<GateState>,
}

impl Gate {
    /// Wraps the fixture policy. A closed gate holds every preparation until `Control::open`.
    pub fn new(access: FixtureAccess, open: bool) -> (Self, Control) {
        let (entered, entries) = mpsc::unbounded_channel();
        let state = Arc::new(GateState {
            open: Mutex::new(open),
            opened: Condvar::new(),
            refuse: AtomicBool::new(false),
        });
        (
            Self {
                access,
                entered,
                state: state.clone(),
            },
            Control {
                entered: entries,
                state,
            },
        )
    }
}

impl Control {
    /// Waits until one more preparation has entered the gate, so its call holds a work slot.
    pub async fn entered(&mut self) {
        self.entered
            .recv()
            .await
            .expect("a preparation enters the gate");
    }

    /// Lets every held and later preparation continue.
    pub fn open(&self) {
        *self.state.open.lock().unwrap() = true;
        self.state.opened.notify_all();
    }

    /// Makes the identity check refuse every later call with `access_denied`.
    pub fn refuse_identities(&self) {
        self.state.refuse.store(true, Ordering::SeqCst);
    }
}

impl ContextResolver for Gate {
    fn authorize(
        &self,
        operation: &'static str,
        context: &RequestedContext,
    ) -> Result<(), OperationFailure> {
        if self.state.refuse.load(Ordering::SeqCst) {
            return Err(OperationFailure::AccessDenied);
        }
        self.access.authorize(operation, context)
    }

    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let _ = self.entered.send(());
        let mut open = self.state.open.lock().unwrap();
        while !*open {
            open = self.state.opened.wait(open).unwrap();
        }
        drop(open);
        self.access.prepare(operation, context, needs)
    }
}

impl HostAccess for Gate {
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Result<(), OperationFailure> {
        self.access.authenticate(headers)
    }

    fn advertises(&self, operation: &str) -> bool {
        self.access.advertises(operation)
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        self.access.bound_identity()
    }
}
