//! Finds the test files that are in no test target, or in more than one.
//!
//! The module tree of each test target comes from its root file: each
//! `mod name;` and `#[path = "..."]` declaration, with the directory rules of
//! rustc. Each file is parsed once. A file whose path under a `tests`
//! directory has a `support` directory or a name that ends in `_support` is
//! shared support. Only shared support can load in more than one target, and
//! it declares no tests.
//!
//! Each `tests/<group>/main.rs` must be a test target. Each other `.rs` file
//! directly in `tests/` must load in a test target. A `.rs` file deeper in
//! `tests/` must load in a test target only when `syn` parses it and it
//! declares a test function, so data files, compile-fail inputs and files
//! that do not parse can stay there.
//!
//! Sibling `mod` items with one name can only be `#[cfg]` alternatives, as
//! rustc refuses two of them in one build. A file that more than one of them
//! loads counts once.
//!
//! Limits:
//!
//! - A `#[cfg]` module counts as loaded. Two `#[cfg]` alternatives with
//!   different names, or at different levels, that load one file give a false
//!   "loads 2 times" fault.
//! - Tests that a macro makes, for example a `macro_rules!` that expands to
//!   `#[test] fn` or a `proptest!` block, are not seen. Shared support that
//!   makes tests in this way and loads in two targets is not reported.
//! - Only targets of kind `test` are examined. A `[[bench]]` or `[[example]]`
//!   file under `tests/` gives a false "in no test target" or "not a test
//!   target" fault. A `src/` file that loads a test file with `#[path]` is not
//!   seen, so a double load from it is not reported.
//! - A `[[test]]` with `harness = false` counts as a target that runs its
//!   `#[test]` functions, and libtest does not run them. A target with
//!   `required-features` counts as running, also when no CI job turns on
//!   those features.

use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    rc::Rc,
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
/// `.rs` files under `tests` directories that it examined.
pub fn layout_faults(metadata: &Value) -> (Vec<String>, usize) {
    let root = PathBuf::from(metadata["workspace_root"].as_str().expect("workspace root"));
    let mut sources = Sources::default();
    let mut owners = BTreeMap::<PathBuf, Vec<String>>::new();
    let mut faults = Vec::new();
    let mut examined = Vec::new();
    for package in metadata["packages"].as_array().expect("metadata packages") {
        let name = package["name"].as_str().expect("package name");
        let manifest = PathBuf::from(package["manifest_path"].as_str().expect("manifest path"));
        let tests = manifest.parent().expect("crate directory").join("tests");
        let mut target_roots = BTreeSet::new();
        for target in package["targets"].as_array().expect("package targets") {
            if target["kind"][0] != "test" {
                continue;
            }
            let target_name = format!("{name}::{}", target["name"].as_str().expect("target name"));
            let target_root = normal(Path::new(target["src_path"].as_str().expect("target path")));
            let mut files = Vec::new();
            sources.load(&target_root, true, &mut Vec::new(), &mut files);
            for file in files {
                owners.entry(file).or_default().push(target_name.clone());
            }
            target_roots.insert(target_root);
        }
        let mut found = Vec::new();
        rust_files(&tests, 0, &mut found);
        for (path, depth) in found {
            if depth == 1 && path.file_name().is_some_and(|file| file == "main.rs") {
                if !target_roots.contains(&path) {
                    faults.push(format!(
                        "{} is not a test target of {name}: add it as a [[test]]",
                        shown(&path, &root),
                    ));
                }
                continue;
            }
            examined.push((path, depth, name.to_owned()));
        }
    }
    for (path, depth, package) in &examined {
        if owners.contains_key(path) {
            continue;
        }
        if *depth == 0 {
            faults.push(format!(
                "{} is in no test target of {package}: add it to a group main.rs or as a [[test]]",
                shown(path, &root),
            ));
        } else if unloaded_file_declares_tests(path) {
            faults.push(format!(
                "{} declares tests and is in no test target of {package}: load it as a \
                 module of a test target",
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
        } else if sources.declares_tests(path) {
            faults.push(format!(
                "{} is shared support that loads {count} times ({names}) and declares tests, \
                 so they run {count} times: move the tests to a file that loads once",
                shown(path, &root),
            ));
        }
    }
    (faults, examined.len())
}

/// Adds each `.rs` file under `directory` to `files`, with its depth below
/// `directory`, in path order.
fn rust_files(directory: &Path, depth: usize, files: &mut Vec<(PathBuf, usize)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut paths = entries
        .map(|entry| normal(&entry.expect("read tests entry").path()))
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, depth + 1, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push((path, depth));
        }
    }
}

