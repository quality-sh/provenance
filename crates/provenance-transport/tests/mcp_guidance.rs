use provenance_macros::verifies;
use provenance_transport::StatementHost;
use rmcp::ServiceExt as _;

#[tokio::test]
#[verifies("rule_porcelain_mcp_guidance_native", examples)]
async fn initialization_teaches_domain_without_a_prime_tool() {
    let host = StatementHost::default();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let info = client.peer_info().unwrap();
    let instructions = info.instructions.as_deref().expect("server instructions");
    for term in ["Requirement", "Rule", "Resolution", "Implementation binding", "Verification"] {
        assert!(instructions.contains(term), "guidance omits {term}");
    }
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().all(|tool| tool.name != "prime"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
