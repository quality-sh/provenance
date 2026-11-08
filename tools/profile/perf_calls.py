#!/usr/bin/env python3
"""Split `perf script` output into one wall-time profile per provenance call.

Input on stdin: perf script -F comm,pid,tid,time,event,ip,sym,dso --show-task-events
Argument 1: calls.tsv from wrap.sh (pid TAB arguments). Argument 2: output directory.

For each thread, the time from one event to the next goes to the stack of the
first event: on CPU after a cpu-clock sample, blocked after a context switch.
The main thread's intervals add up to its wall time.

Writes filtered.txt.gz (the events of the provenance process trees only),
calls.json (one entry for each call) and collapsed/<key>.<part>.txt.
"""
import collections
import gzip
import json
import os
import re
import sys

HEADER = re.compile(r'^(?P<comm>.+?)\s+(?P<pid>-?\d+)/(?P<tid>-?\d+)\s+(?:\[\d+\]\s+)?(?P<time>\d+\.\d+):\s+(?P<rest>.*)$')
FORK = re.compile(r'PERF_RECORD_FORK\((\d+):(\d+)\):\((\d+):(\d+)\)')
EXIT = re.compile(r'PERF_RECORD_EXIT\((\d+):(\d+)\):\((\d+):(\d+)\)')
COMM = re.compile(r'PERF_RECORD_COMM(?: exec)?: (.+):(\d+)/(\d+)')
HASH = re.compile(r'::h[0-9a-f]{16}$')


def clean(sym, dso):
    if sym in ('[unknown]', ''):
        base = os.path.basename(dso.strip('()')) if dso else '?'
        return f'[{base}]'
    sym = HASH.sub('', sym)
    if 'kernel' in dso:
        return sym + '_[k]'
    return sym


def id_shape(args):
    """A call key with the record ids taken out."""
    if not args:
        return 'provenance'
    first = args[0]
    if first == 'api' and len(args) > 1:
        parts = args[1].split('/')
        shaped = [p if i % 2 == 1 or i == 0 else '{id}' for i, p in enumerate(parts)]
        method = 'GET'
        if '--method' in args:
            method = args[args.index('--method') + 1]
        return f'api {method} {"/".join(shaped)}'
    if first in ('review', 'mcp'):
        return f'{first} host'
    if len(args) > 1 and not first.startswith('-') and re.match(r'^[a-z]+$', args[1]) and first not in (
            'check', 'report', 'skills', 'schema', 'validate', 'materialize', 'search', 'export', 'import', 'init', 'prime', 'api'):
        return f'<id> {args[1]}'
    words = [a for a in args if not a.startswith('/') and not a.startswith('{')][:4]
    keep = []
    for word in words:
        if re.match(r'^[0-9a-f]{40}$', word):
            word = '{sha}'
        keep.append(word)
    return ' '.join(keep)


