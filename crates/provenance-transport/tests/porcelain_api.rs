#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use provenance_porcelain::api::{ApiArguments, ApiMethod, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::HostApiPort;
use support::api_fixture::{host, Repository};

#[tokio::test]
async fn host_api_port_maps_canonical_failures_for_direct_callers() {
    let repository = Repository::new("The shared graph is readable.");
    let service = Porcelain::new(HostApiPort::new(host(&repository)));

    let unknown = service
        .execute_api(
            ApiRequest::from(ApiArguments {
                path: Some("nowhere".into()),
                ..Default::default()
            })
            .expect("valid request"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        unknown.kind,
        provenance_porcelain::api::ApiErrorKind::UnknownPath
    );
    assert_eq!(unknown.failure["error"]["kind"], "unknown_operation");

    let method = service
        .execute_api(
            ApiRequest::from(ApiArguments {
                path: Some("requirements/req_shared/submit".into()),
                method: Some(ApiMethod::Get),
                ..Default::default()
            })
            .expect("valid request"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        method.kind,
        provenance_porcelain::api::ApiErrorKind::MethodNotAllowed
    );
    assert_eq!(method.failure["error"]["kind"], "method_not_allowed");
}
