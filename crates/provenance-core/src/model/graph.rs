use serde::{Deserialize, Serialize};

use super::parsing::normalize_enum_value;

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
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "boundary")]
    Boundary,
}

impl NodeType {
    /// Every canonical graph record kind in stable contract order.
    pub const ALL: [Self; 8] = [
        Self::Source,
        Self::Requirement,
        Self::Resolution,
        Self::Rule,
        Self::Topic,
        Self::Question,
        Self::Domain,
        Self::Boundary,
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
            Self::Domain => "domain",
            Self::Boundary => "boundary",
        }
    }

    /// The contract ordering of node kinds: results order by this rank and
    /// then by canonical id. The rank is declared, never derived from an
    /// `Ord` implementation, and the two newest kinds append after the six
    /// settled positions so existing page boundaries stand still.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Source => 0,
            Self::Requirement => 1,
            Self::Resolution => 2,
            Self::Rule => 3,
            Self::Topic => 4,
            Self::Question => 5,
            Self::Domain => 6,
            Self::Boundary => 7,
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "source" => Ok(Self::Source),
            "requirement" => Ok(Self::Requirement),
            "resolution" => Ok(Self::Resolution),
            "rule" => Ok(Self::Rule),
            "topic" => Ok(Self::Topic),
            "question" => Ok(Self::Question),
            "domain" => Ok(Self::Domain),
            "boundary" => Ok(Self::Boundary),
            _ => anyhow::bail!(
                "parent type must be source, requirement, resolution, rule, topic, question, \
                 domain, or boundary"
            ),
        }
    }
}
