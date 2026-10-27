//! Bounded acquisition of exclusive file locks.

use std::{
    fs::{File, TryLockError},
    io,
    time::{Duration, Instant},
};

/// The maximum wait for an exclusive lock held by another file handle.
pub const WAIT_LIMIT: Duration = Duration::from_secs(10);
const RETRY_PAUSE: Duration = Duration::from_millis(10);

/// Acquires an exclusive lock or returns a timeout. The caller owns the file
/// and releases its lock by closing it. File access errors return at once.
pub fn exclusive(file: &File) -> io::Result<()> {
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::Error(error)) => return Err(error),
            Err(TryLockError::WouldBlock) if started.elapsed() >= WAIT_LIMIT => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out after 10 seconds waiting for an exclusive file lock",
                ));
            }
            Err(TryLockError::WouldBlock) => std::thread::sleep(RETRY_PAUSE),
        }
    }
}
