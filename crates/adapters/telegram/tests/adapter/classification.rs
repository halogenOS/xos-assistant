//! The classification contract's source half: the adapter's batch
//! discipline reads the core's terminal-or-transient statement and never a
//! variant name. The behavioral half — a terminal refusal acknowledged past,
//! a transient failure halting the batch — is pinned in the translation and
//! offset modules; this scan keeps the source honest about HOW those
//! outcomes are decided, so the core's error vocabulary can grow without an
//! adapter release.

use std::fs;
use std::path::{Path, PathBuf};

/// The core's error variant names. Listed here, in a test, on purpose: this
/// suite is the contract's consumer side, and the list failing to compile or
/// scan is exactly the alarm wanted when the vocabulary changes shape.
const CORE_ERROR_VARIANTS: &[&str] = &[
    "ChannelKindMismatch",
    "ClaimLost",
    "ErasureUnsettled",
    "MissingContentTable",
    "UnknownVendor",
];

#[test]
fn the_adapter_source_names_no_core_error_variant() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);
    assert!(!files.is_empty(), "the adapter's sources are scanned");
    for file in files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("the source file {} reads: {e}", file.display()));
        for variant in CORE_ERROR_VARIANTS {
            assert!(
                !content.contains(variant),
                "{} names the core error variant {variant}; the batch \
                 discipline must read the classification instead",
                file.display()
            );
        }
    }
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
