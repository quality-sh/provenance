use provenance_core::StableId;
use provenance_macros::Relations;

#[derive(Relations)]
pub struct Requirement {
    pub id: StableId,
    #[relation(target = Requirement, flow = target_upstream, colour = "red")]
    pub refines: Option<StableId>,
}

fn main() {}
