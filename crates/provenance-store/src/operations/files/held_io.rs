use super::{
    held_metadata::{identity, FileIdentity, FileMetadata},
    RepositoryFileRefusal as Refusal,
};
use std::{fs::File, io::Read as _, io::Seek as _};

pub(super) fn read(
    mut file: File,
    limit: usize,
) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    read_inner(&mut file, limit)
}

pub(super) fn read_clone(
    file: &mut File,
    limit: usize,
) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    read_inner(file, limit)
}

fn read_inner(
    file: &mut File,
    limit: usize,
) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    let metadata = file.metadata().map_err(Refusal::Read)?;
    if metadata.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(Refusal::TooLarge { limit });
    }
    let identity = identity(file)?;
    let file_metadata = FileMetadata::read(file)?;
    crate::test_probes::at("repository_file_after_metadata")
        .map_err(std::io::Error::other)
        .map_err(Refusal::Read)?;
    file.rewind().map_err(Refusal::Read)?;
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    std::io::Read::by_ref(&mut *file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(Refusal::Read)?;
    if bytes.len() > limit {
        return Err(Refusal::TooLarge { limit });
    }
    std::str::from_utf8(&bytes).map_err(|_| Refusal::InvalidUtf8)?;
    Ok((bytes, identity, file_metadata))
}
