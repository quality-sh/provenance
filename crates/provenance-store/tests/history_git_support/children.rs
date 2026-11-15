//! The Git child processes of the test process.

/// The ids of the Git child processes of the test process. On Unix a child
/// that ended and that the test process did not wait for is still listed.
pub fn git_children() -> Vec<u32> {
    let test = std::process::id();
    processes()
        .into_iter()
        .filter(|(id, parent, name)| {
            *parent == test
                && std::path::Path::new(name)
                    .file_stem()
                    .is_some_and(|stem| stem.eq_ignore_ascii_case("git"))
                && started_after_test(*id)
        })
        .map(|(id, _, _)| id)
        .collect()
}

/// On Unix a process whose parent ends gets a new parent, so the parent id
/// is enough.
#[cfg(unix)]
const fn started_after_test(_: u32) -> bool {
    true
}

/// Whether the process `id` started after the test process. On Windows a
/// process keeps the parent id after its parent ends, and Windows gives that
/// id to a new process again. A Git process of an ended process that had
/// the id of the test process started before the test process. A process
/// that the test cannot open is not listed.
#[cfg(windows)]
fn started_after_test(id: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the call takes no pointer, and a zero handle is a failure.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id) };
    if process == 0 {
        return false;
    }
    let started = creation_time(process);
    // SAFETY: `process` is an open handle, and it is closed once, here.
    unsafe { CloseHandle(process) };
    // SAFETY: the call takes no pointer and gives a handle that needs no close.
    let test = creation_time(unsafe { GetCurrentProcess() });
    started
        .zip(test)
        .is_some_and(|(started, test)| started >= test)
}

/// The creation time of a process, in units of 100 ns.
#[cfg(windows)]
fn creation_time(process: windows_sys::Win32::Foundation::HANDLE) -> Option<u64> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    let empty = || FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (empty(), empty(), empty(), empty());
    // SAFETY: `process` is a handle with query access, and each pointer is
    // to a `FILETIME` of this function.
    let done =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    (done != 0)
        .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// Each process with its id, the id of its parent and its program name.
#[cfg(unix)]
fn processes() -> Vec<(u32, u32, String)> {
    let output = std::process::Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,comm="])
        .output()
        .unwrap();
    assert!(output.status.success(), "ps failed: {output:?}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (id, rest) = line.trim_start().split_once(' ')?;
            let (parent, name) = rest.trim_start().split_once(' ')?;
            Some((
                id.parse().ok()?,
                parent.parse().ok()?,
                name.trim().to_owned(),
            ))
        })
        .collect()
}

/// Each process with its id, the id of its parent and its program name.
#[cfg(windows)]
fn processes() -> Vec<(u32, u32, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut found = Vec::new();
    // SAFETY: the call takes no pointer.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    assert_ne!(snapshot, INVALID_HANDLE_VALUE, "no process snapshot");
    // SAFETY: the entry holds only integers and arrays of integers, so zero
    // bytes are a valid value.
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap();
    // SAFETY: `snapshot` is open, and `entry` is an entry with its size set.
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more {
        let name = &entry.szExeFile;
        let length = name
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(name.len());
        found.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            String::from_utf16_lossy(&name[..length]),
        ));
        // SAFETY: as for `Process32FirstW`.
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    // SAFETY: `snapshot` is open, and it is closed once, here.
    unsafe { CloseHandle(snapshot) };
    found
}
