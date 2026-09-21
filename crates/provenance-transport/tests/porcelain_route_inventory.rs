use provenance_store::operations::catalog;

const ROUTE_INVENTORY_FNV1A: u64 = 5_051_781_947_626_311_070;

fn add(hash: &mut u64, text: &str) {
    for byte in text.bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[test]
fn porcelain_seam_preserves_public_routes() {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for definition in catalog::definitions() {
        add(&mut hash, definition.method.as_str());
        add(&mut hash, " ");
        add(&mut hash, definition.path);
        add(&mut hash, "\n");
    }

    assert_eq!(hash, ROUTE_INVENTORY_FNV1A);
}
