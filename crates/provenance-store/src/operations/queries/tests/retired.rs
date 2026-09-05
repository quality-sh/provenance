//! A retired record is answered only when the request asks for it, on
//! every operation that hands records back, and the stamp names the
//! tables that decided it.

use super::comparison::test_stores::TestStore;
use crate::operations::queries;
use provenance_core::protocol::{GetQuery, SDK_PROTOCOL_VERSION};
use provenance_core::NodeType;
use provenance_macros::verifies;

fn get(id: &str, include_retired: bool) -> GetQuery {
    GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: NodeType::Requirement,
        id: id.into(),
        include_retired,
    }
}

#[tokio::test]
#[verifies("rule_retired_records_answer_only_when_asked", examples)]
async fn get_reads_a_retired_record_only_when_asked() {
    let store = TestStore::pinned();
    let hidden = queries::get(
        Some(store.root.clone()),
        &store.scope,
        get("req_old_overtime", false),
    )
    .await
    .unwrap();
    assert!(
        !hidden.result.found,
        "a retired record is out of an active view"
    );
    assert!(hidden.result.node.is_none());
    assert_eq!(hidden.stamp.attested, ["requirements"]);
    assert!(hidden.stamp.live.is_empty(), "get reads nothing live");

    let shown = queries::get(
        Some(store.root.clone()),
        &store.scope,
        get("req_old_overtime", true),
    )
    .await
    .unwrap();
    assert!(shown.result.found);
    assert!(shown.result.node.unwrap().retired());
    assert_eq!(shown.stamp.attested, ["requirements"]);
}
