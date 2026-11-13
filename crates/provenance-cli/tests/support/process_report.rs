//! A report of what a process waits on, read from `/proc` on Linux.
//!
//! The parent of a process can read the wait channel and the system call of
//! each of its threads. A thread in `flock` waits on a file lock, in a futex
//! on a mutex, a channel or a thread join, in `do_wait` on a child process,
//! and in `pipe_write` on a full pipe. The lock table names the process that
//! holds each lock on a file that the process has open.

/// The report of process `pid` and of its child processes.
#[cfg(target_os = "linux")]
pub fn report(pid: u32) -> String {
    let mut text = String::new();
    let mut inodes = Vec::new();
    describe(pid, 0, &mut text, &mut inodes);
    text.push_str(&locks(&inodes));
    text
}

#[cfg(not(target_os = "linux"))]
pub fn report(pid: u32) -> String {
    format!("process {pid}: this platform gives no process report\n")
}

#[cfg(target_os = "linux")]
fn describe(pid: u32, depth: usize, text: &mut String, inodes: &mut Vec<u64>) {
    use std::fmt::Write as _;
    use std::os::unix::fs::MetadataExt as _;
    let indent = "  ".repeat(depth);
    let directory = format!("/proc/{pid}");
    let _ = writeln!(text, "{indent}process {pid}: {}", command_line(pid));
    let mut children = Vec::new();
    for task in entries(&format!("{directory}/task")) {
        let path = format!("{directory}/task/{task}");
        let name = read(&format!("{path}/comm"));
        let stat = read(&format!("{path}/stat"));
        let state = stat
            .rsplit_once(") ")
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .unwrap_or("?");
        let channel = read(&format!("{path}/wchan"));
        let call = read(&format!("{path}/syscall"));
        let call = call.split_whitespace().next().unwrap_or("?");
        let _ = writeln!(
            text,
            "{indent}  thread {task} {name}: state {state}, wait channel {channel}, system call {call}"
        );
        children.extend(
            read(&format!("{path}/children"))
                .split_whitespace()
                .filter_map(|child| child.parse::<u32>().ok()),
        );
    }
    for descriptor in entries(&format!("{directory}/fd")) {
        let path = format!("{directory}/fd/{descriptor}");
        let target = std::fs::read_link(&path).map_or_else(
            |error| error.to_string(),
            |target| target.display().to_string(),
        );
        if let Ok(metadata) = std::fs::metadata(&path) {
            inodes.push(metadata.ino());
        }
        let _ = writeln!(text, "{indent}  descriptor {descriptor}: {target}");
    }
    for child in children {
        if depth < 3 {
            describe(child, depth + 1, text, inodes);
        }
    }
}

/// The lines of the kernel lock table on the inodes of the open files, each
/// with the command of the process that holds or waits for the lock.
#[cfg(target_os = "linux")]
fn locks(inodes: &[u64]) -> String {
    use std::fmt::Write as _;
    let mut text = String::from("locks on the open files:\n");
    for line in read("/proc/locks").lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(at) = fields
            .iter()
            .position(|field| field.matches(':').count() == 2)
        else {
            continue;
        };
        let inode = fields[at]
            .rsplit(':')
            .next()
            .and_then(|inode| inode.parse().ok());
        if !inode.is_some_and(|inode| inodes.contains(&inode)) {
            continue;
        }
        let holder = fields[at - 1].parse::<u32>().map_or_else(
            |_| "?".to_owned(),
            |holder| format!("process {holder}: {}", command_line(holder)),
        );
        let _ = writeln!(text, "  {line} ({holder})");
    }
    text
}

#[cfg(target_os = "linux")]
fn command_line(pid: u32) -> String {
    read(&format!("/proc/{pid}/cmdline")).replace('\0', " ")
}

#[cfg(target_os = "linux")]
fn entries(directory: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory).map_or_default(|entries| {
        entries
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .collect()
    });
    names.sort_by_key(|name| name.parse::<u64>().unwrap_or(u64::MAX));
    names
}

#[cfg(target_os = "linux")]
fn read(path: &str) -> String {
    std::fs::read(path).map_or_else(
        |error| format!("({error})"),
        |bytes| String::from_utf8_lossy(&bytes).trim().to_owned(),
    )
}
