use super::{PublicationLock, TransactionDirectory};
use crate::wiki::publish::PublishError;
use std::io::Write;

pub(in crate::wiki::publish) fn acquire_lock(
    transaction: &TransactionDirectory,
) -> Result<PublicationLock, PublishError> {
    let paths = &transaction.paths;
    let lock = transaction
        .create_file(&transaction.leaves.lock)
        .map_err(|error| PublishError::io("create publication lock", &paths.lock, error))?;
    let identity_file = lock
        .try_clone()
        .map_err(|error| PublishError::io("clone publication lock handle", &paths.lock, error))?;
    let identity = same_file::Handle::from_file(identity_file).map_err(|error| {
        PublishError::io("record publication lock identity", &paths.lock, error)
    })?;
    let mut lock = PublicationLock {
        file: lock,
        identity,
    };
    let initialization = lock
        .file
        .write_all(b"provenance wiki publication in progress\n")
        .map_err(|error| PublishError::io("write publication lock", &paths.lock, error))
        .and_then(|()| {
            lock.file
                .sync_all()
                .map_err(|error| PublishError::io("sync publication lock", &paths.lock, error))
        });
    if let Err(primary) = initialization {
        return match lock.cleanup(transaction) {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(PublishError::CleanupFailed {
                primary: Box::new(primary),
                path: paths.lock.clone(),
                cleanup,
            }),
        };
    }
    Ok(lock)
}

impl PublicationLock {
    pub(in crate::wiki::publish) fn cleanup(
        self,
        transaction: &TransactionDirectory,
    ) -> std::io::Result<()> {
        transaction.rename(&transaction.leaves.lock, &transaction.leaves.lock_cleanup)?;
        if transaction.child_identity(&transaction.leaves.lock_cleanup)? != self.identity {
            let mismatch = std::io::Error::other("publication lock path changed before cleanup");
            return match transaction
                .rename(&transaction.leaves.lock_cleanup, &transaction.leaves.lock)
            {
                Ok(()) => Err(mismatch),
                Err(restore) => Err(std::io::Error::other(format!(
                    "{mismatch}; restoring the replacement lock path also failed: {restore}"
                ))),
            };
        }
        drop(self);
        transaction.remove_file(&transaction.leaves.lock_cleanup)
    }
}
