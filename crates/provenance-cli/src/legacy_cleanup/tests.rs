use std::fmt::Write as _;

use provenance_macros::verifies;

use super::*;

/// `SplitMix64`. The seeds below are fixed, so every run walks the same
/// generated files and a failure is reproducible from the test name.
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

    fn pick(&mut self, choices: &[&'static str]) -> &'static str {
        choices[self.below(choices.len())]
    }
}

const FRONTMATTER_VALUES: &[&str] = &[
    "old",
    "a name",
    "0.1.0",
    "with - dash",
    "unicode é",
    "日本語",
    "<!-- not a header -->",
    "trailing space ",
    "",
];

const ASCII_FRONTMATTER_VALUES: &[&str] = &[
    "old",
    "a name",
    "0.1.0",
    "with - dash",
    "<!-- not a header -->",
    "trailing space ",
    "",
];

// Payload chunks worth stressing: blank lines, frontmatter delimiters
// loose in the body, multibyte characters that move every byte offset
// after them, and a decoy header line the parser must ignore because it
// is not the one sitting right after the frontmatter.
const PAYLOAD_CHUNKS: &[&str] = &[
    "a",
    " ",
    "\n",
    "---\n",
    "\n---\n",
    "# Heading\n",
    "body text ",
    "é",
    "日本語\n",
    "<!--",
    "-->",
    "\r\n",
    "<!-- Installed by provenance 9.9.9; content hash fnv1a64:0000000000000000 -->\n",
];

const ASCII_CHUNKS: &[&str] = &[
    "a",
    " ",
    "\n",
    "---\n",
    "# Heading\n",
    "body text ",
    "<!--",
    "-->",
];

fn gen_frontmatter(rng: &mut Rng, values: &[&'static str]) -> String {
    let mut text = String::from("---\n");
    for index in 0..=rng.below(3) {
        let value = rng.pick(values);
        writeln!(text, "key{index}: {value}").unwrap();
    }
    text.push_str("---\n");
    text
}

fn gen_payload(rng: &mut Rng, chunks: &[&'static str], min_bytes: usize) -> String {
    let mut text = String::new();
    while text.len() < min_bytes {
        text.push_str(rng.pick(chunks));
    }
    text
}

fn gen_version(rng: &mut Rng) -> String {
    format!("{}.{}.{}", rng.below(10), rng.below(30), rng.below(100))
}

/// Independent restatement of the stamp the installer writes, kept apart
/// from the parser's own constants: one header line carrying the hash of
/// the file as installed, placed straight after the frontmatter.
fn header_line(version: &str, installed: &str) -> String {
    format!(
        "<!-- Installed by provenance {version}; content hash fnv1a64:{} -->",
        fnv1a64(installed)
    )
}

fn stamp(frontmatter: &str, version: &str, payload: &str) -> String {
    let header = header_line(version, &format!("{frontmatter}{payload}"));
    format!("{frontmatter}{header}\n{payload}")
}

#[test]
#[verifies("rule_legacy_cleanup_ownership", property)]
fn accepts_every_correctly_stamped_file() {
    let mut rng = Rng(0x5eed_0001);
    for _ in 0..2048 {
        let frontmatter = gen_frontmatter(&mut rng, FRONTMATTER_VALUES);
        let payload = gen_payload(&mut rng, PAYLOAD_CHUNKS, 64);
        let contents = stamp(&frontmatter, &gen_version(&mut rng), &payload);

        assert!(
            valid_managed_skill(&contents),
            "refused to own a file stamped exactly as installed: {contents:?}"
        );
    }
}

#[test]
#[verifies("rule_legacy_cleanup_ownership", property)]
fn rejects_every_single_byte_edit_to_the_installed_bytes() {
    let mut rng = Rng(0x5eed_0002);
    for _ in 0..24 {
        let frontmatter = gen_frontmatter(&mut rng, ASCII_FRONTMATTER_VALUES);
        let payload = gen_payload(&mut rng, ASCII_CHUNKS, 40);
        let contents = stamp(&frontmatter, &gen_version(&mut rng), &payload);
        // Everything the hash covers: the frontmatter and the payload,
        // which is the whole file bar the header line the installer added.
        let payload_start = contents.len() - payload.len();
        let installed = (0..frontmatter.len()).chain(payload_start..contents.len());

        for offset in installed {
            let original = contents.as_bytes()[offset];
            for replacement in 0..=0x7f_u8 {
                if replacement == original {
                    continue;
                }
                let mut bytes = contents.as_bytes().to_vec();
                bytes[offset] = replacement;
                let edited = String::from_utf8(bytes).unwrap();

                assert!(
                    !valid_managed_skill(&edited),
                    "claimed a file edited at byte {offset}: {edited:?}"
                );
            }
        }
    }
}

#[test]
#[verifies("rule_legacy_cleanup_ownership", property)]
fn rejects_every_header_off_its_required_placement() {
    let mut rng = Rng(0x5eed_0003);
    for _ in 0..1024 {
        let frontmatter = gen_frontmatter(&mut rng, FRONTMATTER_VALUES);
        let payload = gen_payload(&mut rng, PAYLOAD_CHUNKS, 64);
        let header = header_line(&gen_version(&mut rng), &format!("{frontmatter}{payload}"));
        let body = frontmatter.strip_prefix("---\n").unwrap();
        let misplaced = [
            (
                "above the frontmatter",
                format!("{header}\n{frontmatter}{payload}"),
            ),
            (
                "inside the frontmatter",
                format!("---\n{header}\n{body}{payload}"),
            ),
            (
                "one line late",
                format!("{frontmatter}decoy line\n{header}\n{payload}"),
            ),
            (
                "below the payload",
                format!("{frontmatter}{payload}{header}\n"),
            ),
            ("absent", format!("{frontmatter}{payload}")),
        ];

        for (placement, contents) in misplaced {
            assert!(
                !valid_managed_skill(&contents),
                "claimed a file with its header {placement}: {contents:?}"
            );
        }
    }
}

#[test]
fn agents_without_an_owned_block_are_unchanged() {
    let contents = b"# User instructions\n\xff";
    assert_eq!(project_agents(contents), contents);
}
