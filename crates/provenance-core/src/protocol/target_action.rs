/// One target-first action selected by a shared consumer and registered by the catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetAction {
    Create,
    Update,
    Answer,
    Claim,
    Release,
    Submit,
}

impl TargetAction {
    pub const ALL: [Self; 6] = [
        Self::Create,
        Self::Update,
        Self::Answer,
        Self::Claim,
        Self::Release,
        Self::Submit,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Answer => "answer",
            Self::Claim => "claim",
            Self::Release => "release",
            Self::Submit => "submit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }
}
