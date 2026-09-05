use provenance_core::protocol::StampPolicy;
use provenance_store::cache::ProjectionFamily;
use provenance_store::operations::reader::ReadContext;
use provenance_store::operations::stamp::seal;

fn misuse(context: ReadContext) {
    let requirements = context.snapshot().table(ProjectionFamily::Requirements);
    let _stamp = seal(context, StampPolicy::CatchUp);
    let _ = requirements.family();
}

fn main() {}
