#!/usr/bin/env python3

import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "simulation_report", Path(__file__).with_name("simulation_report.py")
)
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)

FAILURE = {
    "layer": "engine",
    "law": "L5",
    "detail": "operation 1 met a moved base again",
    "seed": "1234",
    "case": {"engines": 2, "steps": [{"Turn": 0}]},
}
ENVIRONMENT = "HEGEL_SEED=1234"
REPLAY = (
    "HEGEL_SEED=1234 cargo nextest run --profile simulation --run-ignored all "
    "-p provenance-simulation --test engine -E 'test(=random_cases_keep_the_laws)'"
)
# Real nextest logs of the engine layer on Linux and of the CLI layer on
# Linux and Windows. In each log, tests pass (PASS) and tests fail in each
# form: a probe that breaks a law, a failed check (FAIL), a broken planted
# check, a crash (SIGABRT on Linux, ABORT with its exit code on Windows) and a
# test that nextest stops at its time limit (SLOW and TERMINATING while it
# runs, then TIMEOUT). The engine tests are in the test target `zz-dash`, a
# name with a dash.
REAL_LOGS = Path(__file__).parent / "fixtures" / "simulation_report"
REAL_FORMS = {
    "engine": ("provenance-simulation", "zz-dash", "SIGABRT", "HEGEL_SEED=1"),
    "cli-linux": ("provenance-cli", "simulation_cli", "SIGABRT", "HEGEL_SEED=1 SIMULATION_L6=each"),
    "cli-windows": ("provenance-cli", "simulation_cli", "ABORT", "HEGEL_SEED=1 SIMULATION_L6=end"),
}
# The other final status words of nextest 0.9.143 for a test that did not
# pass: a signal with no name, a crash with another signal, a failure with a
# leak, a test that did not start, a leak that fails, a flaky test that is a
# failure, the last try after retries, and a word that the script does not
# know.
BUILT_FAILURES = (
    "ABORT SIG 99", "SIGSEGV", "FAIL + LEAK", "XFAIL", "LEAK-FAIL", "FLKY-FL 2/3",
    "TRY 3 FAIL", "TRY 3 SEGV", "TRY 3 LKFAIL", "A NEW WORD",
)


# The counter of a status line while a test runs or of a skip.
PADDED = "(\u2500\u2500\u2500)"


def status_line(status, binary_id, test, time="   6.102s", counter="(7/7)"):
    """A status line as nextest prints it: the status right-aligned to 12
    characters, the time and the counter."""
    return (
        f"\x1b[31;1m{status:>12}\x1b[0m [{time}] {counter} "
        f"\x1b[35;1m{binary_id}\x1b[0m \x1b[34;1m{test}\x1b[0m\n"
    )


def failed(binary_id, test, stderr, status="FAIL"):
    """A test that did not pass, as nextest prints it in the log of a layer:
    the status line, then the output with each line indented by four
    spaces."""
    lines = "".join(f"    {line}\n" for line in stderr.splitlines())
    return (
        status_line(status, binary_id, test)
        + "\x1b[31;1m \x1b[0m \x1b[31;1mstderr\x1b[0m \x1b[31;1m\u2500\u2500\u2500\x1b[0m\n"
        f"{lines}\n" + "\u2500" * 12 + "\n"
    )


def nextest_log(*tests, finished=True, passed=()):
    """The log of a layer with the failed `tests` and the status lines
    `passed`; a log that is not `finished` stops before the summary of
    nextest, which repeats the status line of each failed test."""
    log = "\x1b[32;1m    Starting\x1b[0m \x1b[1m7\x1b[0m tests across \x1b[1m2\x1b[0m binaries\n"
    log += "".join(tests) + "".join(passed)
    if finished:
        log += (
            "\x1b[31;1m     Summary\x1b[0m [   6.103s] \x1b[1m7\x1b[0m tests run: "
            "\x1b[1m6\x1b[0m \x1b[32;1mpassed\x1b[0m, \x1b[1m1\x1b[0m \x1b[31;1mfailed\x1b[0m\n"
        )
        for test in tests:
            log += test.splitlines()[0] + "\n"
        log += "\x1b[31;1merror\x1b[0m: test run failed\n"
    return log


