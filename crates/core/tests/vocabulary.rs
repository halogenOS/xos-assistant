//! The platform-vocabulary scan: the core contains no platform vocabulary,
//! checked against the committed word list in `docs/platform-vocabulary.txt`.
//!
//! The scan reads every source file of this crate — the library, its manifest
//! and these tests — and fails on any case-insensitive whole-word match: a
//! word is one run of letters and digits, so a compound identifier still
//! trips on the platform name it contains, while neutral prose that merely
//! embeds a listed word inside a longer word does not. The forbidden words
//! live only in the list file, which is why this test names none of them.

use std::fs;
use std::path::{Path, PathBuf};

/// The committed word list, with comments and blank lines dropped.
fn forbidden_words() -> Vec<String> {
    let list = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/platform-vocabulary.txt");
    let content = fs::read_to_string(&list)
        .unwrap_or_else(|e| panic!("the committed word list at {} reads: {e}", list.display()));
    let words: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_lowercase)
        .collect();
    assert!(
        !words.is_empty(),
        "an empty word list would scan for nothing and pass vacuously"
    );
    words
}

/// Every file the invariant covers: the crate's Rust sources and its
/// manifest, collected recursively so a new module joins the scan by
/// existing.
fn scanned_files() -> Vec<PathBuf> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![crate_root.join("Cargo.toml")];
    for dir in ["src", "tests"] {
        collect_rust_files(&crate_root.join(dir), &mut files);
    }
    assert!(
        files.iter().any(|f| f.ends_with("src/lib.rs")),
        "the scan must reach the crate's sources"
    );
    files
}

fn collect_rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("the source directory {} lists: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a listable directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push(path);
        }
    }
}

/// Whether a line carries the word as a whole word: one of the line's runs
/// of letters and digits equals it. `_` and `-` separate runs, so a compound
/// identifier is scanned part by part.
fn carries_word(line: &str, word: &str) -> bool {
    line.split(|c: char| !c.is_alphanumeric())
        .any(|token| token == word)
}

#[test]
fn the_core_crate_carries_no_platform_vocabulary() {
    let words = forbidden_words();
    let mut findings = Vec::new();
    for file in scanned_files() {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("the source file {} reads: {e}", file.display()))
            .to_lowercase();
        for (number, line) in content.lines().enumerate() {
            for word in &words {
                if carries_word(line, word) {
                    findings.push(format!("{}:{}: '{word}'", file.display(), number + 1));
                }
            }
        }
    }
    assert!(
        findings.is_empty(),
        "platform vocabulary leaked into the core:\n{}",
        findings.join("\n")
    );
}

#[test]
fn the_scan_tells_whole_words_from_substrings() {
    // The scan lowercases each file before matching, so the helper sees
    // lowercase lines only — these mirror that contract.
    assert!(carries_word("a compound_platformname_id", "platformname"));
    assert!(carries_word("platformname::connect(&key)", "platformname"));
    assert!(
        !carries_word("an unplatformnamed thing", "platformname"),
        "a listed word inside a longer word is not a match"
    );
}
