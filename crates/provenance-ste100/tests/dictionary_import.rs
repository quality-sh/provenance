use pdf_oxide::writer::PdfWriter;
use provenance_macros::verifies;
use provenance_ste100::{
    import_dictionary, load_dictionary_index_for_source, store_dictionary_index,
    DictionaryImportError, DictionaryIndexError, DictionaryStatus, PartOfSpeech, StandardIssue,
};

const STATED_APPROVED_WORDS: usize = 875;
const STATED_UNAPPROVED_WORDS: usize = 1_274;
const APPROVED_TABLE_ROWS: usize = 878;
const UNAPPROVED_TABLE_ROWS: usize = 1_318;

#[test]
#[verifies("rule_ste_dictionary_pdf_validation", examples)]
fn rejects_input_that_is_not_a_pdf() {
    let error = import_dictionary(b"not a PDF").expect_err("invalid input must fail closed");

    assert!(matches!(error, DictionaryImportError::InvalidPdf { .. }));
}

#[test]
#[verifies("rule_ste_dictionary_pdf_validation", examples)]
fn rejects_a_pdf_without_issue_9_identity() {
    let pdf = dictionary_pdf(1, 0, 8, true);

    let error = import_dictionary(&pdf).expect_err("a different issue must fail closed");

    assert!(matches!(
        error,
        DictionaryImportError::UnsupportedDocument { .. }
    ));
}

#[test]
#[verifies("rule_ste_dictionary_pdf_validation", examples)]
fn rejects_a_pdf_without_the_issue_9_word_totals() {
    let pdf = dictionary_pdf(1, 0, 9, false);

    let error = import_dictionary(&pdf).expect_err("missing Issue 9 totals must fail closed");

    assert!(matches!(
        error,
        DictionaryImportError::UnsupportedDocument { .. }
    ));
}

