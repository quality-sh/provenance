// The wire contract is in provenance-contract. The store has no route table and does not
// export the full record family rows.
fn main() {
    let _ = provenance_store::operations::catalog::definitions();
    provenance_store::expand_record_family_rows!();
}
