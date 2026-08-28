use crate::{
    atomic_file::{FileRollbackJournal, FileSnapshot},
    cli::SteOnboardingMode,
};
use anyhow::Context;
use camino::Utf8Path;
use fs2::FileExt;
use provenance_macros::rule;
use provenance_ste100::DictionaryImport;
use provenance_store::{dictionary_reference, layout::ProvenanceLayout};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const OFFICIAL_ASSET: &str = "https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf";
const REQUEST_FORM: &str = "https://www.asd-ste100.org/STE_downloads.html#article02-2l";
const CHANGE_FORM: &str = "https://www.asd-ste100.org/STE_downloads.html#features038-31";
const DOWNLOAD_ATTEMPTS: usize = 3;
const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;

pub struct Plan {
    import: Option<DictionaryImport>,
    reference_before: Option<FileSnapshot>,
    reference_bytes: Option<Vec<u8>>,
    message: Option<String>,
}

/// Selects the interactive form path or the bounded agent download path.
#[rule("rule_ste_dictionary_interactive_acquisition")]
#[rule("rule_ste_dictionary_agent_acquisition")]
#[rule("rule_ste_dictionary_no_operational_download")]
pub fn prepare(
    repo: &Utf8Path,
    mode: SteOnboardingMode,
    selected_pdf: Option<&Utf8Path>,
) -> anyhow::Result<Plan> {
    let layout = ProvenanceLayout::new(repo.to_owned());
    let reference_path = dictionary_reference::dictionary_reference_path(&layout);
    let reference_before = FileSnapshot::read(reference_path.as_std_path())?;
    if dictionary_reference::load_project_dictionary(&layout).is_some() {
        reference_before.recheck(reference_path.as_std_path())?;
        return Ok(Plan::unchanged(reference_before));
    }

    let (import, message) = match (mode, selected_pdf) {
        (_, Some(pdf)) => (
            Some(import_pdf(pdf)?),
            Some(format!(
                "Imported the selected Issue 9 dictionary.\n{}",
                product_notice()
            )),
        ),
        (SteOnboardingMode::Interactive, None) => (None, Some(interactive_guidance())),
        (SteOnboardingMode::Agent, None) => match acquire_agent_dictionary_blocking() {
            Ok(import) => (
                Some(import),
                Some(format!(
                    "Imported the official Issue 9 dictionary.\n{}",
                    product_notice()
                )),
            ),
            Err(error) => (None, Some(fallback_guidance(&error))),
        },
    };

    let Some(import) = import else {
        return Ok(Plan {
            import: None,
            reference_before: None,
            reference_bytes: None,
            message,
        });
    };
    let before = FileSnapshot::read(reference_path.as_std_path())?;
    let mut bytes = serde_json::to_vec_pretty(&import.identity)
        .context("serialize the dictionary reference")?;
    bytes.push(b'\n');
    Ok(Plan {
        import: Some(import),
        reference_before: Some(before),
        reference_bytes: Some(bytes),
        message,
    })
}

impl Plan {
    const fn unchanged(reference_before: FileSnapshot) -> Self {
        Self {
            import: None,
            reference_before: Some(reference_before),
            reference_bytes: None,
            message: None,
        }
    }

    pub(super) fn recheck(&self, repo: &Utf8Path) -> anyhow::Result<()> {
        if let Some(before) = &self.reference_before {
            let layout = ProvenanceLayout::new(repo.to_owned());
            before
                .recheck(dictionary_reference::dictionary_reference_path(&layout).as_std_path())?;
        }
        Ok(())
    }

    pub(super) fn apply_in(
        &self,
        repo: &Utf8Path,
        rollback: &mut FileRollbackJournal,
    ) -> anyhow::Result<()> {
        let Some(import) = &self.import else {
            return Ok(());
        };
        let index_directory = dictionary_reference::index_directory()
            .context("no machine data directory is available")?;
        provenance_ste100::store_dictionary_index(import, &index_directory)
            .context("store the dictionary index")?;
        let layout = ProvenanceLayout::new(repo.to_owned());
        rollback.replace(
            dictionary_reference::dictionary_reference_path(&layout).as_std_path(),
            self.reference_before.as_ref().expect("planned snapshot"),
            self.reference_bytes.as_deref().expect("planned bytes"),
        )
    }

    pub(super) fn print_message(&self) {
        if let Some(message) = &self.message {
            println!("{message}");
        }
    }
}

fn import_pdf(path: &Utf8Path) -> anyhow::Result<DictionaryImport> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read the dictionary PDF at {path}"))?;
    import_bytes(&bytes)
}

