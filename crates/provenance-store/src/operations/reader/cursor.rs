//! The shared continuation identity and authenticated position.
use super::ReadContext;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

type Signature = Hmac<Sha256>;

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    pub stage: u8,
    pub rank: u8,
    pub counter: i64,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    identity: String,
    instance: String,
    serial: i64,
    digest: String,
    derivation: u32,
    position: Position,
}

pub struct Cursor {
    identity: String,
    key: Vec<u8>,
}

impl Cursor {
    /// Binds continuation to the operation, resolved target, scope, and selector.
    #[rule("rule_cursor_binds_query_identity")]
    pub fn open(
        ctx: &ReadContext,
        operation: &str,
        selector: &impl Serialize,
        token: Option<&str>,
    ) -> anyhow::Result<(Self, Position)> {
        let identity = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                1,
                operation,
                ctx.repo(),
                ctx.snapshot().scope(),
                selector,
            ))?)
        );
        let cursor = Self {
            identity,
            key: signing_key(ctx)?,
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

fn signing_key(ctx: &ReadContext) -> anyhow::Result<Vec<u8>> {
    let dir = crate::layout::ProvenanceLayout::new(ctx.repo().to_owned()).cache_dir();
    let path = dir.join("read-cursor.key");
    if !path.exists() {
        let mut file = tempfile::NamedTempFile::new_in(&dir)?;
        file.write_all(uuid::Uuid::new_v4().as_bytes())?;
        file.write_all(uuid::Uuid::new_v4().as_bytes())?;
        match file.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(33)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() == 32, "invalid cursor signing key");
    Ok(bytes)
}