def main():
    calls_tsv, out = sys.argv[1], sys.argv[2]
    os.makedirs(os.path.join(out, 'collapsed'), exist_ok=True)
    argv = {}
    with open(calls_tsv) as handle:
        for line in handle:
            pid, _, rest = line.rstrip('\n').partition('\t')
            argv[int(pid)] = rest.split(' ') if rest else []
    owner = {}          # pid -> root call pid
    parent = {}
    thread_of = {}      # tid -> pid
    exec_time = {}      # pid -> time of exec of provenance
    start = {}          # pid -> fork time
    proc_comm = collections.defaultdict(list)  # pid -> comms exec'd
    exit_time = {}      # tid -> time
    last = {}           # tid -> (time, kind, stack)
    # per root: part -> Counter(stack -> microseconds)
    prof = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    stats = collections.defaultdict(lambda: collections.Counter())
    children = collections.defaultdict(set)
    filtered = gzip.open(os.path.join(out, 'filtered.txt.gz'), 'wt')

    def part_for(root, pid, tid, offcpu):
        if pid != root:
            return 'child'
        if tid == pid:
            return 'main'
        return 'threads'

    def flush(tid, now):
        prev = last.get(tid)
        if not prev:
            return
        then, kind, stack, pid, comm = prev
        root = owner.get(pid)
        if root is None:
            return
        us = int(round((now - then) * 1e6))
        if us <= 0:
            return
        offcpu = kind == 'cs'
        frames = list(reversed(stack))
        if pid == root and tid == pid and not comm.startswith('provenance'):
            frames = ['[wrapper sh before exec]']
        if pid != root:
            frames = [f'[{comm} {pid}]'] + frames
        leaf = '[blocked]' if offcpu else '[on-cpu]'
        key = ';'.join(frames + [leaf])
        part = part_for(root, pid, tid, offcpu)
        prof[root][part][key] += us
        stats[root][f'{part}_{"off" if offcpu else "on"}_us'] += us

    event_lines = []
    header = None

    def finish_event():
        nonlocal header, event_lines
        if header is None:
            return
        comm, pid, tid, t, rest = header
        stack = []
        for frame in event_lines:
            body = frame.strip()
            addr, _, symdso = body.partition(' ')
            sym, _, dso = symdso.rpartition(' (')
            stack.append(clean(sym.strip() or symdso, '(' + dso))
        kind = 'cs' if 'context-switches' in rest or rest.startswith('cs') else 'sample'
        if pid in owner:
            flush(tid, t)
            last[tid] = (t, kind, stack, pid, comm)
        header = None
        event_lines = []

    for raw in sys.stdin:
        line = raw.rstrip('\n')
        if not line:
            finish_event()
            continue
        if line.startswith((' ', '\t')):
            if header is not None:
                event_lines.append(line)
                if header[1] in owner:
                    filtered.write(raw)
            continue
        finish_event()
        m = HEADER.match(line)
        if not m:
            continue
        comm, pid, tid, t, rest = m['comm'].strip(), int(m['pid']), int(m['tid']), float(m['time']), m['rest']
        if 'PERF_RECORD_' in rest:
            f = FORK.search(rest)
            if f:
                cpid, ctid, ppid, ptid = map(int, f.groups())
                if cpid == ppid:
                    thread_of[ctid] = cpid
                    if cpid in owner:
                        last[ctid] = (t, 'sample', ['[new thread]'], cpid, comm)
                else:
                    parent[cpid] = ppid
                    start[cpid] = t
                    if cpid in argv:
                        owner[cpid] = cpid
                        stats[cpid]['fork_time'] = t
                        last[ctid] = (t, 'sample', ['[fork to exec]'], cpid, 'sh')
                    elif ppid in owner:
                        owner[cpid] = owner[ppid]
                        children[owner[ppid]].add(cpid)
                        last[ctid] = (t, 'sample', ['[forked]'], cpid, comm)
            c = COMM.search(rest)
            if c and pid in owner:
                proc_comm[pid].append(c.group(1))
                if pid == owner[pid] and c.group(1).startswith('provenance'):
                    exec_time[pid] = t
            e = EXIT.search(rest)
            if e and pid in owner:
                flush(tid, t)
                last.pop(tid, None)
                exit_time[tid] = t
            if pid in owner:
                filtered.write(raw)
            continue
        header = (comm, pid, tid, t, rest)
        if pid in owner:
            filtered.write(raw)
    finish_event()
    filtered.close()

    calls = []
    for root in sorted(owner.keys() & argv.keys()):
        if root not in stats:
            continue
        key = id_shape(argv[root])
        execs = collections.Counter()
        for child in children[root]:
            names = proc_comm.get(child) or ['?']
            execs[names[-1]] += 1
        s = stats[root]
        wall = (exit_time.get(root, 0) - s['fork_time']) * 1e3 if root in exit_time else None
        calls.append({
            'pid': root, 'key': key, 'args': argv[root], 'wall_ms': wall,
            'wrapper_ms': ((exec_time.get(root, s['fork_time']) - s['fork_time']) * 1e3),
            'main_on_ms': s['main_on_us'] / 1e3, 'main_off_ms': s['main_off_us'] / 1e3,
            'threads_on_ms': s['threads_on_us'] / 1e3, 'threads_off_ms': s['threads_off_us'] / 1e3,
            'child_on_ms': s['child_on_us'] / 1e3, 'child_processes': dict(execs),
        })
    with open(os.path.join(out, 'calls.json'), 'w') as handle:
        json.dump(calls, handle, indent=1)
    by_key = collections.defaultdict(list)
    for call in calls:
        by_key[call['key']].append(call['pid'])
    for key, pids in by_key.items():
        safe = re.sub(r'[^A-Za-z0-9_.-]+', '_', key)[:120]
        for part in ('main', 'threads', 'child'):
            total = collections.Counter()
            for pid in pids:
                total.update(prof[pid][part])
            if not total:
                continue
            with open(os.path.join(out, 'collapsed', f'{safe}.{part}.txt'), 'w') as handle:
                handle.write(f'# key={key} calls={len(pids)} unit=microseconds summed over calls\n')
                for stack, us in total.most_common():
                    handle.write(f'{stack} {us}\n')


if __name__ == '__main__':
    main()
