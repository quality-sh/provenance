use serde::{Deserialize, Serialize};

use super::parsing::parse_enum_word;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "resolution")]
    Resolution,
    #[serde(rename = "rule")]
    Rule,
    #[serde(rename = "topic")]
    Topic,
    #[serde(rename = "question")]
    Question,
}

impl NodeType {
    /// Every canonical graph record kind in stable contract order.
    pub const ALL: [Self; 6] = [
        Self::Source,
        Self::Requirement,
        Self::Resolution,
        Self::Rule,
        Self::Topic,
        Self::Question,
    ];

    /// The canonical wire word for this graph record kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Requirement => "requirement",
            Self::Resolution => "resolution",
            Self::Rule => "rule",
            Self::Topic => "topic",
            Self::Question => "question",
        }
    }

    /// The contract ordering of node kinds: results order by this rank and
    /// then by canonical id. The rank is declared, never derived from an
    /// `Ord` implementation.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Source => 0,
            Self::Requirement => 1,
            Self::Resolution => 2,
            Self::Rule => 3,
            Self::Topic => 4,
            Self::Question => 5,
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        parse_enum_word(value)
    }
}