#[test]
#[verifies("rule_ste_dictionary_structure_validation", examples)]
fn rejects_an_incomplete_dictionary() {
    let pdf = dictionary_pdf(1, 1, 9, true);

    let error = import_dictionary(&pdf).expect_err("an incomplete dictionary must fail closed");

    assert!(
        matches!(
            &error,
            DictionaryImportError::InvalidStructure {
                approved_rows: 1,
                unapproved_rows: 1,
                ..
            }
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
#[verifies("rule_ste_dictionary_structure_validation", examples)]
#[verifies("rule_ste_dictionary_import_identity", examples)]
fn imports_a_complete_positioned_dictionary_deterministically() {
    let pdf = dictionary_pdf(APPROVED_TABLE_ROWS, UNAPPROVED_TABLE_ROWS, 9, true);

    let first = import_dictionary(&pdf).expect("the complete dictionary must import");
    let second = import_dictionary(&pdf).expect("the same dictionary must import again");

    assert_eq!(first, second);
    assert_eq!(first.identity.issue, StandardIssue::Nine);
    assert_eq!(first.identity.source_sha256.len(), 64);
    assert_eq!(first.identity.data_sha256.len(), 64);
    assert!(!first.identity.extractor_version.is_empty());
    assert_eq!(
        first.entries.len(),
        APPROVED_TABLE_ROWS + UNAPPROVED_TABLE_ROWS
    );
    assert_eq!(
        status_count(&first.entries, DictionaryStatus::Approved),
        APPROVED_TABLE_ROWS
    );
    assert_eq!(first.entries[0].headword, "AA0");
    assert_eq!(first.entries[0].word_forms, ["AA0", "AAZ"]);
    assert_eq!(first.entries[1].headword, "AA1WRAPPED");
    assert_eq!(first.entries[2].headword, "AA2");
    assert_eq!(first.entries[2].word_forms, ["AA2", "by AA2 CHANCE"]);
    assert_eq!(first.entries[3].headword, "AA3 WORD PHRASE");
    assert_eq!(first.entries[4].headword, "AA4COUNTERWISE");
    assert_eq!(first.entries[4].part_of_speech, PartOfSpeech::Adverb);
    assert_eq!(
        first.entries[0].approved_meaning_or_alternatives,
        "A meaning"
    );
    assert_eq!(first.entries[0].ste_example, "USE THE ITEM.");
    assert_eq!(first.entries[0].non_ste_example, None);
    let first_unapproved = first
        .entries
        .iter()
        .find(|entry| entry.status == DictionaryStatus::Unapproved)
        .expect("the fixture has unapproved words");
    assert_eq!(
        first_unapproved.non_ste_example.as_deref(),
        Some("Use the bunapproved0000 item.")
    );
    assert_eq!(first_unapproved.ste_example, "USE THE ITEM.");
    let prefix_entry = first
        .entries
        .iter()
        .find(|entry| entry.part_of_speech == PartOfSpeech::Prefix)
        .expect("the fixture has a prefix entry");
    assert_eq!(prefix_entry.headword, "bprefix0001-");
    assert_eq!(prefix_entry.status, DictionaryStatus::Unapproved);
    assert_eq!(
        prefix_entry.approved_meaning_or_alternatives,
        "Use AGAIN (adv) with the basic word"
    );
    assert_eq!(prefix_entry.ste_example, "");
    assert_eq!(prefix_entry.non_ste_example, None);
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn a_verified_index_loads_for_the_same_source_bytes() {
    let pdf = dictionary_pdf(APPROVED_TABLE_ROWS, UNAPPROVED_TABLE_ROWS, 9, true);
    let import = import_dictionary(&pdf).expect("import the fixture");
    let directory = scratch_directory("matching-source");
    store_dictionary_index(&import, &directory).expect("store the index");

    let loaded =
        load_dictionary_index_for_source(&directory, &pdf).expect("load the matching index");

    assert_eq!(loaded, import);
    std::fs::remove_dir_all(directory).expect("remove the scratch directory");
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn changed_source_or_extractor_version_cannot_reuse_an_index() {
    let pdf = dictionary_pdf(APPROVED_TABLE_ROWS, UNAPPROVED_TABLE_ROWS, 9, true);
    let mut old_import = import_dictionary(&pdf).expect("import the fixture");
    let directory = scratch_directory("changed-source");
    old_import.identity.extractor_version.push_str("-old");
    store_dictionary_index(&old_import, &directory).expect("store the old index");

    assert!(matches!(
        load_dictionary_index_for_source(&directory, &pdf),
        Err(DictionaryIndexError::NotFound { .. })
    ));

    old_import.identity.extractor_version = env!("CARGO_PKG_VERSION").to_owned();
    store_dictionary_index(&old_import, &directory).expect("store the current index");
    let mut changed_pdf = pdf;
    changed_pdf.push(b' ');
    assert!(matches!(
        load_dictionary_index_for_source(&directory, &changed_pdf),
        Err(DictionaryIndexError::NotFound { .. })
    ));
    std::fs::remove_dir_all(directory).expect("remove the scratch directory");
}

fn scratch_directory(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "provenance-ste100-source-index-{name}-{}",
        std::process::id()
    ))
}

#[test]
#[ignore = "requires a local official ASD-STE100 Issue 9 PDF"]
#[verifies("rule_ste_dictionary_structure_validation", conformance)]
fn imports_the_local_official_issue_9_pdf() {
    let path = std::env::var("ASD_STE100_ISSUE9_PDF")
        .expect("set ASD_STE100_ISSUE9_PDF to the local official PDF");
    let pdf = std::fs::read(path).expect("read the local official PDF");

    let dictionary = import_dictionary(&pdf).expect("import the official Issue 9 dictionary");

    assert_eq!(dictionary.identity.issue, StandardIssue::Nine);
    assert_eq!(
        status_count(&dictionary.entries, DictionaryStatus::Approved),
        APPROVED_TABLE_ROWS
    );
    assert_eq!(
        status_count(&dictionary.entries, DictionaryStatus::Unapproved),
        UNAPPROVED_TABLE_ROWS
    );

    let index_directory = std::env::temp_dir().join(format!(
        "provenance-ste100-official-index-{}",
        std::process::id()
    ));
    provenance_ste100::store_dictionary_index(&dictionary, &index_directory)
        .expect("store the official import as an index");
    let reloaded = provenance_ste100::load_dictionary_index(&index_directory, &dictionary.identity)
        .expect("load the official index again");
    assert_eq!(reloaded, dictionary);
    std::fs::remove_dir_all(&index_directory).expect("remove the scratch directory");
}

fn status_count(entries: &[provenance_ste100::DictionaryEntry], status: DictionaryStatus) -> usize {
    entries
        .iter()
        .filter(|entry| entry.status == status)
        .count()
}

fn dictionary_pdf(
    approved: usize,
    unapproved: usize,
    issue: u8,
    include_word_totals: bool,
) -> Vec<u8> {
    let mut writer = PdfWriter::new();
    let entries = approved_entries(approved).chain(unapproved_entries(unapproved));
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
        .add_text(&format!("Issue {issue}"), 500.0, 20.0, "Helvetica", 8.0);
        if include_word_totals {
            page.add_text(
                &format!("{STATED_APPROVED_WORDS} approved words {STATED_UNAPPROVED_WORDS} words"),
                250.0,
                760.0,
                "Helvetica",
                7.0,
            );
        }

        for row in 0u8..35 {
            let Some(entry) = entries.next() else {
                break;
            };
            let y = f32::from(row).mul_add(-19.0, 725.0);
            let mut line_y = y;
            for line in &entry.word {
                page.add_text(line, 72.0, line_y, "Helvetica-Bold", 8.0);
                line_y -= 9.0;
            }
            page.add_text(&entry.non_ste, 440.0, y, "Helvetica", 8.0)
                .add_text(&entry.meaning, 164.0, y, "Helvetica", 8.0)
                .add_text(&entry.ste, 310.0, y, "Helvetica", 8.0);
        }

        page_number += 1;
    }

    writer.finish().expect("build the synthetic PDF")
}

struct FixtureEntry {
    word: Vec<String>,
    meaning: String,
    ste: String,
    non_ste: String,
}

fn approved_entries(count: usize) -> impl Iterator<Item = FixtureEntry> {
    (0..count).map(approved_entry)
}

fn approved_entry(index: usize) -> FixtureEntry {
    FixtureEntry {
        word: if index == 0 {
            vec!["AA0 (or AAZ) (n)".to_owned()]
        } else if index == 1 {
            vec!["AA1WRAP-".to_owned(), "PED (n)".to_owned()]
        } else if index == 2 {
            vec!["AA2".to_owned(), "(by AA2 CHANCE) (n)".to_owned()]
        } else if index == 3 {
            vec!["AA3 WORD".to_owned(), "PHRASE (n)".to_owned()]
        } else if index == 4 {
            vec!["AA4COUNTER".to_owned(), "WISE (adv)".to_owned()]
        } else {
            vec![format!("AAPPROVED{index:04} (n)")]
        },
        meaning: "A meaning".to_owned(),
        ste: "USE THE ITEM.".to_owned(),
        non_ste: String::new(),
    }
}

fn unapproved_entries(count: usize) -> impl Iterator<Item = FixtureEntry> {
    (0..count).map(unapproved_entry)
}

fn unapproved_entry(index: usize) -> FixtureEntry {
    if index == 1 {
        return FixtureEntry {
            word: vec!["bprefix0001- (prefix)".to_owned()],
            meaning: "Use AGAIN (adv) with the basic word".to_owned(),
            ste: String::new(),
            non_ste: String::new(),
        };
    }
    FixtureEntry {
        word: vec![format!("bunapproved{index:04} (n)")],
        meaning: "ITEM (n)".to_owned(),
        ste: "USE THE ITEM.".to_owned(),
        non_ste: format!("Use the bunapproved{index:04} item."),
    }
}
