//! Write definitions or generate Rust types from an `OpenAPI` document.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    provenance_codegen::run(&args)
}
