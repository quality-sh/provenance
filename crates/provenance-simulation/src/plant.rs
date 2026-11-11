//! Planted faults: each one breaks one law, so that a test shows that the
//! law can fail. A planted run writes no failure file.

/// A fault that a test plants in the simulated storage or in the identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plant {
    /// An armed refusal lands the set and then answers a refusal (L1).
    LandRefused,
    /// After a moved base, an engine reads the old state again (L5).
    StaleRead,
    /// After a stop, a read outside every engine fails (L2).
    FailReopen,
    /// The first set that an engine publishes answers that it landed, but
    /// the storage drops it (L3).
    LoseWrite,
    /// The section lock holds nothing and a publish lands on any base (L4).
    NoConcurrencyControl,
    /// The first read of an engine fails (O1).
    FailRead,
    /// The storage sets the time of the newest message of each set that it
    /// lands to zero, so a later message has no later time (O2).
    BackdateMessage,
    /// The clock continues from the last run, so two runs of one case write
    /// other times (L8).
    SharedClock,
}