def panic(test, message):
    return f"thread '{test}' (2) panicked at crates/provenance-simulation/src/report.rs:9:5:\n{message}\n"


RANDOM = "random_cases_keep_the_laws"
RANDOM_FAILURE = failed(
    "provenance-simulation::engine",
    RANDOM,
    "simulation law L3 broken in layer engine: other\n"
    '#[hegel::reproduce_failure("OLDBLOB")]\n'
    + panic(RANDOM, "simulation law L5 broken in layer engine: operation 1\nseed: 1234")
    + "steps: ...\n"
    '#[hegel::reproduce_failure("AAEC")]\n',
)
LOG = nextest_log(RANDOM_FAILURE)
PROBE = "a_case_with_every_fault_keeps_the_laws"
PROBE_FAILURE = failed(
    "provenance-simulation::engine",
    PROBE,
    panic(PROBE, "simulation probe broke law L1 in layer engine: operation 3 failed"),
)
PROBE_LOG = nextest_log(PROBE_FAILURE)
REFUSED = "a_refused_post_lands_no_thread_and_no_message"
CHECK_FAILURE = failed(
    "provenance-simulation::refused_post",
    REFUSED,
    panic(REFUSED, "assertion `left == right` failed: turns 3\n  left: (1, 0)\n right: (0, 0)")
    + "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
)
PLANTED = "a_lost_write_breaks_l3"
CHECK_BROKEN = failed(
    "provenance-simulation::engine_planted",
    PLANTED,
    panic(PLANTED, "simulation check broken: law L3 did not see its planted fault: x"),
)
TIMEOUT = failed("provenance-simulation::engine", PROBE, "", status="TIMEOUT")


def layer_artifact(root, failures, log=LOG, name="simulation-engine", environment=ENVIRONMENT):
    """A downloaded artifact in its own directory; `name=None` unpacks it in `root`."""
    directory = Path(root) / name if name else Path(root)
    (directory / "failures").mkdir(parents=True)
    layer = failures[0]["layer"] if failures else "engine"
    (directory / f"{layer}.log").write_text(log)
    (directory / f"{layer}.env").write_text(environment + "\n")
    for failure in failures:
        path = directory / "failures" / f"{failure['layer']}-{failure['law']}.json"
        path.write_text(json.dumps(failure))
    return directory


def open_issues(root, gh, **options):
    out = io.StringIO()
    report.open_issues(root, "https://run", run=gh, out=out, **options)
    return out.getvalue()


class FakeGh:
    def __init__(self, open_issues=()):
        self.calls = []
        self.open_issues = list(open_issues)

    def __call__(self, *args):
        self.calls.append(args)
        if args[:2] == ("issue", "list"):
            return json.dumps(self.open_issues)
        return ""

    def made(self, verb):
        return [call for call in self.calls if call[:2] == ("issue", verb)]


def titles(gh):
    return sorted(call[3] for call in gh.made("create"))


def bodies(gh):
    return {call[3]: call[7] for call in gh.made("create")}


def issues_of(log, failures=()):
    gh = FakeGh()
    with tempfile.TemporaryDirectory() as root:
        layer_artifact(root, list(failures), log=log, name=None)
        open_issues(root, gh)
    return gh


def replay_of(target, test, environment=ENVIRONMENT):
    return (
        f"{environment} cargo nextest run --profile simulation --run-ignored all "
        f"-p provenance-simulation --test {target} -E 'test(={test})'"
    )


