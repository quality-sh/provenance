//! The name form of a temporary file of the store: a prefix that only one
//! kind of write uses, six letters or digits, and `.tmp`. The clean-up of a
//! stopped write finds its temporary files by this form.

/// The length of the random part of a temporary file name.
const RANDOM_LENGTH: usize = 6;
const SUFFIX: &str = ".tmp";

/// A builder of a temporary file whose name is `prefix`, six letters or
/// digits, and `.tmp`.
pub(in crate::storage::local) fn builder(prefix: &str) -> tempfile::Builder<'_, 'static> {
    let mut builder = tempfile::Builder::new();
    builder
        .prefix(prefix)
        .suffix(SUFFIX)
        .rand_bytes(RANDOM_LENGTH);
    builder
}

/// Whether `name` is `prefix`, six letters or digits, and `.tmp`.
pub(in crate::storage::local) fn is_temporary_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .is_some_and(|random| {
            random.len() == RANDOM_LENGTH && random.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}
