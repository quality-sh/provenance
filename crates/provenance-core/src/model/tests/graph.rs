use super::graph::NodeType;

#[test]
fn topic_and_question_are_thread_parent_node_types() {
    assert_eq!(NodeType::parse("topic").unwrap(), NodeType::Topic);
    assert_eq!(NodeType::parse("question").unwrap(), NodeType::Question);
}

#[test]
fn a_word_parse_normalizes_the_text_and_uses_the_serde_word_list() {
    use crate::model::{MessageRole, SourceType};

    assert_eq!(NodeType::parse(" Question ").unwrap(), NodeType::Question);
    assert_eq!(SourceType::parse("API-Spec").unwrap(), SourceType::ApiSpec);
    assert_eq!(
        MessageRole::parse("assistant").unwrap(),
        MessageRole::Assistant
    );

    let error = NodeType::parse("ticket").unwrap_err().to_string();
    assert!(
        error.contains("ticket") && error.contains("question"),
        "the error must name the bad word and the accepted words: {error}"
    );
}