class ReportTests(unittest.TestCase):
    def test_the_blob_is_the_one_printed_after_the_law(self):
        self.assertEqual(report.blob_for(LOG, FAILURE), "AAEC")

    def test_a_blob_printed_before_the_law_is_found(self):
        log = 'other\n#[hegel::reproduce_failure("EARLY")]\nsimulation law L5 broken in layer engine: x\n'
        self.assertEqual(report.blob_for(log, FAILURE), "EARLY")

    def test_a_law_that_is_not_in_the_log_has_no_blob(self):
        self.assertIsNone(report.blob_for('#[hegel::reproduce_failure("X")]\n', FAILURE))

    def test_a_failure_without_a_blob_is_marked_not_reproduced(self):
        text = report.describe(FAILURE, None, REPLAY)
        self.assertIn("not reproduced", text)

    def test_the_summary_gives_law_seed_blob_steps_and_replay(self):
        with tempfile.TemporaryDirectory() as root:
            directory = layer_artifact(root, [FAILURE])
            text = report.summary("engine", directory / "engine.log", directory / "failures",
                                  environ={"HEGEL_SEED": "1234"})
        self.assertIn("law L5 broken in the engine layer", text)
        self.assertIn("`1234`", text)
        self.assertIn('#[hegel::reproduce_failure("AAEC")]', text)
        self.assertIn('"Turn": 0', text)
        self.assertIn(REPLAY, text)

    def test_the_summary_writes_the_replay_variables_of_the_job(self):
        environ = {"HEGEL_SEED": "77", "HEGEL_TEST_CASES": "", "SIMULATION_L6": "end", "PATH": "x"}
        with tempfile.TemporaryDirectory() as root:
            log = Path(root) / "engine.log"
            log.write_text(PROBE_LOG)
            report.summary("engine", log, Path(root) / "failures", environ=environ)
            self.assertEqual((Path(root) / "engine.env").read_text(), "HEGEL_SEED=77 SIMULATION_L6=end\n")
            text = "".join(body for _, body in report.reports(root, "https://run"))
        self.assertIn(replay_of("engine", PROBE, "HEGEL_SEED=77 SIMULATION_L6=end"), text)

    def test_a_failed_job_without_a_log_still_reports(self):
        with tempfile.TemporaryDirectory() as root:
            text = report.summary("engine", Path(root) / "missing.log", Path(root) / "none",
                                  environ={})
        self.assertIn("failed without a broken law", text)

    def test_a_new_law_opens_one_issue(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE])
            open_issues(root, gh)
        created = gh.made("create")
        self.assertEqual(len(created), 1)
        self.assertIn("Simulation: law L5 broken in the engine layer", created[0])
        self.assertIn("simulation", created[0])
        self.assertIn(REPLAY, created[0][7])
        self.assertEqual(gh.made("comment"), [])

    def test_an_open_issue_for_the_law_gets_a_comment(self):
        title = "Simulation: law L5 broken in the engine layer"
        gh = FakeGh([{"number": 7, "title": title}, {"number": 8, "title": title + " too"}])
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE])
            open_issues(root, gh)
        self.assertEqual(gh.made("create"), [])
        comments = gh.made("comment")
        self.assertEqual(len(comments), 1)
        self.assertEqual(comments[0][2], "7")
        self.assertIn("https://run", comments[0][4])

    def test_one_artifact_unpacked_in_place_names_its_layer(self):
        gh = issues_of(LOG, [FAILURE])
        self.assertEqual(titles(gh), ["Simulation: law L5 broken in the engine layer"])

    def test_a_layer_log_without_a_test_report_reports_the_layer(self):
        gh = issues_of("error: could not compile `provenance-simulation`\n")
        self.assertEqual(titles(gh), ["Simulation: the engine layer failed without a broken law"])

    def test_no_artifact_still_opens_an_issue(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            open_issues(root, gh)
        self.assertEqual(titles(gh), ["Simulation: the simulation layer failed without a broken law"])

    def test_each_layer_gets_its_own_issue(self):
        gh = FakeGh()
        contract_log = nextest_log(failed(
            "provenance-simulation::contract",
            "random_cases_keep_the_laws_through_the_contract",
            "simulation law L5 broken in layer contract: x",
        ))
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE])
            layer_artifact(
                root,
                [{**FAILURE, "layer": "contract"}],
                log=contract_log,
                name="simulation-contract",
            )
            open_issues(root, gh)
        self.assertEqual(titles(gh), [
            "Simulation: law L5 broken in the contract layer",
            "Simulation: law L5 broken in the engine layer",
        ])

    def test_a_law_that_the_shrink_passed_through_opens_no_issue(self):
        # The log ends with L5, so the L3 file of a shrink step opens nothing.
        gh = issues_of(LOG, [FAILURE, {**FAILURE, "law": "L3"}])
        self.assertEqual(titles(gh), ["Simulation: law L5 broken in the engine layer"])

    def test_a_failure_file_of_a_job_that_stopped_reports_the_law_and_the_layer(self):
        gh = issues_of("the job stopped at its time limit\n", [FAILURE])
        self.assertEqual(titles(gh), [
            "Simulation: law L5 broken in the engine layer",
            "Simulation: the engine layer failed without a broken law",
        ])
        law = bodies(gh)["Simulation: law L5 broken in the engine layer"]
        self.assertIn("Replay: not printed; the test did not end with its report", law)

    def test_a_law_line_without_a_failure_file_reports_its_test(self):
        gh = issues_of(LOG)
        self.assertEqual(titles(gh), [f"Simulation: test {RANDOM} failed in the engine layer"])

    def test_a_broken_planted_check_opens_no_law_issue(self):
        gh = issues_of(nextest_log(CHECK_BROKEN))
        self.assertEqual(titles(gh), ["Simulation: a planted fault was not found in the engine layer"])
        body = bodies(gh)["Simulation: a planted fault was not found in the engine layer"]
        self.assertIn("the check itself is broken", body)
        self.assertIn(f"`{PLANTED}`", body)
        self.assertIn(replay_of("engine_planted", PLANTED), body)

    def test_a_broken_check_is_reported_with_the_law_of_its_layer(self):
        gh = issues_of(nextest_log(RANDOM_FAILURE, CHECK_BROKEN), [FAILURE])
        self.assertEqual(titles(gh), [
            "Simulation: a planted fault was not found in the engine layer",
            "Simulation: law L5 broken in the engine layer",
        ])

    def test_the_text_of_a_failed_planted_test_does_not_replace_the_law(self):
        planted_test = "random_planted_cases_repeat_by_seed_and_replay_by_blob"
        planted_text = "planted run in layer engine: law L5 reported: operation 1"
        planted_run = failed(
            "provenance-simulation::engine_planted", planted_test, panic(planted_test, planted_text)
        )
        gh = issues_of(nextest_log(RANDOM_FAILURE, planted_run, CHECK_BROKEN), [FAILURE])
        self.assertEqual(titles(gh), [
            "Simulation: a planted fault was not found in the engine layer",
            "Simulation: law L5 broken in the engine layer",
            f"Simulation: test {planted_test} failed in the engine layer",
        ])
        self.assertIsNone(report.LAW_LINE.search(planted_text))
        self.assertIsNone(report.LAW_LINE.search(CHECK_BROKEN))

    def test_a_probe_that_breaks_a_law_opens_an_issue_with_its_replay(self):
        gh = issues_of(nextest_log(PROBE_FAILURE, PROBE_FAILURE))
        title = f"Simulation: probe {PROBE} broke law L1 in the engine layer"
        self.assertEqual(titles(gh), [title])
        self.assertIn(replay_of("engine", PROBE), bodies(gh)[title])
        self.assertIn("operation 3 failed", bodies(gh)[title])

    def test_a_failed_check_of_a_probe_and_a_broken_law_of_one_layer_open_two_issues(self):
        gh = issues_of(nextest_log(PROBE_FAILURE, CHECK_FAILURE))
        check_title = f"Simulation: test {REFUSED} failed in the engine layer"
        self.assertEqual(titles(gh), [
            f"Simulation: probe {PROBE} broke law L1 in the engine layer",
            check_title,
        ])
        body = bodies(gh)[check_title]
        self.assertIn("assertion `left == right` failed: turns 3 left: (1, 0) right: (0, 0)", body)
        self.assertIn(replay_of("refused_post", REFUSED), body)

    def test_a_test_that_failed_beside_a_law_of_its_layer_has_its_own_issue(self):
        gh = issues_of(nextest_log(RANDOM_FAILURE, CHECK_FAILURE), [FAILURE])
        self.assertEqual(titles(gh), [
            "Simulation: law L5 broken in the engine layer",
            f"Simulation: test {REFUSED} failed in the engine layer",
        ])

    def test_a_test_stopped_at_its_time_limit_has_its_own_issue(self):
        gh = issues_of(nextest_log(RANDOM_FAILURE, TIMEOUT), [FAILURE])
        title = f"Simulation: test {PROBE} did not end in its time limit in the engine layer"
        self.assertEqual(titles(gh), ["Simulation: law L5 broken in the engine layer", title])
        self.assertIn(replay_of("engine", PROBE), bodies(gh)[title])

    def test_a_crash_has_its_own_issue(self):
        crash = failed("provenance-simulation::engine", PROBE, "", status="SIGABRT")
        gh = issues_of(nextest_log(crash))
        title = f"Simulation: test {PROBE} failed in the engine layer"
        self.assertEqual(titles(gh), [title])
        self.assertIn("the test ended with SIGABRT", bodies(gh)[title])

    def test_a_target_name_with_a_dash_is_given_back(self):
        probe = failed("provenance-simulation::refused-post", PROBE, panic(PROBE, "a check"))
        gh = issues_of(nextest_log(probe))
        body = bodies(gh)[f"Simulation: test {PROBE} failed in the engine layer"]
        self.assertIn(replay_of("refused-post", PROBE), body)

    def test_a_log_that_stops_before_the_summary_also_reports_the_layer(self):
        gh = issues_of(nextest_log(PROBE_FAILURE, finished=False))
        self.assertEqual(titles(gh), [
            f"Simulation: probe {PROBE} broke law L1 in the engine layer",
            "Simulation: the engine layer failed without a broken law",
        ])

    def test_the_real_log_of_each_system_gives_a_report_for_each_failed_test(self):
        for layer, (package, target, crash, environment) in REAL_FORMS.items():
            with self.subTest(layer=layer):
                log = (REAL_LOGS / f"{layer}.log").read_text()
                found = dict(report.layer_reports(layer, log, [], "https://run", environment))
                self.assertEqual(sorted(found), sorted([
                    f"Simulation: a planted fault was not found in the {layer} layer",
                    f"Simulation: probe zz_probe_breaks_a_law broke law L3 in the {layer} layer",
                    f"Simulation: test zz_check_fails_in_a_probe failed in the {layer} layer",
                    f"Simulation: test zz_test_aborts failed in the {layer} layer",
                    f"Simulation: test zz_test_hangs did not end in its time limit in the {layer} layer",
                ]))
                for test in ["zz_check_fails_in_a_probe", "zz_test_aborts", "zz_test_hangs"]:
                    command = (
                        f"{environment} cargo nextest run --profile simulation --run-ignored all "
                        f"-p {package} --test {target} -E 'test(={test})'"
                    )
                    body = next(body for title, body in found.items() if f" {test} " in title)
                    self.assertIn(f"- Replay: `{command}`", body)
                probe = found[f"Simulation: probe zz_probe_breaks_a_law broke law L3 in the {layer} layer"]
                self.assertIn("- Detail: a planted break with a second line", probe)
                check = found[f"Simulation: test zz_check_fails_in_a_probe failed in the {layer} layer"]
                self.assertIn("assertion `left == right` failed: a planted check left: 1 right: 2", check)
                crashed = found[f"Simulation: test zz_test_aborts failed in the {layer} layer"]
                self.assertIn(f"- Detail: the test ended with {crash}", crashed)
                broken = found[f"Simulation: a planted fault was not found in the {layer} layer"]
                self.assertIn("`zz_planted_check_broken`: `simulation check broken: law L3", broken)

    def test_a_probe_line_is_not_a_law_line_of_a_random_case(self):
        self.assertIsNone(report.LAW_LINE.search(PROBE_LOG))
        self.assertIsNone(report.last_law(PROBE_LOG, "engine"))

    def test_print_mode_makes_no_write_and_prints_each_issue_and_comment(self):
        title = "Simulation: law L5 broken in the engine layer"
        gh = FakeGh([{"number": 7, "title": title}])
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], log=nextest_log(RANDOM_FAILURE, CHECK_BROKEN))
            text = open_issues(root, gh, print_only=True)
        writes = [call for call in gh.calls if call[:2] != ("issue", "list")]
        self.assertEqual(writes, [])
        self.assertIn(f"## Would comment on issue #7: {title}", text)
        self.assertIn(
            "## Would open an issue: Simulation: a planted fault was not found in the engine layer",
            text,
        )
        self.assertIn('#[hegel::reproduce_failure("AAEC")]', text)
        self.assertIn(REPLAY, text)

    def test_print_mode_is_a_choice_of_the_command_line(self):
        calls = []
        original = report.open_issues
        report.open_issues = lambda *args, **options: calls.append(options)
        try:
            report.main(["issues", "--artifacts", "a", "--run-url", "u", "--mode", "print"])
            report.main(["issues", "--artifacts", "a", "--run-url", "u"])
        finally:
            report.open_issues = original
        self.assertEqual(calls, [{"print_only": True}, {"print_only": False}])

    def test_the_summary_gives_a_broken_check_and_the_law(self):
        with tempfile.TemporaryDirectory() as root:
            log = nextest_log(RANDOM_FAILURE, CHECK_BROKEN)
            directory = layer_artifact(root, [FAILURE], log=log)
            text = report.summary("engine", directory / "engine.log", directory / "failures",
                                  environ={})
        self.assertIn("law L5 broken in the engine layer", text)
        self.assertIn("a planted fault was not found in the engine layer", text)

    def test_each_issue_links_the_guide(self):
        self.assertIn(report.GUIDE, report.describe(FAILURE, "AAEC", REPLAY))
        self.assertIn(report.GUIDE, report.describe_run("engine", False))

    def test_the_summary_of_a_passed_layer_gives_its_counts(self):
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / "cli-linux.json").write_text(json.dumps({"stops": 3}))
            text = report.summary("cli-linux", "missing", "none", root, succeeded=True)
        self.assertIn("| stops | 3 |", text)

    def test_the_text_of_a_run_without_a_law_names_each_cause(self):
        text = report.describe_run("engine", False)
        self.assertIn("does not show the end of the tests. The build failed", text)
        self.assertIn("time limit", text)

    def test_a_log_with_the_summary_and_no_report_says_that_the_tests_ended(self):
        gh = issues_of(nextest_log())
        title = "Simulation: the engine layer failed without a broken law"
        self.assertEqual(titles(gh), [title])
        self.assertIn("shows the end of the tests, but no test in it gives a report", bodies(gh)[title])
        self.assertNotIn("The build failed", bodies(gh)[title])

    def test_each_status_that_is_not_a_pass_or_a_skip_gives_its_own_report(self):
        for status in BUILT_FAILURES:
            with self.subTest(status=status):
                test = "zz_ends_with_a_status"
                other = failed("provenance-simulation::engine", test, "", status=status)
                gh = issues_of(nextest_log(CHECK_FAILURE, other))
                title = f"Simulation: test {test} failed in the engine layer"
                self.assertEqual(titles(gh), sorted([
                    f"Simulation: test {REFUSED} failed in the engine layer", title,
                ]))
                self.assertIn(f"the test ended with {status}", bodies(gh)[title])
                self.assertIn(replay_of("engine", test), bodies(gh)[title])

    def test_the_last_try_at_the_time_limit_is_a_time_limit(self):
        stopped = failed("provenance-simulation::engine", PROBE, "", status="TRY 3 TMT")
        gh = issues_of(nextest_log(CHECK_FAILURE, stopped))
        self.assertIn(
            f"Simulation: test {PROBE} did not end in its time limit in the engine layer",
            titles(gh),
        )

    def test_a_pass_or_a_skip_in_each_form_gives_no_report(self):
        engine = "provenance-simulation::engine"
        passed = [
            status_line("PASS", engine, "a_pass"),
            status_line("SLOW", engine, "a_slow_pass", time="> 60.000s", counter=PADDED),
            status_line("SLOW", engine, "a_slow_pass", time=" 312.000s"),
            status_line("SLOW + LEAK", engine, "a_slow_pass_with_a_leak"),
            status_line("SLOW+TMPASS", engine, "a_slow_pass_at_the_time_limit"),
            status_line("LEAK", engine, "a_pass_with_a_leak"),
            status_line("TIMEOUT-PASS", engine, "a_pass_at_the_time_limit"),
            status_line("START", engine, "a_flaky_pass", time="         "),
            status_line("TRY 1 FAIL", engine, "a_flaky_pass", counter=PADDED),
            status_line("TRY 2 START", engine, "a_flaky_pass", time="         ", counter=PADDED),
            status_line("FLAKY 2/2", engine, "a_flaky_pass"),
            status_line("SKIP", engine, "a_skip", time="         ", counter=PADDED),
        ]
        gh = issues_of(nextest_log(CHECK_FAILURE, passed=passed))
        self.assertEqual(titles(gh), [f"Simulation: test {REFUSED} failed in the engine layer"])


if __name__ == "__main__":
    unittest.main()
