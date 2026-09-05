use provenance_core::StableId;
use provenance_macros::Relations;

#[derive(Relations)]
pub struct Requirement {
    pub id: StableId,
    #[relation(flow = target_upstream)]
    pub refines: Option<StableId>,
}

fn main() {}
