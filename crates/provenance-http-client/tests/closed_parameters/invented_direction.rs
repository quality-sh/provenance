use provenance_http_client::parameters::Direction;

fn invented() -> Direction {
    Direction::Sideways
}

fn main() {
    let _ = invented();
}
