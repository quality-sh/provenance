//! End-to-end RBAC enforcement through the real `provenance` binary.
//!
//! Split by behavior: the mutation goldens, and the init-family laws.

#[path = "cli_rbac/init.rs"]
mod init;
#[path = "cli_rbac/mutations.rs"]
mod mutations;
#[path = "cli_rbac/support.rs"]
mod support;
