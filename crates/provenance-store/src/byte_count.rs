//! The byte count of a serialized value, without the bytes in memory.
use std::io::{self, Write};

/// A writer that counts the bytes written to it. A write that takes the count
/// over `limit` fails, so that a serializer stops at the limit.
pub(crate) struct ByteCount {
    bytes: usize,
    limit: usize,
}

impl ByteCount {
    /// A count that refuses a write that takes it over `limit` bytes.
    pub(crate) const fn up_to(limit: usize) -> Self {
        Self { bytes: 0, limit }
    }
}

impl Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|count| *count <= self.limit)
            .ok_or_else(|| io::Error::other("byte limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The bytes of the JSON form of `value`.
#[cfg(feature = "host")]
pub(crate) fn json_bytes<T: serde::Serialize + ?Sized>(value: &T) -> serde_json::Result<usize> {
    let mut count = ByteCount::up_to(usize::MAX);
    serde_json::to_writer(&mut count, value)?;
    Ok(count.bytes)
}
