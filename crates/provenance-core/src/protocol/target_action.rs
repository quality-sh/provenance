/// A shared action keyword. Record actions have catalog target registrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetAction {
    Create,
    Update,
    Answer,
    Claim,
    Release,
    Submit,
    Discussions,
    Discussion,
    Discuss,
    Reply,
}

impl TargetAction {
    pub const RECORD: [Self; 6] = [
        Self::Create,
        Self::Update,
        Self::Answer,
        Self::Claim,
        Self::Release,
        Self::Submit,
    ];
    pub const DISCUSSION: [Self; 4] = [
        Self::Discussions,
        Self::Discussion,
        Self::Discuss,
        Self::Reply,
    ];
    pub const ALL: [Self; 10] = [
        Self::Create,
        Self::Update,
        Self::Answer,
        Self::Claim,
        Self::Release,
        Self::Submit,
        Self::Discussions,
        Self::Discussion,
        Self::Discuss,
        Self::Reply,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Answer => "answer",
            Self::Claim => "claim",
            Self::Release => "release",
            Self::Submit => "submit",
            Self::Discussions => "discussions",
            Self::Discussion => "discussion",
            Self::Discuss => "discuss",
            Self::Reply => "reply",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }
}
