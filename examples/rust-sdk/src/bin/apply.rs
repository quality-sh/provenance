//! Applies the spec through a separate entry point; building the spec
//! module alone has no engine or persistence side effect.

use provenance_sdk::{operations, Settings};
use rust_sdk_example::provenance_spec::share_links;

fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env();
    let document = share_links()?;
    let result = operations::apply(
        settings.repository.clone(),
        &settings.scope_id()?,
        document.materialize(settings.owner.clone()),
    )?;
    println!(
        "created {} updated {} unchanged {}",
        result.created, result.updated, result.unchanged
    );
    Ok(())
}
