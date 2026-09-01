//! Unit 52's criterion 19, checked mechanically over the consumer crates:
//! the tool palette this assistant used to keep is gone, and the only
//! places its name survives are the enumerated ones, each with the reason
//! it must.
//!
//! The mechanism was a consumer block kind, its content table, its
//! descriptor and a wrapper around every tool handler. Which tools a
//! conversation has is the framework's own record now, so none of that
//! exists here and no identifier of it is spelled anywhere.
//!
//! The NAME cannot go the same way, and the criterion's own neighbour says
//! why: criterion 25 asks for a migration that drops the withdrawn table,
//! which has to name it, and for a test that recreates a database the
//! previous build wrote, which has to write rows of the withdrawn kind.
//! The debt walk's read-through set names it for the same reason: the
//! header rows that build recorded stay in the ledger, and a walk that
//! could not read them would bury a member's unanswered question behind
//! one. Every one of those is about a disk that already exists, not about a
//! mechanism that still runs. This scan is what keeps the difference
//! honest: every surviving spelling is listed below with its reason, an
//! entry that stops matching is removed, and a new one is added
//! deliberately, never a mechanism creeping back unnoticed.

use std::fs;
use std::path::{Path, PathBuf};

/// The spelling the withdrawn mechanism went by, matched without regard to
/// case — the same reading the criterion asks for.
const WITHDRAWN_SPELLING: &str = "palette";

/// One file that may still spell it, with the reason it does.
struct Allowed {
    /// The file, relative to the repository root.
    file: &'static str,
    /// Why the spelling belongs there.
    reason: &'static str,
}

/// Every file in the consumer crates that may name the withdrawn
/// mechanism. Anything else naming it is the mechanism coming back.
const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "crates/core/src/schema.rs",
        reason: "the withdrawn table's name, read by the step that drops it, by the row \
                 that step deletes, and by the withdrawal the store declares at open",
    },
    Allowed {
        file: "crates/core/src/assembly.rs",
        reason: "the debt walk's read-through set, which names the withdrawn kind's stored \
                 string so a header row a previous build recorded stays transparent to the \
                 walk instead of burying the question behind it",
    },
    Allowed {
        file: "crates/core/tests/spine/protection.rs",
        reason: "the database the previous build wrote: the fixture recreates its table, \
                 its registry row and one block of the withdrawn kind, then asserts the \
                 reopen keeps the history",
    },
    Allowed {
        file: "crates/core/tests/spine/report.rs",
        reason: "the test that appends a header row of the withdrawn kind and asserts the \
                 debt walk reads through it, the way it reads through every kind appended \
                 at an arbitrary point",
    },
    Allowed {
        file: "crates/assistant/tests/docs.rs",
        reason: "a dated decision record's filename in the decisions listing, and the \
                 privacy assessment's sentence about the reaction set an adapter offers \
                 — a different subject that is still live",
    },
];

/// This file, which is outside both scans: it spells the searched-for name
/// and every withdrawn identifier on purpose, and a scan reading its own
/// search strings back would report itself.
const THE_SCAN_ITSELF: &str = "crates/assistant/tests/withdrawn_kinds.rs";

/// The repository root, from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every Rust file the consumer crates ship or test with, this one apart.
fn consumer_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(&repo_root().join("crates"), &mut files);
    files.retain(|file| relative(file) != THE_SCAN_ITSELF);
    assert!(
        files.len() > 20,
        "the scan reached {} files, which cannot be the whole consumer",
        files.len()
    );
    files.sort();
    files
}

fn collect_rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("the directory {} lists: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("a listable directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push(path);
        }
    }
}

/// One file's path as the allowlist spells it: repository-relative, with
/// forward slashes.
fn relative(file: &Path) -> String {
    let root = repo_root()
        .canonicalize()
        .expect("the repository root reads");
    let file = file.canonicalize().expect("the scanned file reads");
    file.strip_prefix(&root)
        .expect("every scanned file sits under the repository root")
        .to_string_lossy()
        .replace('\\', "/")
}

/// The lines of a file that spell the withdrawn mechanism, numbered from
/// one.
fn spelling_lines(content: &str) -> Vec<usize> {
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(WITHDRAWN_SPELLING))
        .map(|(index, _)| index + 1)
        .collect()
}

/// The withdrawn mechanism is named in the listed files and nowhere else,
/// and every listed file still names it.
#[test]
fn the_withdrawn_mechanism_survives_only_where_it_is_listed() {
    let mut findings = Vec::new();
    let mut matched: Vec<&str> = Vec::new();
    for file in consumer_files() {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("the file {} reads: {error}", file.display()));
        let lines = spelling_lines(&content);
        if lines.is_empty() {
            continue;
        }
        let path = relative(&file);
        match ALLOWED.iter().find(|allowed| allowed.file == path) {
            Some(allowed) => matched.push(allowed.file),
            None => findings.push(format!("{path}: lines {lines:?}")),
        }
    }
    assert!(
        findings.is_empty(),
        "the withdrawn tool mechanism is named outside the listed files:\n{}\n\
         Which tools a conversation has is the framework's recorded choice; if a new \
         file must name the withdrawn one, add it to ALLOWED with the reason.",
        findings.join("\n")
    );
    for allowed in ALLOWED {
        assert!(
            matched.contains(&allowed.file),
            "{} no longer names it, so its entry is stale: {}",
            allowed.file,
            allowed.reason
        );
    }
}

/// No identifier of the mechanism is spelled anywhere in the consumer
/// crates — the half of the criterion the allowlist above says nothing
/// about, and the half that decides whether the mechanism is really gone.
#[test]
fn no_identifier_of_the_withdrawn_mechanism_is_spelled() {
    let identifiers = [
        "ToolPalette",
        "AdmittedTool",
        "newest_tools",
        "reconcile_palette",
        "TOOL_PALETTE_KIND",
        "TOOL_PALETTE_TABLE",
    ];
    let mut findings = Vec::new();
    for file in consumer_files() {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("the file {} reads: {error}", file.display()));
        for identifier in identifiers.iter().filter(|name| content.contains(**name)) {
            findings.push(format!("{}: '{identifier}'", relative(&file)));
        }
    }
    assert!(
        findings.is_empty(),
        "an identifier of the withdrawn tool mechanism is back:\n{}",
        findings.join("\n")
    );
}

/// The scan finds what it is for: a planted line naming the mechanism is
/// read as one, whatever its case, and ordinary source is not.
#[test]
fn the_scan_finds_a_planted_spelling() {
    assert_eq!(
        spelling_lines("fn plain() {}\nconst TOOL_PALETTE_KIND: &str = \"tool_palette\";\n"),
        vec![2],
        "the planted line is found, whatever case it is spelled in"
    );
    assert!(
        spelling_lines("fn plain() {}\nconst KIND: &str = \"tool_choice\";\n").is_empty(),
        "the recorded choice is not the withdrawn mechanism"
    );
}
