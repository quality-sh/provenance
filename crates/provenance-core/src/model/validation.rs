use provenance_macros::rule;
use serde::{de::Error, Deserialize, Deserializer};

/// Decides which confidence scores a record may carry.
///
/// A confidence is a number from 0.0 to 1.0 inclusive and nothing else: not a
/// NaN, not an infinity, and not 1.5 quietly brought back to 1.0 on the way
/// in. A score outside the range is a mistake by whoever wrote it, so the
/// graph refuses the record instead of guessing what was meant, because a
/// corrected score afterwards reads exactly like one somebody chose.
///
/// The scanner keeps its own copy of this range in
/// `provenance-scanner/src/parser.rs` (`confidence_in_range`), because it
/// reads confidences out of source comments before any record exists. The two
/// agree on the verdict and differ only in the remedy: the graph channel
/// rejects the record, while the annotation channel warns on the line and
/// falls back to the default 1.0, since one bad comment must not stop a scan
/// of the repository.
#[rule("rule_confidence_range")]
pub fn validate_confidence_score(confidence: f64) -> anyhow::Result<()> {
    anyhow::ensure!(
        confidence.is_finite() && (0.0..=1.0).contains(&confidence),
        "confidence must be between 0.0 and 1.0"
    );
    Ok(())
}

pub fn validate_optional_confidence_score(confidence: Option<f64>) -> anyhow::Result<Option<f64>> {
    match confidence {
        Some(confidence) => {
            validate_confidence_score(confidence)?;
            Ok(Some(confidence))
        }
        None => Ok(None),
    }
}

pub(super) fn deserialize_optional_confidence<'de, D>(
    deserializer: D,
) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let confidence = Option::<f64>::deserialize(deserializer)?;
    validate_optional_confidence_score(confidence).map_err(D::Error::custom)
}

/// Decides which commit pins a source may carry.
///
/// A source pin is 7 to 64 hexadecimal digits in either case: the shapes a
/// person actually has to hand from `git log`, a review comment, or a pasted
/// permalink, short forms and shouted uppercase included. A source pin
/// locates: it tells a reader where to go and look, and an abbreviation does
/// that.
#[rule("rule_source_commit_pin")]
pub fn validate_commit_pin(commit_pin: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        (7..=64).contains(&commit_pin.len())
            && commit_pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "commit pin must be a 7-64 character hexadecimal Git commit"
    );
    Ok(())
}

pub fn validate_optional_commit_pin(commit_pin: Option<String>) -> anyhow::Result<Option<String>> {
    match commit_pin {
        Some(commit_pin) => {
            validate_commit_pin(&commit_pin)?;
            Ok(Some(commit_pin))
        }
        None => Ok(None),
    }
}

pub(super) fn deserialize_optional_commit_pin<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let commit_pin = Option::<String>::deserialize(deserializer)?;
    validate_optional_commit_pin(commit_pin).map_err(D::Error::custom)
}

