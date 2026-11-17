//! The few SQLite C functions that the store calls directly.
//!
//! sqlx links SQLite through libsqlite3-sys. The store declares these
//! functions itself and has no direct libsqlite3-sys dependency, so a
//! packaged store resolves libsqlite3-sys through sqlx alone.

use std::os::raw::{c_char, c_double, c_int, c_void};

/// An opaque SQLite connection.
#[repr(C)]
pub struct Sqlite3 {
    _private: [u8; 0],
}

/// An opaque SQLite prepared statement.
#[repr(C)]
pub struct Sqlite3Stmt {
    _private: [u8; 0],
}

pub const SQLITE_OK: c_int = 0;
pub const SQLITE_ROW: c_int = 100;
pub const SQLITE_STMTSTATUS_FULLSCAN_STEP: c_int = 1;

extern "C" {
    pub fn sqlite3_open(filename: *const c_char, db: *mut *mut Sqlite3) -> c_int;
    pub fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    pub fn sqlite3_prepare_v2(
        db: *mut Sqlite3,
        sql: *const c_char,
        bytes: c_int,
        statement: *mut *mut Sqlite3Stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    pub fn sqlite3_bind_double(statement: *mut Sqlite3Stmt, index: c_int, value: c_double)
        -> c_int;
    pub fn sqlite3_step(statement: *mut Sqlite3Stmt) -> c_int;
    pub fn sqlite3_column_bytes(statement: *mut Sqlite3Stmt, column: c_int) -> c_int;
    pub fn sqlite3_finalize(statement: *mut Sqlite3Stmt) -> c_int;
    pub fn sqlite3_next_stmt(db: *mut Sqlite3, statement: *mut Sqlite3Stmt) -> *mut Sqlite3Stmt;
    pub fn sqlite3_stmt_status(statement: *mut Sqlite3Stmt, op: c_int, reset: c_int) -> c_int;
}

/// The connection pointer of a sqlx handle as this module's type.
pub fn connection<T>(raw: std::ptr::NonNull<T>) -> *mut Sqlite3 {
    raw.as_ptr().cast::<c_void>().cast()
}