/// Tells if a file that no target loads parses as Rust and declares tests.
fn unloaded_file_declares_tests(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| syn::parse_file(&text).ok())
        .is_some_and(|syntax| items_declare_tests(&syntax.items))
}

/// The source files that the module trees load, each parsed once.
#[derive(Default)]
struct Sources {
    parsed: BTreeMap<PathBuf, Option<Rc<syn::File>>>,
}

impl Sources {
    /// Gives the syntax of `path`, or `None` when it does not exist.
    fn parse(&mut self, path: &Path) -> Option<Rc<syn::File>> {
        self.parsed
            .entry(path.to_owned())
            .or_insert_with(|| {
                let text = fs::read_to_string(path).ok()?;
                Some(Rc::new(syn::parse_file(&text).unwrap_or_else(|error| {
                    panic!("parse {}: {error}", path.display());
                })))
            })
            .clone()
    }

    /// Tells if `path` declares a test function.
    fn declares_tests(&mut self, path: &Path) -> bool {
        self.parse(path)
            .is_some_and(|syntax| items_declare_tests(&syntax.items))
    }

    /// Adds `path` and each file that its `mod` declarations load to `files`,
    /// one entry for each load.
    ///
    /// A crate root, a `mod.rs` file and a file that a `#[path]` attribute
    /// names own their directory. Another file `name.rs` owns the directory
    /// `name/`. `ancestors` stops a module cycle, which rustc refuses.
    fn load(
        &mut self,
        path: &Path,
        owns_directory: bool,
        ancestors: &mut Vec<PathBuf>,
        files: &mut Vec<PathBuf>,
    ) {
        if ancestors.iter().any(|ancestor| ancestor == path) {
            return;
        }
        files.push(path.to_owned());
        let Some(syntax) = self.parse(path) else {
            return;
        };
        ancestors.push(path.to_owned());
        let directory = path.parent().expect("source file directory");
        let file_directory = if owns_directory {
            directory.to_owned()
        } else {
            directory.join(path.file_stem().expect("source file stem"))
        };
        self.load_items(&syntax.items, directory, &file_directory, ancestors, files);
        ancestors.pop();
    }

    /// Follows the `mod` items of one module.
    ///
    /// `attribute_directory` is where a `#[path]` value starts.
    /// `module_directory` is where a `mod name;` without a `#[path]` attribute
    /// looks for its file.
    fn load_items(
        &mut self,
        items: &[syn::Item],
        attribute_directory: &Path,
        module_directory: &Path,
        ancestors: &mut Vec<PathBuf>,
        files: &mut Vec<PathBuf>,
    ) {
        let mut loaded_by_name = BTreeMap::<String, BTreeSet<PathBuf>>::new();
        for item in items {
            let syn::Item::Mod(module) = item else {
                continue;
            };
            let name = module.ident.to_string();
            let mut module_files = Vec::new();
            match (&module.content, path_attribute(&module.attrs)) {
                (Some((_, inner)), Some(value)) => {
                    let directory = normal(&attribute_directory.join(value));
                    self.load_items(inner, &directory, &directory, ancestors, &mut module_files);
                }
                (Some((_, inner)), None) => {
                    let directory = module_directory.join(&name);
                    self.load_items(inner, &directory, &directory, ancestors, &mut module_files);
                }
                (None, Some(value)) => self.load(
                    &normal(&attribute_directory.join(value)),
                    true,
                    ancestors,
                    &mut module_files,
                ),
                (None, None) => {
                    let file = module_directory.join(format!("{name}.rs"));
                    let directory_file = module_directory.join(&name).join("mod.rs");
                    if file.is_file() {
                        self.load(&normal(&file), false, ancestors, &mut module_files);
                    } else if directory_file.is_file() {
                        self.load(&normal(&directory_file), true, ancestors, &mut module_files);
                    }
                }
            }
            let alternatives = loaded_by_name.entry(name).or_default();
            let earlier = alternatives.clone();
            for file in module_files {
                if !earlier.contains(&file) {
                    alternatives.insert(file.clone());
                    files.push(file);
                }
            }
        }
    }
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

/// Tells if `items` declare a test function, at the top or in an inline module.
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
