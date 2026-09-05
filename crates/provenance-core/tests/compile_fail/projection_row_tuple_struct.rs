use provenance_macros::ProjectionRow;

#[derive(ProjectionRow)]
#[table("pairs")]
pub struct Pair(String, i64);

fn main() {}
