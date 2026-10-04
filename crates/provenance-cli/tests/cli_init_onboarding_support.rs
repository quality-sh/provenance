pub fn without_review_guidance(mut text: String) -> String {
    let start = text
        .find("- To change a Requirement, Rule, or past decision")
        .unwrap();
    let end = start
        + text[start..]
            .find("- To drop a Question or Topic")
            .unwrap();
    text.replace_range(
        start..end,
        "- To change a Requirement, Rule, or past decision, create a Proposal. A human decides each\n  Proposal.\n",
    );
    text
}
