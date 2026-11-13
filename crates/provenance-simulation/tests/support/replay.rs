//! The replay line of a probe, read from the text of a broken probe.

use provenance_simulation::laws::Violation;
use provenance_simulation::report;

/// The name of the probe that [`replay_of_a_broken_probe`] breaks.
pub const PROBE: &str = "a_planted_probe";

/// The replay line that a broken probe of `layer` prints in the test target
/// that calls this function.
pub fn replay_of_a_broken_probe(layer: &str) -> String {
    let broken = Err(Violation {
        law: "L3",
        detail: "a planted law break".to_owned(),
    });
    let payload = std::panic::catch_unwind(|| {
        report::probe(layer, PROBE, |probe| probe.laws(broken));
    })
    .expect_err("a broken probe fails");
    let text = report::panic_text(&*payload);
    text.lines()
        .find_map(|line| line.strip_prefix("replay: "))
        .unwrap_or_else(|| panic!("a replay line in {text}"))
        .to_owned()
}

/// The part of a replay line that selects `package`, `target` and the probe.
pub fn selection(package: &str, target: &str) -> String {
    format!("-p {package} --test {target} -E 'test(={PROBE})'")
}

/// The text of a probe of `layer` whose check that is not a law fails.
pub fn report_of_a_failed_check(layer: &str) -> String {
    let payload = std::panic::catch_unwind(|| {
        report::probe(layer, PROBE, |_| panic!("a planted check"));
    })
    .expect_err("a failed check fails the probe");
    report::panic_text(&*payload)
}
