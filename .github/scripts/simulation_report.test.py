#!/usr/bin/env python3

import importlib.util
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
    "replay": "HEGEL_SEED=1234 cargo nextest run --profile simulation -p provenance-simulation --test engine",
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
            report.open_issues(root, "https://run", run=gh)
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
            report.open_issues(root, "https://run", run=gh)
        self.assertEqual(gh.made("create"), [])
        comments = gh.made("comment")
        self.assertEqual(len(comments), 1)
        self.assertEqual(comments[0][2], "7")
        self.assertIn("https://run", comments[0][4])

    def test_one_artifact_unpacked_in_place_names_its_layer(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE], name=None)
            report.open_issues(root, "https://run", run=gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: law L5 broken in the engine layer"])

    def test_a_layer_log_without_failures_reports_the_layer(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [], name=None)
            report.open_issues(root, "https://run", run=gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: the engine layer failed without a broken law"])

    def test_no_artifact_still_opens_an_issue(self):
        gh = FakeGh()
        with tempfile.TemporaryDirectory() as root:
            report.open_issues(root, "https://run", run=gh)
        titles = [call[3] for call in gh.made("create")]
        self.assertEqual(titles, ["Simulation: the simulation layer failed without a broken law"])

    def test_each_layer_and_law_gets_its_own_issue(self):
        gh = FakeGh()
        other = {**FAILURE, "law": "L3"}
        with tempfile.TemporaryDirectory() as root:
            layer_artifact(root, [FAILURE, other])
            layer_artifact(root, [{**FAILURE, "layer": "contract"}], name="simulation-contract")
            report.open_issues(root, "https://run", run=gh)
        titles = sorted(call[3] for call in gh.made("create"))
        self.assertEqual(titles, [
            "Simulation: law L3 broken in the engine layer",
            "Simulation: law L5 broken in the contract layer",
            "Simulation: law L5 broken in the engine layer",
        ])


if __name__ == "__main__":
    unittest.main()
