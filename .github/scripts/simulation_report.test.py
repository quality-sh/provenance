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
    "replay": (
        "HEGEL_SEED=1234 cargo nextest run --profile simulation --run-ignored all "
        "-p provenance-simulation --test engine -E 'test(=random_cases_keep_the_laws)'"
    ),
}
LOG = (
    "simulation law L3 broken in layer engine: other\n"
    '#[hegel::reproduce_failure("OLDBLOB")]\n'
    "simulation law L5 broken in layer engine: operation 1\n"
    "steps: ...\n"
    '#[hegel::reproduce_failure("AAEC")]\n'
)


def layer_artifact(root, failures, log=LOG, name="simulation-engine"):
    """A downloaded artifact in its own directory; `name=None` unpacks it in `root`."""
    directory = Path(root) / name if name else Path(root)
    (directory / "failures").mkdir(parents=True)
    layer = failures[0]["layer"] if failures else "engine"
    (directory / f"{layer}.log").write_text(log)
    for failure in failures:
        path = directory / "failures" / f"{failure['layer']}-{failure['law']}.json"
        path.write_text(json.dumps(failure))
    return directory


PROBE_LOG = (
    "thread 'a_case_with_every_fault_keeps_the_laws' panicked at tests/engine.rs:9:5:\n"
    "simulation probe a_case_with_every_fault_keeps_the_laws broke law L1 in layer engine: "
    "operation 3 failed\n"
    "replay: cargo nextest run --profile simulation --run-ignored all -p provenance-simulation "
    "--test engine -E 'test(=a_case_with_every_fault_keeps_the_laws)'\n"
)
CHECK_LOG = "simulation check broken: law L3 did not see its planted fault: x\n"
# A planted test that fails prints the text of its planted run.
PLANTED_TEXT = "planted run in layer engine: law L5 reported: operation 1\n"


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


