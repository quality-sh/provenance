use provenance_macros::ProjectionRow;

#[derive(ProjectionRow)]
#[table("notes")]
pub struct Note {
    pub id: String,
    pub search_text: String,
}

fn main() {}
