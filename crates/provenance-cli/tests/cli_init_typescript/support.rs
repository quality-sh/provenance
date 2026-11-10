//! The package manager table and the project helpers of the TypeScript init
//! tests.

use std::path::Path;

/// The bin file of the project package, relative to the directory that
/// contains `node_modules`. It is a macro so that `concat!` can make each
/// command a `&'static str`.
macro_rules! package_bin {
    () => {
        "node_modules/@quality-sh/provenance/bin/provenance.mjs"
    };
}

/// The bin file of the project package.
pub const PACKAGE_BIN: &str = package_bin!();

/// The package manager, the executable that the test looks for on PATH, and
/// the command that runs the project package through that package manager.
pub const MANAGERS: [(&str, &str, &str); 6] = [
    ("npm", "node", concat!("node ", package_bin!())),
    ("pnpm", "pnpm", "pnpm exec ./node_modules/.bin/provenance"),
    ("yarn", "yarn", "yarn run -B provenance"),
    ("bun", "bun", concat!("bun ", package_bin!())),
    (
        "deno",
        "deno",
        "deno task --node-modules-dir=manual --eval \"./node_modules/.bin/provenance\"",
    ),
    ("nub", "nub", concat!("nub ", package_bin!())),
];

pub fn write_package_json(directory: &Path) {
    std::fs::write(
        directory.join("package.json"),
        "{\"name\":\"project\",\"version\":\"1.0.0\",\"private\":true}\n",
    )
    .unwrap();
}

pub fn on_path(executable: &str) -> bool {
    std::process::Command::new(executable)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn shell(directory: &Path, command: &str) -> std::process::Command {
    let mut shell = std::process::Command::new("sh");
    shell
        .args(["-c", command])
        .current_dir(directory)
        .env("npm_config_update_notifier", "false")
        .stdin(std::process::Stdio::null());
    shell
}

/// Puts the CLI under test where the package manager installs the package
/// binary, and a bin file that starts it where the package has its bin file.
#[cfg(unix)]
pub fn link_project_binary(repo: &Path) {
    let cli = std::path::PathBuf::from(assert_cmd::cargo::cargo_bin!("provenance"));
    std::fs::create_dir_all(repo.join("node_modules/.bin")).unwrap();
    std::os::unix::fs::symlink(&cli, repo.join("node_modules/.bin/provenance")).unwrap();
    let bin = repo.join(PACKAGE_BIN);
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(
        bin,
        format!(
            "import {{ spawnSync }} from \"node:child_process\";\n\
             const result = spawnSync({:?}, process.argv.slice(2), {{ stdio: \"inherit\" }});\n\
             process.exit(result.status ?? 1);\n",
            cli.display().to_string()
        ),
    )
    .unwrap();
}

/// This host does not run the commands, so the project needs no binary.
#[cfg(not(unix))]
pub fn link_project_binary(_repo: &Path) {}
