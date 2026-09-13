//! Builds the synthetic ASD-STE100 Issue 9 dictionary PDF the simulator
//! serves to `init`.
//!
//! The page layout mirrors `tests/cli_dictionary/support.rs`: same furniture,
//! column positions, fonts, and entry counts, so the real importer accepts
//! it. The container itself is hand-written here (catalog, page tree, base-14
//! fonts, content streams, xref) because the writer that builds the test
//! fixture is a test-only dependency of this binary.

use std::fmt::Write as _;

/// Row counts must match what the importer validates against.
const APPROVED_ROWS: usize = 878;
const UNAPPROVED_ROWS: usize = 1318;
const STATED_APPROVED_WORDS: usize = 875;
const STATED_UNAPPROVED_WORDS: usize = 1_274;
const ROWS_PER_PAGE: usize = 35;

const FONT_REGULAR: &str = "F1";
const FONT_BOLD: &str = "F2";
/// Table geometry, in PDF points. All integer values keep the layout exact.
const FIRST_ROW_Y: u16 = 725;
const ROW_SPACING: u16 = 19;

/// Maps an index to three letters, so no headword holds a digit. Same
/// mapping as the test fixture, so entry order stays valid.
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

/// Returns the complete PDF for one simulated onboarding.
pub(super) fn dictionary_pdf() -> Vec<u8> {
    let page_count = (APPROVED_ROWS + UNAPPROVED_ROWS).div_ceil(ROWS_PER_PAGE);
    let pages: Vec<String> = (0..page_count).map(page_stream).collect();
    assemble(&pages)
}

/// Renders one page: furniture, then the rows this page carries.
fn page_stream(page: usize) -> String {
    let mut text = String::new();
    push_text(
        &mut text,
        "ASD-STE100 Simplified Technical English",
        72,
        770,
        FONT_REGULAR,
        9,
    );
    push_text(
        &mut text,
        &format!("{STATED_APPROVED_WORDS} approved words {STATED_UNAPPROVED_WORDS} words"),
        250,
        760,
        FONT_REGULAR,
        7,
    );
    push_text(&mut text, "Word", 72, 745, FONT_REGULAR, 8);
    push_text(&mut text, "Approved", 180, 745, FONT_REGULAR, 8);
    push_text(&mut text, "STE", 310, 745, FONT_REGULAR, 8);
    push_text(&mut text, "Non-STE", 440, 745, FONT_REGULAR, 8);
    push_text(
        &mut text,
        &format!("Page 2-1-X{}", page + 1),
        72,
        20,
        FONT_REGULAR,
        8,
    );
    push_text(&mut text, "Issue 9", 500, 20, FONT_REGULAR, 8);

    let total_rows = APPROVED_ROWS + UNAPPROVED_ROWS;
    let first_row = page * ROWS_PER_PAGE;
    let last_row = (first_row + ROWS_PER_PAGE).min(total_rows);
    let mut y = FIRST_ROW_Y;
    for index in first_row..last_row {
        if index < APPROVED_ROWS {
            let headword = format!("AAPPROVED{} (n)", alpha_suffix(index).to_uppercase());
            push_text(&mut text, &headword, 72, y, FONT_BOLD, 8);
            push_text(&mut text, "A meaning", 164, y, FONT_REGULAR, 8);
            push_text(&mut text, "USE THE ITEM.", 310, y, FONT_REGULAR, 8);
        } else {
            let stem = format!("bunapproved{}", alpha_suffix(index - APPROVED_ROWS));
            push_text(&mut text, &format!("{stem} (n)"), 72, y, FONT_BOLD, 8);
            push_text(&mut text, "ITEM (n)", 164, y, FONT_REGULAR, 8);
            push_text(&mut text, "USE THE ITEM.", 310, y, FONT_REGULAR, 8);
            push_text(
                &mut text,
                &format!("Use the {stem} item."),
                440,
                y,
                FONT_REGULAR,
                8,
            );
        }
        y -= ROW_SPACING;
    }
    text
}

fn push_text(out: &mut String, text: &str, x: u16, y: u16, font: &str, size: u8) {
    let _ = writeln!(
        out,
        "BT /{font} {size} Tf {x} {y} Td ({}) Tj ET",
        escape(text)
    );
}

/// Escapes the three bytes a PDF literal string treats as syntax.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '\\' | '(' | ')') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// Assembles objects, xref, and trailer. Object numbering: 1 catalog,
/// 2 page tree, 3 and 4 the two fonts, then page and content pairs.
fn assemble(pages: &[String]) -> Vec<u8> {
    let page_count = pages.len();
    let object_count = 4 + page_count * 2;
    let kids: Vec<String> = (0..page_count)
        .map(|page| format!("{} 0 R", 5 + page * 2))
        .collect();

    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::with_capacity(object_count);
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut push = |body: String| {
        offsets.push(out.len());
        out.extend_from_slice(body.as_bytes());
    };
    push("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_owned());
    push(format!(
        "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {page_count} >>\nendobj\n",
        kids.join(" ")
    ));
    push(font_object(3, "Helvetica"));
    push(font_object(4, "Helvetica-Bold"));
    for (page, stream) in pages.iter().enumerate() {
        let page_id = 5 + page * 2;
        let content_id = page_id + 1;
        push(format!(
            "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /{FONT_REGULAR} 3 0 R /{FONT_BOLD} 4 0 R >> >> \
             /Contents {content_id} 0 R >>\nendobj\n"
        ));
        push(format!(
            "{content_id} 0 obj\n<< /Length {} >>\nstream\n{stream}\nendstream\nendobj\n",
            stream.len()
        ));
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", object_count + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            object_count + 1
        )
        .as_bytes(),
    );
    out
}

fn font_object(id: u32, base_font: &str) -> String {
    format!(
        "{id} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /{base_font} \
         /Encoding /WinAnsiEncoding >>\nendobj\n"
    )
}

#[cfg(test)]
mod tests {
    use super::dictionary_pdf;

    /// The importer validates the Issue 9 identity, the stated word totals,
    /// the table structure, and the exact entry counts. Passing this one
    /// call means `init` accepts the fixture.
    #[test]
    fn the_fixture_imports_with_the_expected_entry_counts() {
        let import = provenance_ste100::import_dictionary(&dictionary_pdf())
            .expect("the fixture dictionary must import");
        assert_eq!(import.entries.len(), 878 + 1318);
    }
}
