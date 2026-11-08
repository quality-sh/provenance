#!/usr/bin/env python3
"""Count the system calls of each provenance call in one `strace -f -tt -T` file.

Argument 1: strace.txt.gz. Argument 2: calls.tsv from wrap.sh. Argument 3: output JSON.
For each call (the wrapper pid and its descendants): child processes with their
arguments, fsync-family calls with their time, renames, file creations, and the
time line of the process tree (first and last line).
"""
import collections
import gzip
import json
import re
import sys

LINE = re.compile(r'^(\d+)\s+(\d\d):(\d\d):(\d\d\.\d+)\s+(.*)$')
CALL = re.compile(r'^(?:<\.\.\. )?([a-z0-9_]+)(?: resumed>|\()(.*)$')
RESULT = re.compile(r'=\s+(-?\d+|\?)(?:\s+\w+ \([^)]*\))?\s*(?:<([\d.]+)>)?\s*$')
SYNCS = ('fsync', 'fdatasync', 'syncfs', 'sync_file_range')


def main():
    trace, calls_tsv, out = sys.argv[1:4]
    argv = {}
    with open(calls_tsv) as handle:
        for line in handle:
            pid, _, rest = line.rstrip('\n').partition('\t')
            argv[int(pid)] = rest
    owner = {}
    stats = collections.defaultdict(lambda: {
        'children': [], 'syscalls': collections.Counter(), 'sync_ms': 0.0, 'syncs': [],
        'renames': [], 'creates': 0, 'first': None, 'last': None, 'wait_ms': 0.0, 'state_opens': 0,
        'pwrite64': 0, 'pread64': 0, 'copy_file_range': 0, 'timeline': []})
    parent_of = {}
    for raw in gzip.open(trace, 'rt', errors='replace'):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        c = CALL.match(m[5])
        if c and c[1] in ('clone', 'clone3', 'vfork', 'fork') and 'unfinished' not in m[5]:
            r = RESULT.search(m[5])
            if r and r[1] != '?' and int(r[1]) > 0:
                parent_of[int(r[1])] = int(m[1])

    def root_of(pid):
        seen = 0
        while pid not in argv and pid in parent_of and seen < 50:
            pid = parent_of[pid]
            seen += 1
        return pid if pid in argv else None

    for raw in gzip.open(trace, 'rt', errors='replace'):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        pid = int(m[1])
        t = int(m[2]) * 3600 + int(m[3]) * 60 + float(m[4])
        body = m[5]
        c = CALL.match(body)
        name = c[1] if c else ''
        if pid not in owner:
            owner[pid] = root_of(pid)
        root = owner[pid]
        if root is None:
            continue
        s = stats[root]
        if s['first'] is None:
            s['first'] = t
        s['last'] = t
        if name == 'execve' and pid != root and c and c[0].startswith('execve(') and '= -1' not in body:
            args = re.findall(r'"((?:[^"\\]|\\.)*)"', body)
            rel = round((t - s['first']) * 1e3, 1)
            s['children'].append({'pid': pid, 'at_ms': rel, 'argv': args[1:12]})
            s['timeline'].append((rel, 'exec ' + ' '.join(args[1:6])))
        if 'unfinished' in body:
            continue
        r = RESULT.search(body)
        dur = float(r[2]) * 1e3 if r and r[2] else 0.0
        s['syscalls'][name] += 1
        rel = round((t - s['first']) * 1e3, 1)
        if name in SYNCS:
            s['sync_ms'] += dur
            fd = re.search(r'\((\d+)<([^>]*)>', body)
            s['syncs'].append({'pid': pid, 'at_ms': rel, 'ms': round(dur, 2), 'path': fd[2] if fd else body[:120]})
            s['timeline'].append((rel, f'{name} {fd[2] if fd else ""} {dur:.2f} ms'))
        elif name.startswith('rename'):
            paths = re.findall(r'"((?:[^"\\]|\\.)*)"', body)
            s['renames'].append({'at_ms': rel, 'paths': paths})
            s['timeline'].append((rel, f'{name} {" -> ".join(paths)}'))
        elif name in ('openat', 'open') and 'O_CREAT' in body:
            s['creates'] += 1
        elif name in ('wait4', 'waitid') and pid == root:
            s['wait_ms'] += dur
        elif name in ('pwrite64', 'pread64', 'copy_file_range'):
            s[name] += 1
        if name in ('openat', 'open') and '.provenance' in body:
            s['state_opens'] += 1
        if name in ('exit_group',) and pid == root:
            s['timeline'].append((rel, 'exit_group'))
    result = []
    for root, s in stats.items():
        s['wall_ms'] = round((s['last'] - s['first']) * 1e3, 1)
        s['args'] = argv[root]
        s['pid'] = root
        s['git_processes'] = sum(1 for child in s['children'] if child['argv'] and child['argv'][0] in ('git',) or (child['argv'] and child['argv'][0].endswith('/git')))
        s['syscalls'] = dict(s['syscalls'].most_common(25))
        s['timeline'] = s['timeline'][:400]
        result.append(s)
    with open(out, 'w') as handle:
        json.dump(sorted(result, key=lambda item: item['pid']), handle, indent=1)


if __name__ == '__main__':
    main()
