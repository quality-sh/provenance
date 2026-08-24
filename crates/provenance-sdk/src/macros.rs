//! The macro projection over the string-keyed kernel.

/// Says whether a Rust identifier names a spec key.
///
/// An identifier `_` stands for `-` or `_` in the key, because an
/// identifier cannot carry a hyphen; every other byte must match
/// exactly. Const so the `provenance_spec!` link holds at compile time
/// on stable Rust.
#[must_use]
pub const fn identifier_matches_key(identifier: &str, key: &str) -> bool {
    let identifier = identifier.as_bytes();
    let key = key.as_bytes();
    if identifier.len() != key.len() {
        return false;
    }
    let mut index = 0;
    while index < identifier.len() {
        let expected = if key[index] == b'-' { b'_' } else { key[index] };
        if identifier[index] != expected {
            return false;
        }
        index += 1;
    }
    true
}

/// Declares one spec as a named function, and refuses at compile time an
/// identifier that does not spell the spec key.
///
/// ```
/// use provenance_sdk::{requirement, rule, provenance_spec};
///
/// provenance_spec!(share_links => "share-links" {
///     requirement("sharing")
///         .statement("Users can securely share documentation")
///         .rules([rule("expiry").statement("Share links expire within 30 days")]),
/// });
///
/// let document = share_links().unwrap();
/// assert_eq!(document.spec(), "share-links");
/// ```
#[macro_export]
macro_rules! provenance_spec {
    ($vis:vis $name:ident => $key:literal { $($requirement:expr),* $(,)? }) => {
        const _: () = assert!(
            $crate::identifier_matches_key(stringify!($name), $key),
            "provenance_spec! identifier does not spell the spec key"
        );
        $vis fn $name() -> Result<$crate::SpecDocument, $crate::AuthoringError> {
            $crate::spec($key).requirements([$($requirement),*]).build()
        }
    };
}

/// Records a rule's implementation site, and refuses at compile time a
/// file path that does not exist in the calling crate.
///
/// ```
/// use provenance_sdk::{implemented_by, rule};
///
/// let expiry = implemented_by!(
///     rule("expiry").statement("Share links expire within 30 days"),
///     "src/lib.rs",
///     verify
/// );
/// ```
#[macro_export]
macro_rules! implemented_by {
    ($rule:expr, $file:literal, $symbol:ident) => {{
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file));
        $rule.implemented_at($file, stringify!($symbol))
    }};
}