/// Decides what a resolution input must say.
///
/// An input that names nothing locates nothing: the reference says where the
/// input came from and the summary says what it told the decision, so a blank
/// field is refused rather than repaired, because a reference filled in later
/// reads exactly like one that was there when the decision was made.
///
/// Blank means empty or nothing but whitespace. The reference is not resolved:
/// it may name a record in the graph, a file in this repository, a clause of
/// legislation, or a conversation with a person the graph will never see. This
/// rule decides that something was named, not that the thing named exists.
#[rule("rule_resolution_input_content")]
pub fn validate_resolution_input_content(reference: &str, summary: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !reference.trim().is_empty(),
        "resolution input reference must not be blank"
    );
    anyhow::ensure!(
        !summary.trim().is_empty(),
        "resolution input summary must not be blank"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use provenance_macros::verifies;

    use super::{
        validate_commit_pin, validate_confidence_score, validate_resolution_input_content,
    };

    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut word = self.0;
            word = (word ^ (word >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            word = (word ^ (word >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            word ^ (word >> 31)
        }

        fn below(&mut self, bound: usize) -> usize {
            usize::try_from(self.next_u64() % bound as u64).unwrap()
        }
    }

    /// Every character a hexadecimal digit can be, written out rather than
    /// derived, so the property has something of its own to hold
    /// `is_ascii_hexdigit` to.
    const HEX_DIGITS: &str = "0123456789abcdefABCDEF";

    /// Characters a pin must not hold: letters just past `f`, the punctuation
    /// a branch name or a `git describe` output carries, whitespace, and
    /// non-ASCII text.
    const NON_HEX: [&str; 14] = [
        "g", "G", "z", "-", "_", "/", ":", " ", "\t", "+", ".", "é", "中", "\u{200b}",
    ];

    /// The pin shape restated on its own terms: count the characters, and
    /// check each one appears in the written-out alphabet.
    fn pin_is_wellformed(pin: &str) -> bool {
        let mut digits = 0_usize;
        for character in pin.chars() {
            if !HEX_DIGITS.contains(character) {
                return false;
            }
            digits += 1;
        }
        matches!(digits, 7..=64)
    }

    /// Pins around every length that matters, in mixed case, one in four
    /// carrying a character that is not a hexadecimal digit somewhere in it,
    /// the ends included.
    fn gen_pin(rng: &mut Rng) -> String {
        const LENGTHS: [usize; 11] = [0, 1, 6, 7, 8, 20, 39, 40, 63, 64, 65];

        let alphabet: Vec<char> = HEX_DIGITS.chars().collect();
        let length = LENGTHS[rng.below(LENGTHS.len())];
        let mut pin: String = (0..length)
            .map(|_| alphabet[rng.below(alphabet.len())])
            .collect();
        if rng.below(4) == 0 {
            let at = rng.below(pin.chars().count() + 1);
            let mut spoiled: String = pin.chars().take(at).collect();
            spoiled.push_str(NON_HEX[rng.below(NON_HEX.len())]);
            spoiled.extend(pin.chars().skip(at));
            pin = spoiled;
        }
        pin
    }

    #[test]
    #[verifies("rule_source_commit_pin", property)]
    fn accepts_exactly_the_strings_of_7_to_64_hexadecimal_digits() {
        let mut rng = Rng(0x5eed_0001);
        let mut accepted = 0;
        let mut refused = 0;
        for _ in 0..4096 {
            let pin = gen_pin(&mut rng);
            let wellformed = pin_is_wellformed(&pin);

            assert_eq!(
                validate_commit_pin(&pin).is_ok(),
                wellformed,
                "pin {pin:?} of {} characters got the wrong verdict",
                pin.chars().count()
            );
            if wellformed {
                accepted += 1;
            } else {
                refused += 1;
            }
        }
        assert!(
            accepted >= 512 && refused >= 512,
            "{accepted} accepted and {refused} refused; the property proves too little"
        );
    }

    #[test]
    #[verifies("rule_source_commit_pin", conformance)]
    fn source_commit_pin_contract_keeps_abbreviations_and_uppercase() {
        for (name, pin, accepted) in [
            ("six lowercase digits", "a".repeat(6), false),
            ("seven lowercase digits", "a".repeat(7), true),
            ("seven uppercase digits", "A".repeat(7), true),
            ("mixed case abbreviation", "aBcD012".to_string(), true),
            ("forty uppercase digits", "F".repeat(40), true),
            ("sixty-four lowercase digits", "0".repeat(64), true),
            ("sixty-five lowercase digits", "0".repeat(65), false),
            ("non-hexadecimal text", "commit0".to_string(), false),
        ] {
            assert_eq!(
                validate_commit_pin(&pin).is_ok(),
                accepted,
                "source pin contract got the wrong verdict for {name}: {pin:?}"
            );
        }
    }

    /// The score range restated on its own terms: a NaN compares to nothing,
    /// so it has no ordering against either end and is out; everything else
    /// must sit at or above 0.0 and at or below 1.0, which puts both
    /// infinities out too.
    fn score_in_range(confidence: f64) -> bool {
        matches!(
            confidence.partial_cmp(&0.0),
            Some(Ordering::Greater | Ordering::Equal)
        ) && matches!(
            confidence.partial_cmp(&1.0),
            Some(Ordering::Less | Ordering::Equal)
        )
    }

    /// Scores from three sources: the values a fence post gets wrong, a spread
    /// that straddles both ends, and whole random bit patterns, which is where
    /// the NaNs and the enormous numbers come from.
    fn gen_score(rng: &mut Rng) -> f64 {
        const FIXED: [f64; 14] = [
            0.0,
            -0.0,
            1.0,
            0.5,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::EPSILON,
            1.0 - f64::EPSILON,
            1.0 + f64::EPSILON,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
        ];
        const SPREAD: usize = 1 << 20;

        match rng.below(3) {
            0 => FIXED[rng.below(FIXED.len())],
            1 => {
                let step = u32::try_from(rng.below(SPREAD)).unwrap();
                let width = u32::try_from(SPREAD).unwrap();
                (f64::from(step) / f64::from(width)).mul_add(1.5, -0.25)
            }
            _ => f64::from_bits(rng.next_u64()),
        }
    }

    #[test]
    #[verifies("rule_confidence_range", property)]
    fn accepts_exactly_the_scores_from_zero_to_one() {
        let mut rng = Rng(0x5eed_0002);
        let mut accepted = 0;
        let mut refused = 0;
        for _ in 0..4096 {
            let confidence = gen_score(&mut rng);
            let in_range = score_in_range(confidence);

            assert_eq!(
                validate_confidence_score(confidence).is_ok(),
                in_range,
                "score {confidence:?} got the wrong verdict"
            );
            if in_range {
                accepted += 1;
            } else {
                refused += 1;
            }
        }
        assert!(
            accepted >= 256 && refused >= 256,
            "{accepted} accepted and {refused} refused; the property proves too little"
        );
    }

    /// Every kind of field content the rule sorts on: nothing at all, the
    /// several shapes of whitespace that look like text in a terminal, and
    /// something a reader can actually follow. Each field is drawn from this
    /// list independently, so the pairs cover both fields blank, either one
    /// blank, and neither.
    const FIELD_CONTENTS: [&str; 7] = [
        "",
        " ",
        "   ",
        "\t",
        "\n",
        " \t\r\n ",
        "docs/shaping.md section 3",
    ];

    fn is_blank(content: &str) -> bool {
        content.chars().all(char::is_whitespace)
    }

    #[test]
    #[verifies("rule_resolution_input_content", exhaustion)]
    fn accepts_exactly_the_inputs_whose_reference_and_summary_both_say_something() {
        for reference in FIELD_CONTENTS {
            for summary in FIELD_CONTENTS {
                assert_eq!(
                    validate_resolution_input_content(reference, summary).is_ok(),
                    !is_blank(reference) && !is_blank(summary),
                    "reference {reference:?} and summary {summary:?} got the wrong verdict"
                );
            }
        }
    }

    #[test]
    #[verifies("rule_resolution_input_content", examples)]
    fn a_blank_field_is_named_in_the_refusal() {
        assert_eq!(
            validate_resolution_input_content("  ", "Program rules reviewed")
                .unwrap_err()
                .to_string(),
            "resolution input reference must not be blank"
        );
        assert_eq!(
            validate_resolution_input_content("SAH program manual", "")
                .unwrap_err()
                .to_string(),
            "resolution input summary must not be blank"
        );
    }

    /// The rule decides that something was named, not that it exists. A
    /// reference to a page of legislation nobody can open from here is a good
    /// input, and this pins that it stays one.
    #[test]
    #[verifies("rule_resolution_input_content", examples)]
    fn a_reference_to_the_outside_world_is_accepted_unresolved() {
        assert!(validate_resolution_input_content(
            "Aged Care Act 2024, schedule 2",
            "Sets the fee cap we chose to enforce"
        )
        .is_ok());
    }
}
