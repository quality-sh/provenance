use provenance_porcelain::Porcelain;

#[derive(Debug, PartialEq)]
struct TestPort(&'static str);

#[test]
fn porcelain_receives_its_port_from_the_caller() {
    let porcelain = Porcelain::new(TestPort("test-port"));

    assert_eq!(porcelain.into_port(), TestPort("test-port"));
}
