use super::{OutputState, StageIdentity, TransactionDirectory};
use crate::wiki::publish::{CleanupWarning, PublishError};
use camino::Utf8Path;

pub(in crate::wiki::publish) fn replace_output(
    output: &Utf8Path,
    policy: super::super::OutputPolicy,
    transaction: &TransactionDirectory,
    output_state: OutputState,
    stage_identity: &StageIdentity,
) -> Result<Vec<CleanupWarning>, PublishError> {
    replace_output_with_validation(
        output,
        transaction,
        output_state,
        stage_identity,
        |_, _| Ok(()),
        |_| Ok(()),
        |backup| transaction.validate_output(&transaction.leaves.backup, backup, policy),
    )
}

#[cfg(test)]
pub(in crate::wiki::publish) fn replace_output_with(
    output: &Utf8Path,
    paths: &super::TransactionPaths,
    before_rename: impl FnMut(&Utf8Path, &Utf8Path) -> std::io::Result<()>,
    before_cleanup: impl FnMut(&Utf8Path) -> std::io::Result<()>,
) -> Result<Vec<CleanupWarning>, PublishError> {
    let transaction = TransactionDirectory::open(output)?;
    let output_state = super::stage::output_identity(output)?;
    // A plain File::open on a directory fails ERROR_ACCESS_DENIED on Windows
    // (no backup semantics); the no-follow directory open is also the right
    // semantics for a transaction-owned stage.
    let stage = crate::safe_fs::Directory::open(paths.stage.as_std_path()).map_err(|error| {
        PublishError::io("open staging directory identity", &paths.stage, error)
    })?;
    let stage_identity = StageIdentity::from_file(stage.as_file()).map_err(|error| {
        PublishError::io("record staging directory identity", &paths.stage, error)
    })?;
    replace_output_with_validation(
        output,
        &transaction,
        output_state,
        &stage_identity,
        before_rename,
        before_cleanup,
        |_| Ok(()),
    )
}

fn replace_output_with_validation(
    output: &Utf8Path,
    transaction: &TransactionDirectory,
    output_state: OutputState,
    stage_identity: &StageIdentity,
    mut before_rename: impl FnMut(&Utf8Path, &Utf8Path) -> std::io::Result<()>,
    mut before_cleanup: impl FnMut(&Utf8Path) -> std::io::Result<()>,
    validate_backup: impl FnOnce(&Utf8Path) -> Result<(), PublishError>,
) -> Result<Vec<CleanupWarning>, PublishError> {
    let paths = &transaction.paths;
    transaction.verify_requested_parent(output)?;
    verify_stage_identity(
        stage_identity,
        transaction,
        &transaction.leaves.stage,
        &paths.stage,
        "staging directory was replaced during generation",
    )?;
    let mut rename = |from: &Utf8Path, to: &Utf8Path| {
        before_rename(from, to)?;
        let from_leaf = from.file_name().expect("transaction path has leaf");
        let to_leaf = to.file_name().expect("transaction path has leaf");
        transaction.rename(from_leaf, to_leaf)
    };
    if let OutputState::Existing(expected_identity) = output_state {
        rename(output, &paths.backup)
            .map_err(|error| PublishError::io("move previous output to backup", output, error))?;
        let validation = validate_backup(&paths.backup).and_then(|()| {
            match transaction.output_identity(&transaction.leaves.backup, &paths.backup)? {
                OutputState::Existing(actual_identity)
                    if actual_identity.0 == expected_identity.0 =>
                {
                    Ok(())
                }
                _ => Err(PublishError::OutputChanged {
                    path: output.to_path_buf(),
                    detail: "filesystem identity no longer matches the preflight output"
                        .to_string(),
                }),
            }
        });
        if let Err(validation) = validation {
            return Err(super::cleanup::rollback_validation_failure(
                output,
                paths,
                &validation,
                &mut rename,
            ));
        }
        if let Err(install) = rename(&paths.stage, output) {
            return Err(super::cleanup::rollback_install_failure(
                output,
                paths,
                install,
                &mut rename,
            ));
        }
        super::cleanup::verify_installed_stage_with_backup(
            stage_identity,
            output,
            transaction,
            &mut rename,
        )?;
        let cleanup = super::cleanup::cleanup_backup(transaction, &mut before_cleanup);
        if let Err(error) = cleanup {
            return Ok(vec![CleanupWarning {
                path: paths.backup.clone(),
                action: "remove previous output backup",
                error: error.to_string(),
            }]);
        }
    } else {
        rename(&paths.stage, output).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                PublishError::OutputChanged {
                    path: output.to_path_buf(),
                    detail: "output appeared after preflight".to_string(),
                }
            } else {
                PublishError::io("install completed wiki", output, error)
            }
        })?;
        if let Err(validation) = verify_stage_identity(
            stage_identity,
            transaction,
            &transaction.output_leaf,
            output,
            "installed output does not match the generated staging directory",
        ) {
            rename(output, &paths.stage).map_err(|error| {
                PublishError::io("quarantine replaced staging directory", output, error)
            })?;
            return Err(validation);
        }
    }
    Ok(Vec::new())
}

pub(super) fn verify_stage_identity(
    expected: &StageIdentity,
    transaction: &TransactionDirectory,
    leaf: &str,
    path: &Utf8Path,
    detail: &'static str,
) -> Result<(), PublishError> {
    if transaction
        .child_identity(leaf)
        .map_err(|error| PublishError::io("verify staging directory identity", path, error))?
        == expected.0
    {
        Ok(())
    } else {
        Err(PublishError::OutputChanged {
            path: path.to_path_buf(),
            detail: detail.to_string(),
        })
    }
}
