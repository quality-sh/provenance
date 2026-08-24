//! String-keyed, address-bearing handles over a built document.
//!
//! Compile-time key safety is a frontend concern: language sugar wraps
//! these handles, the kernel stays string-keyed.

use super::addresses::{requirement_address, rule_address, source_address};
use super::SpecDocument;
use crate::model::DeclarationAddress;

pub struct SpecHandles<'a> {
    document: &'a SpecDocument,
}

impl SpecDocument {
    pub const fn handles(&self) -> SpecHandles<'_> {
        SpecHandles { document: self }
    }
}

impl<'a> SpecHandles<'a> {
    pub fn source(&self, key: &str) -> anyhow::Result<SourceHandle> {
        anyhow::ensure!(
            self.document.sources.iter().any(|source| source.key == key),
            "spec `{}` does not declare source `{key}`",
            self.document.spec
        );
        Ok(SourceHandle {
            address: source_address(&self.document.spec, key)?,
        })
    }

    pub fn requirement(&self, key: &str) -> anyhow::Result<RequirementHandle<'a>> {
        anyhow::ensure!(
            self.document
                .requirements
                .iter()
                .any(|requirement| requirement.key == key),
            "spec `{}` does not declare requirement `{key}`",
            self.document.spec
        );
        Ok(RequirementHandle {
            document: self.document,
            address: requirement_address(&self.document.spec, key)?,
            key: key.to_string(),
        })
    }
}

pub struct SourceHandle {
    pub address: DeclarationAddress,
}

pub struct RequirementHandle<'a> {
    document: &'a SpecDocument,
    pub address: DeclarationAddress,
    key: String,
}

impl RequirementHandle<'_> {
    /// Resolves a rule this requirement owns or shares.
    pub fn rule(&self, key: &str) -> anyhow::Result<RuleHandle> {
        let rule = self
            .document
            .rules
            .iter()
            .find(|rule| {
                rule.key == key && rule.requirements.iter().any(|owner| owner == &self.key)
            })
            .ok_or_else(|| {
                anyhow::anyhow!("requirement `{}` does not own rule `{key}`", self.key)
            })?;
        Ok(RuleHandle {
            address: match &rule.address {
                Some(address) => address.clone(),
                None => rule_address(&self.document.spec, rule)?,
            },
        })
    }
}

pub struct RuleHandle {
    pub address: DeclarationAddress,
}