class ReportTests(unittest.TestCase):
    def test_the_blob_is_the_one_printed_after_the_law(self):
        self.assertEqual(report.blob_for(LOG, FAILURE), "AAEC")

    def test_a_blob_printed_before_the_law_is_found(self):
        log = 'other\n#[hegel::reproduce_failure("EARLY")]\nsimulation law L5 broken in layer engine: x\n'
        self.assertEqual(report.blob_for(log, FAILURE), "EARLY")

    def test_a_law_that_is_not_in_the_log_has_no_blob(self):
        self.assertIsNone(report.blob_for('#[hegel::reproduce_failure("X")]\n', FAILURE))

    def test_a_failure_without_a_blob_is_marked_not_reproduced(self):
        text = report.describe(FAILURE, None)
        self.assertIn("not reproduced", text)

    def test_the_summary_gives_law_seed_blob_steps_and_replay(self):
        with tempfile.TemporaryDirectory() as root:
            directory = layer_artifact(root, [FAILURE])
            text = report.summary("engine", directory / "engine.log", directory / "failures")
        self.assertIn("law L5 broken in the engine layer", text)
        self.assertIn("`1234`", text)
        self.assertIn('#[hegel::reproduce_failure("AAEC")]', text)
        self.assertIn('"Turn": 0', text)
        self.assertIn(FAILURE["replay"], text)

    def test_a_failed_job_without_a_law_still_reports(self):
        with tempfile.TemporaryDirectory() as root:
            text = report.summary("engine", Path(root) / "missing.log", Path(root) / "none")
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
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], name=None)
            open_issues(root, gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: law L5 broken in the engine layer"])

    def test_a_layer_log_without_failures_reports_the_layer(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [], name=None)
            open_issues(root, gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: the engine layer failed without a broken law"])

    def test_no_artifact_still_opens_an_issue(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            open_issues(root, gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: the simulation layer failed without a broken law"])

    def test_each_layer_gets_its_own_issue(self):
        gh = FakeGh()
        contract_log = "simulation law L5 broken in layer contract: x\n"
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE])
            layer_artifact(
                root,
                [{**FAILURE, "layer": "contract"}],
                log=contract_log,
                name="simulation-contract",
            )
            open_issues(root, gh)
        titles = sorted(call[3] for call in gh.made("create"))
        self.assertEqual(titles, [
            "Simulation: law L5 broken in the contract layer",
            "Simulation: law L5 broken in the engine layer",
        ])

    def test_a_law_that_the_shrink_passed_through_opens_no_issue(self):
        # The log ends with L5, so the L3 file of a shrink step opens nothing.
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE, {**FAILURE, "law": "L3"}])
            open_issues(root, gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: law L5 broken in the engine layer"])

    def test_a_failure_file_without_a_law_line_still_opens_an_issue(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], log="the job stopped at its time limit\n")
            open_issues(root, gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: law L5 broken in the engine layer"])

    def test_a_broken_planted_check_opens_no_law_issue(self):
        gh = FakeGh()
        log = "simulation check broken: law L3 did not see its planted fault: x\n"
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [], log=log, name=None)
            open_issues(root, gh)
        created = gh.made("create")
        self.assertEqual(
            [call[3] for call in created],
            ["Simulation: a planted fault was not found in the engine layer"],
        )
        self.assertIn("the check itself is broken", created[0][7])

    def test_a_broken_check_is_reported_with_the_law_of_its_layer(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], log=LOG + CHECK_LOG)
            open_issues(root, gh)
        titles = sorted(call[3] for call in gh.made("create"))
        self.assertEqual(titles, [
            "Simulation: a planted fault was not found in the engine layer",
            "Simulation: law L5 broken in the engine layer",
        ])

    def test_the_text_of_a_failed_planted_test_does_not_replace_the_law(self):
        # The planted text comes after the last broken-law line of the log.
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], log=LOG + PLANTED_TEXT + CHECK_LOG)
            open_issues(root, gh)
        titles = sorted(call[3] for call in gh.made("create"))
        self.assertIn("Simulation: law L5 broken in the engine layer", titles)
        self.assertIsNone(report.LAW_LINE.search(PLANTED_TEXT))
        self.assertIsNone(report.LAW_LINE.search(CHECK_LOG))

    def test_a_probe_that_breaks_a_law_opens_an_issue_with_its_replay(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [], log=PROBE_LOG + PROBE_LOG, name=None)
            open_issues(root, gh)
        created = gh.made("create")
        self.assertEqual(
            [call[3] for call in created],
            ["Simulation: probe a_case_with_every_fault_keeps_the_laws broke law L1 "
             "in the engine layer"],
        )
        self.assertIn("-E 'test(=a_case_with_every_fault_keeps_the_laws)'", created[0][7])
        self.assertIn("operation 3 failed", created[0][7])

    def test_a_probe_line_is_not_a_law_line_of_a_random_case(self):
        self.assertIsNone(report.LAW_LINE.search(PROBE_LOG))
        self.assertIsNone(report.last_law(PROBE_LOG, "engine"))

    def test_print_mode_makes_no_write_and_prints_each_issue_and_comment(self):
        title = "Simulation: law L5 broken in the engine layer"
        gh = FakeGh([{"number": 7, "title": title}])
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], log=LOG + CHECK_LOG)
            text = open_issues(root, gh, print_only=True)
        writes = [call for call in gh.calls if call[:2] != ("issue", "list")]
        self.assertEqual(writes, [])
        self.assertIn(f"## Would comment on issue #7: {title}", text)
        self.assertIn(
            "## Would open an issue: Simulation: a planted fault was not found in the engine layer",
            text,
        )
        self.assertIn('#[hegel::reproduce_failure("AAEC")]', text)
        self.assertIn(FAILURE["replay"], text)

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
            directory = layer_artifact(root, [FAILURE], log=LOG + CHECK_LOG)
            text = report.summary("engine", directory / "engine.log", directory / "failures")
        self.assertIn("law L5 broken in the engine layer", text)
        self.assertIn("a planted fault was not found in the engine layer", text)

    def test_each_issue_links_the_guide(self):
        self.assertIn(report.GUIDE, report.describe(FAILURE, "AAEC"))
        self.assertIn(report.GUIDE, report.describe_run("engine"))

    def test_the_summary_of_a_passed_layer_gives_its_counts(self):
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / "cli-linux.json").write_text(json.dumps({"stops": 3}))
            text = report.summary("cli-linux", "missing", "none", root, succeeded=True)
        self.assertIn("| stops | 3 |", text)

    def test_the_text_of_a_run_without_a_law_names_each_cause(self):
        text = report.describe_run("engine")
        self.assertIn("a probe test or a planted test failed", text)
        self.assertIn("time limit", text)


if __name__ == "__main__":
    unittest.main()
