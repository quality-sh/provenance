//! The languages that the scanner reads, and the comment syntax of each.
//!
//! Each lexer of the scanner and each test of a comment line reads the
//! comment syntax from `Language::comment_syntax`, so a language states its
//! comments in one place.

/// A language that the scanner reads, named by the extension of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    /// The comment syntax of this language.
    pub(crate) const fn comment_syntax(self) -> CommentSyntax {
        match self {
            Self::Python => CommentSyntax {
                line: "#",
                block: None,
            },
            Self::Rust | Self::JavaScript | Self::TypeScript | Self::Java | Self::Go => {
                CommentSyntax {
                    line: "//",
                    block: Some(BlockComment {
                        open: "/*",
                        close: "*/",
                    }),
                }
            }
        }
    }
}

/// The comments of a language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommentSyntax {
    /// The prefix that starts a comment to the end of its line.
    pub(crate) line: &'static str,
    /// The block comment, when the language has one.
    pub(crate) block: Option<BlockComment>,
}

/// The open and the close of a block comment. Rust block comments nest: the
/// Rust line states count the depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockComment {
    pub(crate) open: &'static str,
    pub(crate) close: &'static str,
}

impl CommentSyntax {
    /// Whether `text` starts with a line comment.
    pub(crate) fn starts_line_comment(self, text: &[u8]) -> bool {
        text.starts_with(self.line.as_bytes())
    }

    /// The length of the block-comment open at the start of `text`.
    pub(crate) fn block_open_at(self, text: &[u8]) -> Option<usize> {
        let open = self.block?.open;
        text.starts_with(open.as_bytes()).then_some(open.len())
    }

    /// The length of the block-comment close at the start of `text`.
    pub(crate) fn block_close_at(self, text: &[u8]) -> Option<usize> {
        let close = self.block?.close;
        text.starts_with(close.as_bytes()).then_some(close.len())
    }

    /// Whether `text` starts with a comment: a line comment or the open of a
    /// block comment.
    pub(crate) fn starts_comment(self, text: &[u8]) -> bool {
        self.starts_line_comment(text) || self.block_open_at(text).is_some()
    }
}
