//! Finds the test files that are in no test target, or in more than one.
//!
//! The module tree of each test target comes from its root file: each
//! `mod name;` and `#[path = "..."]` declaration, with the directory rules of
//! rustc. A file whose path under a `tests` directory has a `support`
//! directory or a name that ends in `_support` is shared support. Only
//! shared support can load in more than one target, and it declares no tests.

use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

/// Reads `cargo metadata --no-deps` for the manifest in `directory`.
pub fn metadata(directory: &Path) -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(directory)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed in {}: {}",
        directory.display(),
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
}

/// Gives the layout faults of the workspace in `metadata`, and the number of
/// top-level test files that it examined.
pub fn layout_faults(metadata: &Value) -> (Vec<String>, usize) {
    let root = PathBuf::from(metadata["workspace_root"].as_str().expect("workspace root"));
    let mut owners = BTreeMap::<PathBuf, Vec<String>>::new();
    let mut top_level = Vec::new();
    for package in metadata["packages"].as_array().expect("metadata packages") {
        let name = package["name"].as_str().expect("package name");
        let manifest = PathBuf::from(package["manifest_path"].as_str().expect("manifest path"));
        let tests = manifest.parent().expect("crate directory").join("tests");
        for target in package["targets"].as_array().expect("package targets") {
            if target["kind"][0] != "test" {
                continue;
            }
            let target_name = format!("{name}::{}", target["name"].as_str().expect("target name"));
            let mut files = Vec::new();
            load(
                &normal(Path::new(target["src_path"].as_str().expect("target path"))),
                true,
                &mut Vec::new(),
                &mut files,
            );
            for file in files {
                owners.entry(file).or_default().push(target_name.clone());
            }
        }
        if let Ok(entries) = fs::read_dir(&tests) {
            for entry in entries {
                let path = normal(&entry.expect("read tests entry").path());
                if path.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
                    top_level.push((path, name.to_owned()));
                }
            }
        }
    }
    let mut faults = Vec::new();
    for (path, package) in &top_level {
        if !owners.contains_key(path) {
            faults.push(format!(
                "{} is in no test target of {package}: add it to a group main.rs or as a [[test]]",
                shown(path, &root),
            ));
        }
    }
    for (path, targets) in &owners {
        let count = targets.len();
        if count < 2 {
            continue;
        }
        let names = targets.join(", ");
        if !is_support(path) {
            faults.push(format!(
                "{} loads {count} times ({names}), so its tests run {count} times: load it \
                 once, or make it shared support (a support directory or a _support name) \
                 that declares no tests",
                shown(path, &root),
            ));
        } else if declares_tests(path) {
            faults.push(format!(
                "{} is shared support that loads {count} times ({names}) and declares tests, \
                 so they run {count} times: move the tests to a file that loads once",
                shown(path, &root),
            ));
        }
    }
    (faults, top_level.len())
}

/// Adds `path` and each file that its `mod` declarations load to `files`, one
/// entry for each load.
///
/// A crate root, a `mod.rs` file and a file that a `#[path]` attribute names
/// own their directory. Another file `name.rs` owns the directory `name/`.
/// `ancestors` stops a module cycle, which rustc refuses.
fn load(path: &Path, owns_directory: bool, ancestors: &mut Vec<PathBuf>, files: &mut Vec<PathBuf>) {
    if ancestors.iter().any(|ancestor| ancestor == path) {
        return;
    }
    files.push(path.to_owned());
    let Some(syntax) = parse(path) else {
        return;
    };
    ancestors.push(path.to_owned());
    let directory = path.parent().expect("source file directory");
    let file_directory = if owns_directory {
        directory.to_owned()
    } else {
        directory.join(path.file_stem().expect("source file stem"))
    };
    load_items(&syntax.items, directory, &file_directory, ancestors, files);
    ancestors.pop();
}

/// Follows the `mod` items of one module.
///
/// `attribute_directory` is where a `#[path]` value starts. `module_directory`
/// is where a `mod name;` without a `#[path]` attribute looks for its file.
fn load_items(
    items: &[syn::Item],
    attribute_directory: &Path,
    module_directory: &Path,
    ancestors: &mut Vec<PathBuf>,
    files: &mut Vec<PathBuf>,
) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let name = module.ident.to_string();
        let path_value = path_attribute(&module.attrs);
        match (&module.content, path_value) {
            (Some((_, inner)), Some(value)) => {
                let directory = normal(&attribute_directory.join(value));
                load_items(inner, &directory, &directory, ancestors, files);
            }
            (Some((_, inner)), None) => {
                let directory = module_directory.join(&name);
                load_items(inner, &directory, &directory, ancestors, files);
            }
            (None, Some(value)) => load(
                &normal(&attribute_directory.join(value)),
                true,
                ancestors,
                files,
            ),
            (None, None) => {
                let file = module_directory.join(format!("{name}.rs"));
                let directory_file = module_directory.join(&name).join("mod.rs");
                if file.is_file() {
                    load(&normal(&file), false, ancestors, files);
                } else if directory_file.is_file() {
                    load(&normal(&directory_file), true, ancestors, files);
                }
            }
        }
    }
}

/// Parses `path` as a Rust file, or gives `None` when it does not exist.
fn parse(path: &Path) -> Option<syn::File> {
    let text = fs::read_to_string(path).ok()?;
    Some(syn::parse_file(&text).unwrap_or_else(|error| {
        panic!("parse {}: {error}", path.display());
    }))
}

/// Gives the value of a `#[path = "..."]` attribute.
fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| {
        let syn::Meta::NameValue(meta) = &attribute.meta else {
            return None;
        };
        if !meta.path.is_ident("path") {
            return None;
        }
        match &meta.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => Some(value.value()),
            _ => None,
        }
    })
}

/// Tells if the path of `file` under its `tests` directory names it as
/// shared support.
fn is_support(file: &Path) -> bool {
    let components = file
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let Some(tests) = components
        .iter()
        .rposition(|component| component == "tests")
    else {
        return false;
    };
    components[tests + 1..].iter().any(|component| {
        let stem = component.strip_suffix(".rs").unwrap_or(component);
        stem == "support" || stem.ends_with("_support")
    })
}

/// Tells if `file` declares a test function, in the file or in an inline module.
fn declares_tests(file: &Path) -> bool {
    parse(file).is_some_and(|syntax| items_declare_tests(&syntax.items))
}

fn items_declare_tests(items: &[syn::Item]) -> bool {
    items.iter().any(|item| match item {
        syn::Item::Fn(function) => function.attrs.iter().any(|attribute| {
            attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
        }),
        syn::Item::Mod(module) => module
            .content
            .as_ref()
            .is_some_and(|(_, inner)| items_declare_tests(inner)),
        _ => false,
    })
}

/// Removes `.` and `..` components without a read of the file system.
fn normal(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    result
}

/// Shows `path` from the workspace root, with `/` between components.
fn shown(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
