use super::ReviewRecord;
use crate::NodeType;
use serde::{Serialize, Serializer};

macro_rules! define_review_record_serde {
    ($( $variant:ident($record:ty, $kind:ident), )*) => {
        #[derive(Serialize)]
        #[serde(untagged)]
        enum RecordRef<'a> {
            $( $variant(&'a $record), )*
        }

        impl<'a> From<&'a ReviewRecord> for RecordRef<'a> {
            fn from(record: &'a ReviewRecord) -> Self {
                match record {
                    $( ReviewRecord::$variant(value) => Self::$variant(value), )*
                }
            }
        }

        impl ReviewRecord {
            pub fn deserialize_closed(
                kind: NodeType,
                value: &serde_json::Value,
            ) -> anyhow::Result<Self> {
                fn closed<T: serde::de::DeserializeOwned>(
                    value: &serde_json::Value,
                ) -> anyhow::Result<T> {
                    let text = serde_json::to_string(value)?;
                    let mut unknown = None;
                    let mut deserializer = serde_json::Deserializer::from_str(&text);
                    let record = serde_ignored::deserialize(&mut deserializer, |path| {
                        if unknown.is_none() {
                            unknown = Some(path.to_string());
                        }
                    })?;
                    anyhow::ensure!(
                        unknown.is_none(),
                        "unknown field `{}`",
                        unknown.unwrap_or_default()
                    );
                    Ok(record)
                }

                Ok(match kind {
                    $( NodeType::$kind => Self::$variant(closed::<$record>(value)?), )*
                })
            }
        }
    };
}

super::review_record_kinds!(define_review_record_serde);

impl Serialize for ReviewRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RecordRef::from(self).serialize(serializer)
    }
}
