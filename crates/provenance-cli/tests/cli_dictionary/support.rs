use assert_cmd::Command;
use pdf_oxide::writer::PdfWriter;
use provenance_ste100::DictionaryImport;
use std::{
    path::{Path, PathBuf},
    process::Output,
    sync::OnceLock,
};

const STATED_APPROVED_WORDS: usize = 875;
const STATED_UNAPPROVED_WORDS: usize = 1_274;
pub const APPROVED_TABLE_ROWS: usize = 878;
pub const UNAPPROVED_TABLE_ROWS: usize = 1_318;

/// One unapproved headword from the synthetic fixture. The word holds no digit
/// and no underscore, so the vocabulary check treats it as prose.
pub const UNAPPROVED_WORD: &str = "bunapprovedaaa";

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn init(repo: &Path) {
    provenance()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

/// Creates one Requirement with the index directory set on the child process.
pub fn create_requirement(
    repo: &Path,
    index_directory: &Path,
    id: &str,
    statement: &str,
) -> Output {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--statement",
            statement,
        ])
        .output()
        .unwrap()
}

pub fn reference_path(repo: &Path) -> PathBuf {
    repo.join(".provenance")
        .join("state")
        .join("dictionary.json")
}

/// Writes the project dictionary reference without touching the environment.
pub fn write_reference(repo: &Path, dictionary: &DictionaryImport) {
    let path = reference_path(repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&dictionary.identity).unwrap(),
    )
    .unwrap();
}

pub fn error_json(output: &Output) -> serde_json::Value {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    serde_json::from_str::<serde_json::Value>(
        stderr
            .trim()
            .strip_prefix("Error: ")
            .expect("CLI failure contains one machine-readable JSON object"),
    )
    .unwrap()["error"]
        .clone()
}

/// The synthetic Issue 9 PDF. The builder makes every word, so no dictionary
/// content from the standard is present.
pub fn dictionary_pdf() -> &'static [u8] {
    static PDF: OnceLock<Vec<u8>> = OnceLock::new();
    PDF.get_or_init(|| build_pdf(APPROVED_TABLE_ROWS, UNAPPROVED_TABLE_ROWS))
}

pub fn imported_dictionary() -> &'static DictionaryImport {
    static DICTIONARY: OnceLock<DictionaryImport> = OnceLock::new();
    DICTIONARY.get_or_init(|| {
        provenance_ste100::import_dictionary(dictionary_pdf())
            .expect("the synthetic dictionary must import")
    })
}

struct FixtureEntry {
    word: String,
    meaning: String,
    ste: String,
    non_ste: String,
}

/// Maps an index to three letters, so no headword holds a digit.
fn alpha_suffix(index: usize) -> String {
    const LETTERS: &[u8; 26] = b"abcdefghijklmnopqrstuvwxyz";
    let mut suffix = [b'a'; 3];
    let mut value = index;
    for slot in suffix.iter_mut().rev() {
        *slot = LETTERS[value % 26];
        value /= 26;
    }
    String::from_utf8(suffix.to_vec()).expect("the suffix holds ASCII letters")
}

fn approved_entry(index: usize) -> FixtureEntry {
    FixtureEntry {
        word: format!("AAPPROVED{} (n)", alpha_suffix(index).to_uppercase()),
        meaning: "A meaning".to_owned(),
        ste: "USE THE ITEM.".to_owned(),
        non_ste: String::new(),
    }
}

fn unapproved_entry(index: usize) -> FixtureEntry {
    let headword = format!("bunapproved{}", alpha_suffix(index));
    FixtureEntry {
        word: format!("{headword} (n)"),
        meaning: "ITEM (n)".to_owned(),
        ste: "USE THE ITEM.".to_owned(),
        non_ste: format!("Use the {headword} item."),
    }
}

fn build_pdf(approved: usize, unapproved: usize) -> Vec<u8> {
    let mut writer = PdfWriter::new();
    let entries = (0..approved)
        .map(approved_entry)
        .chain((0..unapproved).map(unapproved_entry));
    let mut entries = entries.peekable();
    let mut page_number = 1;

    while entries.peek().is_some() {
        let mut page = writer.add_page(612.0, 792.0);
        page.add_text(
            "ASD-STE100 Simplified Technical English",
            72.0,
            770.0,
            "Helvetica",
            9.0,
        )
        .add_text("Word", 72.0, 745.0, "Helvetica", 8.0)
        .add_text("Approved", 180.0, 745.0, "Helvetica", 8.0)
        .add_text("STE", 310.0, 745.0, "Helvetica", 8.0)
        .add_text("Non-STE", 440.0, 745.0, "Helvetica", 8.0)
        .add_text(
            &format!("Page 2-1-X{page_number}"),
            72.0,
            20.0,
            "Helvetica",
            8.0,
        )
        .add_text("Issue 9", 500.0, 20.0, "Helvetica", 8.0)
        .add_text(
            &format!("{STATED_APPROVED_WORDS} approved words {STATED_UNAPPROVED_WORDS} words"),
            250.0,
            760.0,
            "Helvetica",
            7.0,
        );

        for row in 0u8..35 {
            let Some(entry) = entries.next() else {
                break;
            };
            let y = f32::from(row).mul_add(-19.0, 725.0);
            page.add_text(&entry.word, 72.0, y, "Helvetica-Bold", 8.0)
                .add_text(&entry.non_ste, 440.0, y, "Helvetica", 8.0)
                .add_text(&entry.meaning, 164.0, y, "Helvetica", 8.0)
                .add_text(&entry.ste, 310.0, y, "Helvetica", 8.0);
        }

        page_number += 1;
    }

    writer.finish().expect("build the synthetic PDF")
}
