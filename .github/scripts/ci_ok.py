#!/usr/bin/env python3
"""Decide the CI OK result from the results of the jobs that it needs.

A job that failed or was cancelled fails CI OK. A job that was skipped
passes: path filters and the test selection skip a job when a pull request
does not change its inputs. RESULTS holds the needs context as JSON.
"""

import json
import os
import sys

FAILED = ("failure", "cancelled")


def failed_jobs(needs):
    return sorted(name for name, job in needs.items() if job["result"] in FAILED)


def main():
    needs = json.loads(os.environ["RESULTS"])
    for name, job in needs.items():
        print(f"{name}: {job['result']}")
    failed = failed_jobs(needs)
    if failed:
        print(f"Failed or cancelled jobs: {', '.join(failed)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
