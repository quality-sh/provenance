use super::*;

/// The sites of a file of `count` comment sites, each above an attribute
/// whose bracket does not close, and the code tokens of that file.
fn open_brackets(count: usize) -> (Vec<String>, Vec<(usize, usize)>) {
    let mut lines = Vec::new();
    let mut sites = Vec::new();
    for _ in 0..count {
        sites.push((lines.len() + 1, lines.len()));
        lines.push("// @provenance rule: rule_open".to_owned());
        lines.push("#[cfg(any(".to_owned());
        lines.push("    a, b,".to_owned());
    }
    (lines, sites)
}

#[test]
fn the_reads_of_the_sites_of_a_file_cover_each_token_at_most_one_time() {
    let (lines, sites) = open_brackets(200);
    let lines = lines.iter().map(String::as_str).collect::<Vec<_>>();
    let states = super::super::rust_line_states(Language::Rust, &lines);
    let tokens = file_tokens(Language::Rust, &lines, &states);

    let ranges = read_ranges(&tokens, &sites);

    assert_eq!(ranges.len(), sites.len());
    for pair in ranges.windows(2) {
        assert!(pair[0].end <= pair[1].start, "{pair:?}");
    }
    let read = ranges.iter().map(ExactSizeIterator::len).sum::<usize>();
    assert_eq!(read, tokens.len());
}
