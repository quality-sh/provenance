use provenance_http_client::parameters::Side;

fn parse(raw: &str) -> Side {
    raw.parse().unwrap()
}

fn main() {
    let _ = parse("before");
}
