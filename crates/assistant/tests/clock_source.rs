//! Unit 34's AC4, checked mechanically over the consumer crates: the
//! assistant holds no clock of its own, and the framework's reading is the
//! only source its date and time facts come from.
//!
//! Two scans, over every workspace member the root manifest names — so a
//! crate added to the workspace joins both by existing:
//!
//! - **The manifests** name no clock or timezone crate, dependencies and
//!   dev-dependencies alike. `chrono` is not on that list and stays: it is
//!   the type of a message's UTC timestamp, which is an instant a message
//!   carries, never a statement about what the wall clock says here.
//! - **The shipped modules** name none of chrono's local-time entry
//!   points, no timezone resolver and no wall-clock format marker. Those
//!   are what re-deriving local time in the consumer would have to look
//!   like, and each would be the clock decision written a second time,
//!   free to drift from the ledger's date markers.
//!
//! The scan reads the crates' `src` directories, which is what the process
//! ships and answers from; a test file naming a marker in a string — this
//! one does — is not the consumer holding a clock.

use std::fs;
use std::path::{Path, PathBuf};

/// The crates that would give a consumer a clock or a timezone of its own.
/// A hyphen and an underscore are the same name here, so a manifest cannot
/// slip one past the list by spelling it the other way.
const CLOCK_CRATES: [&str; 8] = [
    "chrono-tz",
    "hifitime",
    "iana-time-zone",
    "jiff",
    "time",
    "time-tz",
    "tz-rs",
    "tzdb",
];

/// What re-deriving local time in a module looks like: chrono's local
/// entry points, the zone resolvers, the C call the abbreviation lives in,
/// and the format markers a wall-clock rendering needs.
const LOCAL_CLOCK_MARKERS: [&str; 8] = [
    "chrono::Local",
    "offset::Local",
    "Local::now",
    "DateTime<Local>",
    "iana_time_zone",
    "chrono_tz",
    "localtime",
    "%H:%M",
];

/// The one clock source a consumer module may name: the framework's
/// reading, taken at the moment it is rendered.
const THE_ONE_SOURCE: &str = "ClockReading::now_local()";

/// The repository root, from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every workspace member's directory, read from the root manifest's
/// member list.
fn member_dirs() -> Vec<PathBuf> {
    let root = repo_root().join("Cargo.toml");
    let manifest = fs::read_to_string(&root)
        .unwrap_or_else(|error| panic!("the root manifest {} reads: {error}", root.display()));
    let members: Vec<PathBuf> = manifest
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("members"))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with(']'))
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            let relative = trimmed.strip_prefix('"')?.strip_suffix('"')?;
            Some(repo_root().join(relative))
        })
        .collect();
    assert!(
        members.len() >= 3,
        "the member list parsed to {members:?}, which cannot be the whole workspace"
    );
    members
}

/// Whether a token is spelled like a crate name — letters, digits,
/// hyphens and underscores and nothing else. A quoted feature string or a
/// closing bracket is not one.
fn is_crate_name(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// One crate name in comparable form: underscores read as hyphens.
fn canonical(name: &str) -> String {
    name.replace('_', "-")
}

/// Every crate a manifest declares a dependency on, in any dependency
/// table: the keys inside `[dependencies]`-shaped tables, and the names of
/// `[dependencies.name]` tables.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut table = String::new();
    let mut declared = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            header.clone_into(&mut table);
            if let Some((outer, name)) = header.rsplit_once('.')
                && outer.ends_with("dependencies")
                && is_crate_name(name)
            {
                declared.push(name.to_owned());
            }
            continue;
        }
        if !table.ends_with("dependencies") {
            continue;
        }
        if let Some((key, _)) = line.split_once('=')
            && is_crate_name(key.trim())
        {
            declared.push(key.trim().to_owned());
        }
    }
    declared
}

/// Every Rust file under a directory, collected recursively.
fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("the directory {} lists: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("a listable directory entry").path();
        if path.is_dir() {
            rust_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push(path);
        }
    }
}

/// The local-clock markers a piece of source names.
fn markers_in(content: &str) -> Vec<&'static str> {
    LOCAL_CLOCK_MARKERS
        .into_iter()
        .filter(|marker| content.contains(marker))
        .collect()
}

/// No consumer manifest names a clock or timezone crate — the assistant
/// gains no dependency for a fact the framework already reads.
#[test]
fn no_consumer_manifest_names_a_clock_crate() {
    let forbidden: Vec<String> = CLOCK_CRATES.iter().map(|name| canonical(name)).collect();
    let mut findings = Vec::new();
    let mut scanned = 0;
    for manifest in member_dirs()
        .iter()
        .map(|dir| dir.join("Cargo.toml"))
        .chain(std::iter::once(repo_root().join("Cargo.toml")))
    {
        let content = fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("the manifest {} reads: {error}", manifest.display()));
        scanned += 1;
        for declared in declared_dependencies(&content) {
            if forbidden.contains(&canonical(&declared)) {
                findings.push(format!("{}: '{declared}'", manifest.display()));
            }
        }
    }
    assert!(scanned >= 4, "the scan reached {scanned} manifests");
    assert!(
        findings.is_empty(),
        "a clock or timezone crate entered a consumer manifest:\n{}",
        findings.join("\n")
    );
}

/// No shipped consumer module re-derives local time, and the framework's
/// reading is named where the facts are rendered: one clock, one source,
/// one decision.
#[test]
fn the_frameworks_reading_is_the_consumers_only_clock() {
    let mut sources = Vec::new();
    for dir in member_dirs() {
        rust_files(&dir.join("src"), &mut sources);
    }
    assert!(
        sources.len() > 10,
        "the scan reached {} modules, which cannot be the whole consumer",
        sources.len()
    );

    let mut findings = Vec::new();
    let mut reading_named = Vec::new();
    for file in &sources {
        let content = fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("the module {} reads: {error}", file.display()));
        for marker in markers_in(&content) {
            findings.push(format!("{}: '{marker}'", file.display()));
        }
        if content.contains(THE_ONE_SOURCE) {
            reading_named.push(file.display().to_string());
        }
    }
    assert!(
        findings.is_empty(),
        "a consumer module re-derives local time instead of reading the framework:\n{}",
        findings.join("\n")
    );
    assert!(
        !reading_named.is_empty(),
        "no module reads the framework's clock, so this scan proves nothing"
    );
}

/// The scans catch what they are for: a planted dependency and a planted
/// local-clock call are both found, so a clean run means a clean tree.
#[test]
fn the_scans_find_what_they_forbid() {
    assert_eq!(
        declared_dependencies(
            "[package]\nname = \"x\"\n\n[dependencies]\nchrono = \"0.4\"\n\
             tokio = { version = \"1\", features = [\n    \"time\",\n] }\n\
             \n[dev-dependencies.chrono-tz]\nversion = \"0.10\"\n"
        ),
        vec!["chrono", "tokio", "chrono-tz"],
        "keys are dependencies, feature strings are not, and a table header names one too"
    );
    assert_eq!(canonical("iana_time_zone"), "iana-time-zone");
    assert_eq!(
        markers_in("let now = chrono::Local::now().format(\"%H:%M\");"),
        vec!["chrono::Local", "Local::now", "%H:%M"]
    );
    assert!(
        markers_in("let clock = agent_ledger::store::ClockReading::now_local();").is_empty(),
        "reading the framework is the allowed shape"
    );
}