fn import_bytes(bytes: &[u8]) -> anyhow::Result<DictionaryImport> {
    provenance_ste100::import_dictionary(bytes)
        .map_err(|error| anyhow::anyhow!("import the dictionary: {error}"))
}

/// Serializes access to the shared asset and retries only the official URL.
#[rule("rule_ste_dictionary_download_concurrency")]
#[rule("rule_ste_dictionary_download_retry_bound")]
#[rule("rule_ste_dictionary_download_identity")]
#[rule("rule_ste_dictionary_asset_fallback")]
fn acquire_agent_dictionary_blocking() -> anyhow::Result<DictionaryImport> {
    let directory = asset_directory().context("no machine cache directory is available")?;
    std::fs::create_dir_all(&directory).context("create the shared STE asset cache")?;
    let lock = open_lock(&directory.join("issue-9.pdf.lock"))?;
    FileExt::lock_exclusive(&lock).context("lock the shared STE asset cache")?;
    let asset = directory.join("ASD-STE100_ISSUE9.pdf");
    if let Ok(bytes) = std::fs::read(&asset) {
        if let Ok(import) = import_bytes(&bytes) {
            return Ok(import);
        }
        std::fs::remove_file(&asset).context("remove an invalid cached STE asset")?;
    }

    let client = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .redirects(0)
        .build();
    let mut last_error = None;
    for attempt in 0..DOWNLOAD_ATTEMPTS {
        match download(&client).and_then(|bytes| {
            let import = import_bytes(&bytes)?;
            store_asset(&asset, &bytes)?;
            Ok(import)
        }) {
            Ok(import) => return Ok(import),
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 < DOWNLOAD_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("the official asset is unavailable")))
}

fn download(client: &ureq::Agent) -> anyhow::Result<Vec<u8>> {
    let response = client
        .get(&asset_url())
        .set(
            "User-Agent",
            &format!("Provenance/{}", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| anyhow::anyhow!("request the official Issue 9 PDF: {error}"))?;
    if response
        .header("Content-Length")
        .and_then(|length| length.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_ASSET_BYTES)
    {
        anyhow::bail!("the official Issue 9 PDF exceeds the download size limit");
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_ASSET_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("read the official Issue 9 PDF")?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_ASSET_BYTES,
        "the official Issue 9 PDF exceeds the download size limit"
    );
    Ok(bytes)
}

fn open_lock(path: &Path) -> anyhow::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .context("open the shared STE asset lock")
}

fn store_asset(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let partial = path.with_extension(format!("pdf.{}.partial", std::process::id()));
    let mut file = File::create(&partial).context("create the staged STE asset")?;
    file.write_all(bytes)
        .context("write the staged STE asset")?;
    file.sync_all().context("sync the staged STE asset")?;
    std::fs::rename(&partial, path).context("publish the shared STE asset")
}

fn asset_url() -> String {
    if let Some(url) = std::env::var_os("PROVENANCE_TEST_STE100_ASSET_URL") {
        let url = url.to_string_lossy();
        if let Ok(parsed) = url::Url::parse(&url) {
            if parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"))
            {
                return parsed.into();
            }
        }
    }
    OFFICIAL_ASSET.to_owned()
}

/// Keeps downloaded source material in a machine cache, not in a distribution.
#[rule("rule_ste_dictionary_not_distributed")]
fn asset_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("PROVENANCE_STE100_ASSET_DIR") {
        return Some(PathBuf::from(directory));
    }
    Some(cache_directory()?.join("provenance").join("ste100-assets"))
}

#[cfg(target_os = "windows")]
fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
}

fn interactive_guidance() -> String {
    format!(
        "Get ASD-STE100 Issue 9 from the official request page, then rerun init with --ste-pdf <path>:\n{REQUEST_FORM}\n{}",
        product_notice()
    )
}

fn fallback_guidance(error: &anyhow::Error) -> String {
    format!(
        "The official Issue 9 asset is unavailable after {DOWNLOAD_ATTEMPTS} attempts ({error}). Use the official request page; Provenance does not search for another asset:\n{REQUEST_FORM}\n{}",
        product_notice()
    )
}

/// Gives the required ownership, stewardship, source, and claim limits.
#[rule("rule_ste_dictionary_attribution")]
#[rule("rule_ste_dictionary_claim_scope")]
#[rule("rule_ste_dictionary_change_form_link")]
fn product_notice() -> String {
    format!(
        "ASD owns ASD-STE100, and STEMG maintains it. Official request page: {REQUEST_FORM}. Official change-form page: {CHANGE_FORM}. Provenance names only its implemented Issue 9 checks. It does not claim compliance, certification, endorsement, or approval."
    )
}
