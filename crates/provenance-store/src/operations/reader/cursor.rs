//! The shared continuation identity and authenticated position.
#[cfg(feature = "host")]
use super::ReadContext;
#[cfg(feature = "host")]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
#[cfg(feature = "host")]
use hmac::{Hmac, KeyInit as _, Mac};
#[cfg(feature = "host")]
use provenance_core::protocol::read_failure::ReadFailure;
#[cfg(feature = "host")]
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
#[cfg(feature = "host")]
use sha2::{Digest, Sha256};

#[cfg(feature = "host")]
type Signature = Hmac<Sha256>;

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    pub stage: u8,
    pub rank: u8,
    pub counter: i64,
    pub id: String,
}

#[cfg(feature = "host")]
#[derive(Serialize, Deserialize)]
struct Payload {
    identity: String,
    instance: String,
    serial: i64,
    digest: String,
    derivation: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live_revision: Option<String>,
    position: Position,
}

#[cfg(feature = "host")]
pub struct Cursor {
    identity: String,
    key: Vec<u8>,
    live_revision: Option<String>,
}

#[cfg(feature = "host")]
impl Cursor {
    /// Binds continuation to the operation, resolved target, scope, and selector.
    #[rule("rule_cursor_binds_query_identity")]
    pub fn open(
        ctx: &ReadContext,
        operation: &str,
        selector: &impl Serialize,
        token: Option<&str>,
    ) -> anyhow::Result<(Self, Position)> {
        Self::open_at_live_revision(ctx, operation, selector, token, None)
    }

    /// Opens a cursor whose snapshot also includes one locked live source.
    pub fn open_live(
        ctx: &ReadContext,
        operation: &str,
        selector: &impl Serialize,
        token: Option<&str>,
        live_revision: &str,
    ) -> anyhow::Result<(Self, Position)> {
        Self::open_at_live_revision(
            ctx,
            operation,
            selector,
            token,
            Some(live_revision.to_string()),
        )
    }

    fn open_at_live_revision(
        ctx: &ReadContext,
        operation: &str,
        selector: &impl Serialize,
        token: Option<&str>,
        live_revision: Option<String>,
    ) -> anyhow::Result<(Self, Position)> {
        let identity = hex::encode(Sha256::digest(serde_json::to_vec(&(
            1,
            operation,
            ctx.repo(),
            ctx.snapshot().scope(),
            selector,
        ))?));
        let cursor = Self {
            identity,
            key: signing_key(ctx)?,
            live_revision,
        };
        let Some(token) = token else {
            return Ok((cursor, Position::default()));
        };
        let invalid = || ReadFailure::CursorInvalid;
        if token.len() > 8192 {
            return Err(invalid().into());
        }
        let (body, signature) = token.split_once('.').ok_or_else(invalid)?;
        let body = URL_SAFE_NO_PAD.decode(body).map_err(|_| invalid())?;
        let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| invalid())?;
        let mut mac = Signature::new_from_slice(&cursor.key)?;
        mac.update(&body);
        mac.verify_slice(&signature).map_err(|_| invalid())?;
        let payload: Payload = serde_json::from_slice(&body).map_err(|_| invalid())?;
        if payload.identity != cursor.identity {
            return Err(invalid().into());
        }
        check_revision(ctx, &payload)?;
        if payload.live_revision != cursor.live_revision {
            return Err(ReadFailure::CursorRevisionChanged.into());
        }
        Ok((cursor, payload.position))
    }

    pub fn encode(&self, ctx: &ReadContext, position: Position) -> anyhow::Result<String> {
        let snapshot = ctx.snapshot();
        let payload = Payload {
            identity: self.identity.clone(),
            instance: snapshot.instance_id().into(),
            serial: snapshot.serial(),
            digest: snapshot.digest().into(),
            derivation: crate::operations::stamp::READ_DERIVATION,
            live_revision: self.live_revision.clone(),
            position,
        };
        let bytes = serde_json::to_vec(&payload)?;
        let mut mac = Signature::new_from_slice(&self.key)?;
        mac.update(&bytes);
        let token = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(bytes),
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        );
        if token.len() > 8192 {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        Ok(token)
    }
}

#[cfg(feature = "host")]
/// A changed projection cannot answer a continuation at the old revision.
#[rule("rule_cursor_restarts_on_revision_change")]
fn check_revision(ctx: &ReadContext, payload: &Payload) -> anyhow::Result<()> {
    let snapshot = ctx.snapshot();
    if payload.instance != snapshot.instance_id()
        || payload.serial != snapshot.serial()
        || payload.digest != snapshot.digest()
        || payload.derivation != crate::operations::stamp::READ_DERIVATION
    {
        return Err(ReadFailure::CursorRevisionChanged.into());
    }
    Ok(())
}

#[cfg(feature = "host")]
/// The key of the read database that signs this read's cursors.
fn signing_key(ctx: &ReadContext) -> anyhow::Result<Vec<u8>> {
    Ok(ctx.repository().read_database()?.cursor_key()?.to_vec())
}

#[cfg(test)]
#[cfg(feature = "host")]
mod tests {
    use super::Signature;
    use hmac::{KeyInit as _, Mac as _};

    // This test has no Rule. It keeps the cursor signature the same when the hash library changes.
    #[test]
    fn signature_is_hmac_sha256() {
        // RFC 4231, test case 2.
        let mut mac = Signature::new_from_slice(b"Jefe").unwrap();
        mac.update(b"what do ya want for nothing?");
        assert_eq!(
            hex::encode(mac.finalize().into_bytes()),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }
}
