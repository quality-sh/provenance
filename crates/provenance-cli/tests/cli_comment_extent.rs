//! A Rule comment continues on each comment line below it, in each order of
//! line comments and block comments, and the text of a block comment stops at
//! its close.
use provenance_macros::verifies;

use crate::support::comment_sites::{scan, sites, Site};

fn site(line: u64, rule: &str, name: &str) -> Site {
    (line, rule.to_owned(), None, Some(name.to_owned()))
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_line_comment_and_a_block_comment_line_make_one_rule_comment_in_each_order() {
    let line_first = "// @provenance rule: rule_a\n\
                      /* note */\n\
                      // @provenance rule: rule_b\n\
                      pub fn pays() {}\n";
    let block_first = "/* @provenance rule: rule_a */\n\
                       // note\n\
                       // @provenance rule: rule_b\n\
                       pub fn pays() {}\n";

    let expected = [site(1, "rule_a", "pays"), site(1, "rule_b", "pays")];
    let line_first = scan("src/pay.rs", line_first);
    assert_eq!(sites(&line_first), expected, "{line_first}");
    let block_first = scan("src/pay.rs", block_first);
    assert_eq!(sites(&block_first), expected, "{block_first}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn the_text_of_a_rust_block_comment_stops_at_its_close() {
    let source = "/* @provenance rule: rule_note */ // note\n\
                  pub fn noted() {}\n\
                  /* @provenance rule: rule_alone */\n\
                  pub fn alone() {}\n\
                  /* @provenance rule: rule_inline */ fn inline() {}\n";
    let report = scan("src/pay.rs", source);

    let expected = [
        site(1, "rule_note", "noted"),
        site(3, "rule_alone", "alone"),
        site(5, "rule_inline", "inline"),
    ];
    assert_eq!(sites(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn the_text_of_a_javascript_block_comment_stops_at_its_close() {
    let source = "/* @provenance rule: rule_note */ // note\n\
                  export function noted() {}\n\
                  /* @provenance rule: rule_alone */\n\
                  export function alone() {}\n\
                  /* @provenance rule: rule_inline */ export function inline() {}\n";
    let report = scan("src/pay.js", source);

    let expected = [
        site(1, "rule_note", "noted"),
        site(3, "rule_alone", "alone"),
        site(5, "rule_inline", "inline"),
    ];
    assert_eq!(sites(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_blank_line_in_an_open_block_comment_continues_the_rule_comment() {
    let source = "/*\n\
                  \x20* @provenance rule: rule_a\n\
                  \n\
                  \x20* @provenance rule: rule_b\n\
                  \x20*/\n\
                  export const total = 0;\n";
    let report = scan("src/total.js", source);

    let expected = [site(2, "rule_a", "total"), site(2, "rule_b", "total")];
    assert_eq!(sites(&report), expected, "{report}");
}
