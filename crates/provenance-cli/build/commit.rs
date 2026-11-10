//! Build identity: the release number, followed by the commit when the build
//! comes from a checkout of this repository. A failure to find the commit never
//! stops the build; the identity is then the release number alone.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::inputs;

/// Gives `PROVENANCE_BUILD` to the compiler, for example `0.4.0 (6347b4b0)`.
/// A build with changes to tracked build inputs adds `-dirty` to the commit.
pub fn publish(crate_root: &Path) {
    let release = std::env::var("CARGO_PKG_VERSION").expect("Cargo package version");
    let build = match checkout_root(crate_root).and_then(|root| commit(&root)) {
        Some(commit) => format!("{release} ({commit})"),
        None => release,
    };
    println!("cargo:rustc-env=PROVENANCE_BUILD={build}");
}

/// The workspace root that holds this crate in a checkout. A package build
/// from crates.io has no such root that git names.
fn checkout_root(crate_root: &Path) -> Option<PathBuf> {
    Some(crate_root.parent()?.parent()?.to_path_buf())
}

/// Compares two paths after symbolic links and letter case are resolved.
fn same_place(first: &Path, second: &Path) -> bool {
    match (first.canonicalize(), second.canonicalize()) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

/// The commit of the checkout, with `-dirty` when a path in the build input
/// list differs from that commit. The same list sets when the script runs
/// again, so the mark cannot stay from an earlier state of these paths.
fn commit(root: &Path) -> Option<String> {
    let top = git(root, &["rev-parse", "--show-toplevel"])?;
    if !same_place(Path::new(&top), root) {
        return None;
    }
    let inputs = inputs::paths(root, |arguments| git(root, arguments));
    watch(root, &inputs);
    let head = short(&git(root, &["rev-parse", "HEAD"])?)?;
    // Without optional locks, git diff does not write the index.
    let mut arguments = vec![
        "--no-optional-locks",
        "--literal-pathspecs",
        "diff",
        "--name-only",
        "HEAD",
        "--",
    ];
    arguments.extend(inputs.iter().map(String::as_str));
    let changes = git(root, &arguments)?;
    Some(if changes.is_empty() {
        head
    } else {
        format!("{head}-dirty")
    })
}

/// The first eight characters of a full commit hash. A fixed length keeps the
/// identity the same for one commit in every clone.
fn short(hash: &str) -> Option<String> {
    let full = hash.len() >= 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    full.then(|| hash[..8].to_ascii_lowercase())
}

/// Runs the build script again when the commit moves or a build input changes.
/// The HEAD reflog changes on each commit, amend, checkout, and reset, also when
/// the branch ref is only in `packed-refs` and a commit writes a new loose ref
/// file. A path that does not exist is not watched, because Cargo then runs the
/// script on every build.
fn watch(root: &Path, inputs: &[String]) {
    let mut names = vec![
        "HEAD".to_owned(),
        "packed-refs".to_owned(),
        "logs/HEAD".to_owned(),
    ];
    names.extend(git(root, &["symbolic-ref", "-q", "HEAD"]));
    for name in names {
        if let Some(path) = git(root, &["rev-parse", "--git-path", &name]) {
            let path = root.join(path);
            if path.is_file() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    for input in inputs {
        println!(
            "cargo:rerun-if-changed={}",
            inputs::absolute(root, input).display()
        );
    }
}

/// Runs git like `provenance_store::git_process::git`, which a build script cannot
/// use: on Windows the command opens no console window.
fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.arg("-C").arg(root).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}
