#!/bin/sh
# Pass-through wrapper: log the pid and the arguments, then exec the real
# binary. exec keeps the pid, so the profile joins on it.
printf '%s\t%s\n' "$$" "$*" >> "$PROF_LOG"
exec "$PROF_REAL" "$@"
