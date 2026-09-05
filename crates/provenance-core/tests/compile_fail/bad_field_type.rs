use provenance_core::StableId;
use provenance_macros::Relations;

#[derive(Relations)]
pub struct Requirement {
    pub id: StableId,
    #[relation(target = Source, flow = target_upstream)]
    pub statement: String,
}

fn main() {}
