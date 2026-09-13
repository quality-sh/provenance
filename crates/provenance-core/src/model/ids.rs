use provenance_macros::{rule, verifies};
use serde::{de::Error, Deserialize, Deserializer, Serialize};

/// Every id in the graph is non-empty and uses only lowercase ASCII letters,
/// digits, `_` or `-`.
///
/// This is the only statement of the character set in the runtime: both id
/// constructors and both `Deserialize` impls call it, so there is no second
/// copy to drift. The four JSON Schema `pattern` strings published by
/// `provenance schema show` are replicas of it.
///
/// The rule holds by construction, not by discipline. `StableId` and `ScopeId`
/// wrap a private `String` in a module that contains nothing else, so no code
/// outside this file can build one without going through `new`, and `new` is
/// the only constructor: there is no `From`, no `FromStr`, no `Default`, no
/// public field, and no derived `Deserialize` that would skip the check. An
/// id that breaks the rule is unrepresentable as a `StableId` or a `ScopeId`.
#[rule("rule_id_charset")]
fn is_well_formed_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub u32);

/// A scope id. The inner `String` is private and `new` is the only way in, so
/// every `ScopeId` in existence satisfies [`is_well_formed_id`].
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
#[cfg_attr(feature = "schema", schemars(extend("pattern" = "^[a-z0-9_-]+$")))]
#[verifies("rule_id_charset", construction)]
pub struct ScopeId(String);

impl ScopeId {
    pub fn new(value: impl Into<String>) -> anyhow::Result<Self> {
        let value = value.into();
        if !is_well_formed_id(&value) {
            anyhow::bail!("scope id must use lowercase ASCII letters, digits, '_' or '-'");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ScopeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value.clone()).map_err(|err| {
            D::Error::custom(format!(
                "scope id '{value}' is invalid: {err}; repair hint: correct this value where it is stored (state shard or input JSON)"
            ))
        })
    }
}

/// A stable artifact id. The inner `String` is private and `new` is the only
/// way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
#[cfg_attr(feature = "schema", schemars(extend("pattern" = "^[a-z0-9_-]+$")))]
#[verifies("rule_id_charset", construction)]
pub struct StableId(String);

