use provenance_core::protocol::StampPolicy;
use provenance_core::Requirement;
use provenance_store::operations::reader::ReadContext;
use provenance_store::operations::stamp::seal;

fn misuse(context: ReadContext) {
    let requirements = context.snapshot().table::<Requirement>();
    let _stamp = seal(context, StampPolicy::CatchUp);
    let _ = requirements.count();
}

fn main() {}
