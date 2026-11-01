#!/usr/bin/env python3

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


SCRIPT = Path(__file__).with_name("ci_ok.py")
WORKFLOW = Path(__file__).parents[1] / "workflows" / "ci.yml"


def run(**results):
    needs = {name: {"result": result, "outputs": {}} for name, result in results.items()}
    return subprocess.run(
        [sys.executable, str(SCRIPT)],
        env={**os.environ, "RESULTS": json.dumps(needs)},
        text=True,
        capture_output=True,
        check=False,
    )


class DecisionTests(unittest.TestCase):
    def test_success_and_skipped_jobs_pass(self):
        result = run(changes="success", test="skipped", **{"compile-fail": "skipped"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("test: skipped", result.stdout)

    def test_a_failed_job_fails(self):
        result = run(changes="success", test="failure", fmt="skipped")
        self.assertEqual(result.returncode, 1)
        self.assertIn("test", result.stderr)

    def test_a_cancelled_job_fails(self):
        result = run(changes="success", **{"compile-fail": "cancelled"})
        self.assertEqual(result.returncode, 1)
        self.assertIn("compile-fail", result.stderr)

    def test_a_failed_selection_fails_even_when_every_other_job_skips(self):
        result = run(changes="failure", test="skipped", clippy="skipped")
        self.assertEqual(result.returncode, 1)


class WorkflowWiringTests(unittest.TestCase):
    def test_ci_ok_always_runs_the_decision_over_every_job(self):
        workflow = WORKFLOW.read_text()
        job = workflow.split("\n  ci-ok:\n", 1)[1]
        self.assertIn("name: CI OK", job)
        self.assertIn("if: always()", job)
        self.assertIn("RESULTS: ${{ toJSON(needs) }}", job)
        self.assertIn("python3 .github/scripts/ci_ok.py", job)
        jobs = [
            line[2:-1]
            for line in workflow.split("\njobs:\n", 1)[1].splitlines()
            if line.startswith("  ") and not line.startswith("   ") and line.endswith(":")
        ]
        needs = job.split("needs:", 1)[1].split("if:", 1)[0]
        for name in jobs:
            if name != "ci-ok":
                self.assertIn(f"- {name}\n", needs)


if __name__ == "__main__":
    unittest.main()