impl StableId {
    pub fn new(value: impl Into<String>) -> anyhow::Result<Self> {
        let value = value.into();
        if !is_well_formed_id(&value) {
            anyhow::bail!("stable id must use lowercase ASCII letters, digits, '_' or '-'");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for StableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value.clone()).map_err(|err| {
            D::Error::custom(format!(
                "stable id '{value}' is invalid: {err}; repair hint: correct this value where it is stored (state shard or input JSON)"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{is_well_formed_id, ScopeId, StableId};
    use provenance_macros::verifies;

    /// The character set restated as data, independently of the predicate:
    /// the rule says these characters and no others.
    const ALLOWED: &str = "abcdefghijklmnopqrstuvwxyz0123456789_-";

    /// Characters the rule excludes: uppercase, space, punctuation, control,
    /// and non-ASCII.
    const FORBIDDEN: [char; 16] = [
        'A', 'Z', 'Q', ' ', '\t', '\n', '.', '/', '\\', ':', '+', '@', '*', '\'', 'é', '漢',
    ];

    /// Independent oracle. Says what the rule says without reusing any part of
    /// how the rule says it (no `is_ascii_lowercase`, no character ranges).
    fn oracle_accepts(value: &str) -> bool {
        !value.is_empty() && value.chars().all(|ch| ALLOWED.contains(ch))
    }

    /// Deterministic generator so a failure is reproducible from the seed.
    struct Generator(u64);

    impl Generator {
        fn draw(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }

        /// Strings over the allowed characters plus the forbidden ones, so the
        /// stream contains well-formed ids, near misses, and outright junk.
        fn string(&mut self) -> String {
            let pool: Vec<char> = ALLOWED.chars().chain(FORBIDDEN).collect();
            let length = usize::try_from(self.draw() % 13).unwrap();
            (0..length)
                .map(|_| pool[usize::try_from(self.draw()).unwrap() % pool.len()])
                .collect()
        }
    }

    #[test]
    #[verifies("rule_id_charset", property)]
    fn generated_strings_match_the_independent_character_set_oracle() {
        let mut generator = Generator(0x5eed_1234_abcd_0001);
        for _ in 0..4000 {
            let candidate = generator.string();
            assert_eq!(
                is_well_formed_id(&candidate),
                oracle_accepts(&candidate),
                "rule and oracle disagree on {candidate:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_id_charset", property)]
    fn stable_id_and_scope_id_agree_on_every_generated_string() {
        let mut generator = Generator(0x5eed_1234_abcd_0002);
        for _ in 0..4000 {
            let candidate = generator.string();
            let stable = StableId::new(candidate.clone()).is_ok();
            let scope = ScopeId::new(candidate.clone()).is_ok();
            assert_eq!(
                stable, scope,
                "StableId and ScopeId disagree on {candidate:?}: \
                 StableId {stable}, ScopeId {scope}"
            );
            assert_eq!(
                stable,
                oracle_accepts(&candidate),
                "constructor and oracle disagree on {candidate:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_id_charset", property)]
    fn one_forbidden_character_anywhere_refuses_the_whole_string() {
        let mut generator = Generator(0x5eed_1234_abcd_0003);
        for _ in 0..2000 {
            let base = generator.string();
            let base: String = base.chars().filter(|ch| ALLOWED.contains(*ch)).collect();
            let base = if base.is_empty() {
                "a".to_string()
            } else {
                base
            };
            assert!(StableId::new(base.clone()).is_ok(), "rejected {base:?}");

            for bad in FORBIDDEN {
                let position = usize::try_from(generator.draw()).unwrap() % (base.len() + 1);
                let mut spoiled = base.clone();
                spoiled.insert(position, bad);
                assert!(
                    StableId::new(spoiled.clone()).is_err(),
                    "accepted {spoiled:?}, which carries {bad:?}"
                );
                assert!(
                    ScopeId::new(spoiled.clone()).is_err(),
                    "accepted {spoiled:?}, which carries {bad:?}"
                );
            }
        }
    }

    #[test]
    #[verifies("rule_id_charset", property)]
    fn the_empty_string_is_never_an_id() {
        assert!(!is_well_formed_id(""));
        assert!(StableId::new("").is_err());
        assert!(ScopeId::new("").is_err());
    }

    // The construction claim lives on the two types above, whose private
    // interior and lone constructor make a violating id unrepresentable. The
    // tests below sample the routes in by hand or with generated strings, so
    // each carries the method its body supports.

    #[test]
    #[verifies("rule_id_charset", examples)]
    fn the_constructor_is_the_only_public_route_in() {
        assert!(StableId::new("source_codebase").is_ok());
        assert!(StableId::new("source/codebase").is_err());
        assert!(ScopeId::new("default").is_ok());
        assert!(ScopeId::new("Default").is_err());
    }

    #[test]
    #[verifies("rule_id_charset", property)]
    fn deserialization_runs_the_same_predicate_as_the_constructor() {
        let mut generator = Generator(0x5eed_1234_abcd_0004);
        for _ in 0..2000 {
            let candidate = generator.string();
            let json = serde_json::to_string(&candidate).unwrap();
            assert_eq!(
                serde_json::from_str::<StableId>(&json).is_ok(),
                StableId::new(candidate.clone()).is_ok(),
                "serde and constructor disagree on {candidate:?}"
            );
            assert_eq!(
                serde_json::from_str::<ScopeId>(&json).is_ok(),
                ScopeId::new(candidate.clone()).is_ok(),
                "serde and constructor disagree on {candidate:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_id_charset", examples)]
    fn a_serialized_id_deserializes_back_into_an_id() {
        for value in ["a", "0", "_", "-", "a-b_c9", &"z".repeat(4096)] {
            let id = StableId::new(value).unwrap();
            let round_tripped: StableId =
                serde_json::from_str(&serde_json::to_string(&id).unwrap()).unwrap();
            assert_eq!(round_tripped, id);
            assert!(is_well_formed_id(id.as_str()));
        }
    }

    #[test]
    fn stable_id_deserialize_error_names_invalid_value_and_repair_hint() {
        let err = serde_json::from_str::<StableId>(r#""source/codebase""#).unwrap_err();
        let message = err.to_string();

        assert!(message.contains("source/codebase"));
        assert!(message.contains("stable id"));
        assert!(message.contains("correct this value where it is stored"));
        assert!(message.contains("state shard or input JSON"));
    }

    #[test]
    fn scope_id_deserialize_error_names_invalid_value_and_repair_hint() {
        let err = serde_json::from_str::<ScopeId>(r#""Default""#).unwrap_err();
        let message = err.to_string();

        assert!(message.contains("Default"));
        assert!(message.contains("scope id"));
        assert!(message.contains("correct this value where it is stored"));
        assert!(message.contains("state shard or input JSON"));
    }
}
