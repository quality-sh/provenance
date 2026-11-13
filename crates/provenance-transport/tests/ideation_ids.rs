//! A contribution, synthesis packet, proposal, assertion or disposition id
//! is immutable. When a hand edit or a branch merge puts one id on two lines
//! of its file, each ideation write refuses with `invalid_state_file` at the
//! second line, and the message names the two lines. Each read answers at
//! the stored serial, and its `freshness_error` gives the same fault.
#![cfg(feature = "test-fixture")]

#[path = "ideation_ids/fault_order.rs"]
mod fault_order;
#[path = "ideation_ids/reads.rs"]
mod reads;
#[path = "ideation_ids/repository.rs"]
mod repository;
#[path = "ideation_ids/writes.rs"]
mod writes;

mod support {
    pub mod ideation;
    pub mod records;
    pub mod resource_http;
}
