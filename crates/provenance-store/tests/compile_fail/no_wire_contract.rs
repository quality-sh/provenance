// The wire contract is in provenance-contract. The store has no operation definitions
// and no route table.
fn main() {
    let _ = provenance_store::operations::catalog::definitions();
}
