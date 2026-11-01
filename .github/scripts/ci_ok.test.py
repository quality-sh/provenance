#!/usr/bin/env python3

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


SCRIPT = Path(__file__).with_name("ci_ok.py")
WORKFLOW = Path(__file__).parents[1] / "workflows" / "ci.yml"
NEXTEST = Path(__file__).parents[2] / ".config" / "nextest.toml"
SHARDS = "test-default-features-shard"


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


class ShardDecisionTests(unittest.TestCase):
    """The Default-features CLI test check runs ci_ok.py over its shards."""

    def test_passing_shards_pass(self):
        result = run(changes="success", **{SHARDS: "success"})
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_skipped_shards_pass(self):
        result = run(changes="success", **{SHARDS: "skipped"})
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_failed_or_cancelled_shard_fails(self):
        for outcome in ("failure", "cancelled"):
            with self.subTest(outcome=outcome):
                result = run(changes="success", **{SHARDS: outcome})
                self.assertEqual(result.returncode, 1)
                self.assertIn(SHARDS, result.stderr)

    def test_ci_ok_fails_when_a_shard_fails(self):
        result = run(
            changes="success",
            test="success",
            **{SHARDS: "failure", "test-default-features": "failure"},
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn(f"test-default-features, {SHARDS}", result.stderr)


class WorkflowWiringTests(unittest.TestCase):
    def job(self, name):
        return WORKFLOW.read_text().split(f"\n  {name}:\n", 1)[1].split("\n\n  ", 1)[0]

    def test_the_required_check_needs_every_shard(self):
        job = self.job("test-default-features")
        self.assertIn("name: Default-features CLI test\n", job)
        self.assertIn(f"needs: [changes, {SHARDS}]", job)
        self.assertIn("if: always()", job)
        self.assertIn("RESULTS: ${{ toJSON(needs) }}", job)
        self.assertIn("python3 .github/scripts/ci_ok.py", job)

    def test_the_shards_run_every_cli_test(self):
        # The ci profile leaves out the distribution test, so the other shard
        # runs everything else and the distribution shard runs only that test.
        self.assertIn("default-filter = 'not binary(cli_skills_single_source)'", NEXTEST.read_text())
        job = self.job(SHARDS)
        self.assertIn("fail-fast: false", job)
        self.assertIn(
            "- shard: distribution\n"
            "            tests: --ignore-default-filter --test cli_skills_single_source\n",
            job,
        )
        self.assertIn('- shard: other\n            tests: ""\n', job)
        self.assertIn(
            'cargo nextest run --profile ci -p provenance-cli --no-fail-fast --timings "${tests[@]}"',
            job,
        )
        self.assertIn("cargo test -p provenance-cli --doc --no-fail-fast", job)

    def test_the_shards_stop_their_tests_before_the_job_timeout(self):
        job = self.job(SHARDS)
        job_minutes = int(job.split("timeout-minutes: ", 1)[1].split("\n", 1)[0])
        shard_minutes = [int(line.split(":")[1]) for line in job.splitlines() if line.strip().startswith("minutes:")]
        self.assertEqual(len(shard_minutes), 2)
        for minutes in shard_minutes:
            # Setup, the one-minute kill grace and the report steps fit in the rest.
            self.assertLessEqual(minutes + 10, job_minutes)
        self.assertIn('timeout --kill-after=1m "${MINUTES}m"', job)
        self.assertIn("::error title=Default-features CLI test timeout::", job)

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
