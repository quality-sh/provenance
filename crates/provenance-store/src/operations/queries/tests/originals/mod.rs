//! The pre-re-back executors, preserved verbatim for the differential
//! harness.
//!
//! Each file here carries the executor it preserves and the decision that
//! may remove it: these originals exist so the served executors can be
//! compared against the exact code they replace, over the shared fixture
//! corpus. Removal needs its own later decision and cannot land before the
//! scheduled wiki/gaps convergence completes, because the harness needs a
//! stable old side until the loader removal milestone.

pub(super) mod bindings;
pub(super) mod evidence;
pub(super) mod impact;
pub(super) mod records;
pub(super) mod symbols;
pub(super) mod walk;
