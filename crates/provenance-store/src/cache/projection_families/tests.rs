use super::{ProjectionFamily, FAMILIES};

#[test]
fn every_projection_family_has_one_plain_metadata_row() {
    assert_eq!(ProjectionFamily::ALL.len(), FAMILIES.len());
    for (family, metadata) in ProjectionFamily::ALL.into_iter().zip(FAMILIES) {
        assert_eq!(family, metadata.family);
        assert_eq!(family.family_name(), metadata.table_name);
        assert_eq!(family.shard_suffix(), metadata.shard_suffix);
        assert_eq!(family.node_type(), metadata.node_type);
    }
}

#[test]
fn family_metadata_keeps_stable_storage_and_export_names_together() {
    assert_eq!(ProjectionFamily::Sources.family_name(), "sources");
    assert_eq!(
        ProjectionFamily::Sources.shard_suffix(),
        "sources/source.jsonl"
    );
    assert_eq!(ProjectionFamily::Sources.graph_field(), Some("sources"));
    assert_eq!(
        ProjectionFamily::SynthesisPackets.family_name(),
        "synthesis_packets"
    );
    assert_eq!(
        ProjectionFamily::SynthesisPackets.shard_suffix(),
        "ideation/synthesis_packets.jsonl"
    );
    assert_eq!(ProjectionFamily::SynthesisPackets.graph_field(), None);
}

#[test]
fn catalog_registers_each_family_operation() {
    let registered = crate::operations::catalog::registered_operation_names_for_test();
    for family in ProjectionFamily::ALL {
        for operation in family.catalog_operation_names() {
            assert!(
                registered.contains(operation),
                "{} operation {operation} is absent from the catalog",
                family.family_name()
            );
        }
    }
}
