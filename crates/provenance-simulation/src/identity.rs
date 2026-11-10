//! The clock and the ID source of a run.

use crate::generate;
use crate::plant::Plant;
use provenance_store::repository::{Clock, IdSource, RepositoryIdentity};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

/// The time of the first read of the clock of a run.
const START_SECONDS: i64 = 1_000_000_000;

/// A clock that moves `step` seconds at each read.
#[derive(Debug)]
struct CountingClock {
    seconds: AtomicI64,
    step: i64,
}

impl Clock for CountingClock {
    fn now(&self) -> time::OffsetDateTime {
        let seconds = self.seconds.fetch_add(self.step, Ordering::SeqCst);
        time::OffsetDateTime::from_unix_timestamp(seconds).expect("a valid time")
    }
}

/// The one clock of every run with the planted fault [`Plant::SharedClock`].
static SHARED_CLOCK: LazyLock<Arc<CountingClock>> = LazyLock::new(|| {
    Arc::new(CountingClock {
        seconds: AtomicI64::new(START_SECONDS),
        step: 1,
    })
});

/// IDs that count up.
#[derive(Debug)]
struct CountingIds(AtomicU64);

impl IdSource for CountingIds {
    fn new_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(u128::from(self.0.fetch_add(1, Ordering::SeqCst) + 1))
    }
}

/// The first ID of the initial state. The IDs of a run start at 1, so they
/// never meet an ID that the initial state holds.
pub const INITIAL_STATE_IDS: u64 = 1 << 32;

/// The identity that every engine of one run shares: one clock, one ID
/// source. A clock fault of `plant` changes the clock.
pub fn identity(plant: Option<Plant>) -> RepositoryIdentity {
    let clock: Arc<dyn Clock> = match plant {
        Some(Plant::SharedClock) => SHARED_CLOCK.clone(),
        Some(Plant::ClockBackwards) => Arc::new(CountingClock {
            seconds: AtomicI64::new(START_SECONDS),
            step: -1,
        }),
        _ => Arc::new(CountingClock {
            seconds: AtomicI64::new(START_SECONDS),
            step: 1,
        }),
    };
    bare_identity()
        .with_clock(clock)
        .with_ids(Arc::new(CountingIds(AtomicU64::new(0))))
}

/// An identity whose IDs count up after `start`.
pub fn identity_with_ids(start: u64) -> RepositoryIdentity {
    identity(None).with_ids(Arc::new(CountingIds(AtomicU64::new(start))))
}

/// The identity of the simulated repository with the system clock and IDs.
pub fn bare_identity() -> RepositoryIdentity {
    RepositoryIdentity::new(
        camino::Utf8PathBuf::from("/simulated"),
        scope(),
        provenance_store::settings::Settings::default(),
    )
}

pub fn scope() -> provenance_core::ScopeId {
    provenance_core::ScopeId::new(generate::SCOPE).expect("a scope")
}

pub fn stable(id: &str) -> provenance_core::StableId {
    provenance_core::StableId::new(id).expect("a stable ID")
}
