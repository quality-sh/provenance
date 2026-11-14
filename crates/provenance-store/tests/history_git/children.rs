//! The Git child processes of the test process.

/// The ids of the Git child processes of the test process. On Unix a child
/// that ended and that the test process did not wait for is still listed.
pub fn git_children() -> Vec<u32> {
    let test = std::process::id();
    processes()
        .into_iter()
        .filter(|(_, parent, name)| {
            *parent == test
                && std::path::Path::new(name)
                    .file_stem()
                    .is_some_and(|stem| stem.eq_ignore_ascii_case("git"))
        })
        .map(|(id, _, _)| id)
        .collect()
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
    // SAFETY: the entry is a plain C structure with its size set, and the
    // snapshot handle is closed once.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        assert_ne!(snapshot, INVALID_HANDLE_VALUE, "no process snapshot");
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap();
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
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
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    found
}
