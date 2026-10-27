//! The credential of one local caller, and the host access of one local repository.
use crate::{HostAccess, HttpCredential};
use axum::http::HeaderMap;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_store::operations::{catalog, LocalRepository};
use std::net::SocketAddr;

/// The bearer and origin a local HTTP caller must present.
pub struct LocalAccess {
    credential: String,
    authority: String,
    origin: String,
}

impl LocalAccess {
    /// Bind a caller credential to one loopback address.
    pub fn new(token: &str, address: SocketAddr) -> Result<Self, OperationFailure> {
        let invalid = || OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        };
        if token.len() != 64
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            || address.ip() != std::net::Ipv4Addr::LOCALHOST
            || address.port() == 0
        {
            return Err(invalid());
        }
        Ok(Self {
            credential: format!("Bearer {token}"),
            authority: address.to_string(),
            origin: format!("http://{address}"),
        })
    }

    /// Check the destination and browser origin on both assets and operations.
    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        if single(headers, "host") != Some(self.authority.as_str())
            || (headers.contains_key("origin")
                && single(headers, "origin") != Some(self.origin.as_str()))
            || (headers.contains_key("sec-fetch-site")
                && !matches!(
                    single(headers, "sec-fetch-site"),
                    Some("same-origin" | "none")
                ))
        {
            return Err(OperationFailure::AccessDenied);
        }
        Ok(())
    }
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

/// Compares a secret with a supplied value without an early exit on the first different byte.
pub fn constant_time_eq(expected: &[u8], supplied: &[u8]) -> bool {
    let mut difference = expected.len() ^ supplied.len();
    for (index, expected_byte) in expected.iter().enumerate() {
        let supplied_byte = supplied.get(index).copied().unwrap_or_default();
        difference |= usize::from(expected_byte ^ supplied_byte);
    }
    difference == 0
}

impl HttpCredential for LocalAccess {
    /// The bearer first, then the origin checks.
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        let supplied = single(headers, "authorization")
            .unwrap_or_default()
            .as_bytes();
        let expected = self.credential.as_bytes();
        if !constant_time_eq(expected, supplied) {
            return Err(OperationFailure::Unauthenticated);
        }
        self.check_origin(headers)
    }
}

impl HostAccess for LocalRepository {
    fn advertises(&self, operation: &str) -> bool {
        [
            <catalog::ResolveRecord as catalog::Operation>::NAME,
            <catalog::Search as catalog::Operation>::NAME,
            <catalog::Trace as catalog::Operation>::NAME,
            <catalog::Impact as catalog::Operation>::NAME,
            <catalog::GetReviewedResource as catalog::Operation>::NAME,
            <catalog::ListDiscussions as catalog::Operation>::NAME,
            <catalog::GetDiscussionConversation as catalog::Operation>::NAME,
            <catalog::WriteDiscussion as catalog::Operation>::NAME,
            <catalog::WriteTargetDiscussion as catalog::Operation>::NAME,
        ]
        .contains(&operation)
            || catalog::definitions()
                .iter()
                .any(|definition| definition.name == operation)
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        Some((
            self.repository().to_owned(),
            self.scope().as_str().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{constant_time_eq, LocalAccess};
    use crate::HttpCredential as _;
    use axum::http::HeaderMap;
    use provenance_core::protocol::failure::OperationFailure;

    #[test]
    /// Implementation aid: this checks the constant-time credential comparison helper.
    fn credential_comparison_rejects_length_and_content_differences() {
        let expected = b"Bearer 0123456789abcdef";
        assert!(constant_time_eq(expected, expected));
        assert!(!constant_time_eq(expected, b"Bearer 0123456789abcdee"));
        assert!(!constant_time_eq(expected, b"Bearer 0"));
        assert!(!constant_time_eq(
            expected,
            b"Bearer 0123456789abcdef-extra"
        ));
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        headers
    }

    /// One header table and the refusal the credential answers with.
    type Case<'a> = (
        &'a str,
        &'a [(&'a str, &'a str)],
        Result<(), OperationFailure>,
    );

    #[test]
    fn local_access_keeps_its_origin_checks() {
        let access = LocalAccess::new(&"a".repeat(64), "127.0.0.1:4100".parse().unwrap()).unwrap();
        let bearer = format!("Bearer {}", "a".repeat(64));
        let bearer = bearer.as_str();
        let cases: [Case; 6] = [
            (
                "foreign host",
                &[("authorization", bearer), ("host", "127.0.0.1:4101")],
                Err(OperationFailure::AccessDenied),
            ),
            (
                "foreign origin",
                &[
                    ("authorization", bearer),
                    ("host", "127.0.0.1:4100"),
                    ("origin", "http://127.0.0.1:4101"),
                ],
                Err(OperationFailure::AccessDenied),
            ),
            (
                "cross-site fetch",
                &[
                    ("authorization", bearer),
                    ("host", "127.0.0.1:4100"),
                    ("sec-fetch-site", "cross-site"),
                ],
                Err(OperationFailure::AccessDenied),
            ),
            (
                "duplicated host header",
                &[
                    ("authorization", bearer),
                    ("host", "127.0.0.1:4100"),
                    ("host", "127.0.0.1:4100"),
                ],
                Err(OperationFailure::AccessDenied),
            ),
            (
                "missing bearer on a foreign host",
                &[("host", "127.0.0.1:4101")],
                Err(OperationFailure::Unauthenticated),
            ),
            (
                "same origin",
                &[
                    ("authorization", bearer),
                    ("host", "127.0.0.1:4100"),
                    ("origin", "http://127.0.0.1:4100"),
                    ("sec-fetch-site", "same-origin"),
                ],
                Ok(()),
            ),
        ];
        for (case, pairs, expected) in cases {
            assert_eq!(access.authenticate(&headers(pairs)), expected, "{case}");
        }
    }
}
