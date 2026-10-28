//! Typed follow output. The adapters validate the same contract before delivery.
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt as _;

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::review) enum Continuity {
    Current,
    Unknown,
}

#[derive(Serialize)]
pub(in crate::review) struct Document {
    pub epoch: String,
    pub sequence: u64,
    pub continuity: Continuity,
    pub records: Vec<Value>,
    #[serde(rename = "removedIds")]
    pub removed_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(in crate::review) enum OutputFrame {
    State {
        #[serde(flatten)]
        document: Document,
    },
    Reset {
        #[serde(flatten)]
        document: Document,
    },
    Disconnected {
        #[serde(flatten)]
        document: Document,
    },
    ReadError {
        epoch: String,
        sequence: u64,
        continuity: Continuity,
        reason: String,
    },
}

impl OutputFrame {
    pub const fn disconnected(epoch: String, sequence: u64) -> Self {
        Self::Disconnected {
            document: Document {
                epoch,
                sequence,
                continuity: Continuity::Unknown,
                records: Vec::new(),
                removed_ids: Vec::new(),
            },
        }
    }
    pub const fn read_error(epoch: String, sequence: u64, reason: String) -> Self {
        Self::ReadError {
            epoch,
            sequence,
            continuity: Continuity::Unknown,
            reason,
        }
    }
}

static CONTRACT: LazyLock<jsonschema::JSONSchema> = LazyLock::new(|| {
    let schema =
        serde_json::from_str(include_str!("output.schema.json")).expect("follow contract is JSON");
    jsonschema::JSONSchema::compile(&schema).expect("follow contract is a valid schema")
});

pub(in crate::review) async fn emit(frame: &OutputFrame) -> anyhow::Result<()> {
    let value = serde_json::to_value(frame)?;
    anyhow::ensure!(CONTRACT.is_valid(&value), "invalid follow output frame");
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    let mut output = tokio::io::stdout();
    output.write_all(&bytes).await?;
    output.flush().await?;
    Ok(())
}
