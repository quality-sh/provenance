use provenance_core::StableId;
use provenance_macros::Relations;

#[derive(Relations)]
pub struct Requirement {
    pub id: StableId,
    pub refines: Vec<StableId>,
}

fn main() {}
