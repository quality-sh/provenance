#!/bin/sh
# Pass-through wrapper for the API latency harness. It logs the pid, the row
# label and the arguments. When PROF_MODE is set and the row label matches
# PROF_MATCH, it runs the real binary under perf or strace; else it execs it.
printf '%s\t%s\t%s\n' "$$" "${PROF_LABEL:-}" "$*" >> "$PROF_LOG"
if [ -n "${PROF_MODE:-}" ] && [ -n "${PROF_LABEL:-}" ] && printf '%s' "$PROF_LABEL" | grep -Eq "$PROF_MATCH"; then
  # A command row also makes setup calls through `provenance api`; only route rows profile those.
  if [ "$1" = api ]; then
    case "$PROF_LABEL" in GET\ *|POST\ *|PATCH\ *) ;; *) exec "$PROF_REAL" "$@" ;; esac
  fi
  printf '%s\t%s\t%s\n' "$$" "$PROF_LABEL" "$*" >> "$PROF_DIR/profiled.tsv"
  case "$PROF_MODE" in
    perf) exec perf record -q -k mono -g -e cpu-clock/freq=1999/ -e context-switches/period=1/ \
            -o "$PROF_DIR/$$.data" -- "$PROF_REAL" "$@" ;;
    strace) exec strace -f -qq -tt -T -yy -s 400 -o "$PROF_DIR/$$.strace" \
            -e trace=%process,%file,fsync,fdatasync,syncfs,sync_file_range,copy_file_range,pread64,pwrite64,read,write,flock,fcntl \
            "$PROF_REAL" "$@" ;;
  esac
fi
exec "$PROF_REAL" "$@"
