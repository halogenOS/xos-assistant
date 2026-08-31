//! The adapter's source scans: the facts about this crate that are stated
//! by what its sources do NOT name.
//!
//! The classification contract's source half: the adapter's batch
//! discipline reads the core's terminal-or-transient statement and never a
//! variant name. The behavioral half — a terminal refusal acknowledged past,
//! a transient failure halting the batch — is pinned in the translation and
//! offset modules; this scan keeps the source honest about HOW those
//! outcomes are decided, so the core's error vocabulary can grow without an
//! adapter release.
//!
//! Beside it, the message-reach scan (unit T3, 2026-08-31): the platform
//! methods that reach back into a message already sent — editing it,
//! deleting it, streaming a partial one — are absent from this crate, so a
//! later unit that adds one does so deliberately. It is named for the reach,
//! not for authority: authority means a person's standing everywhere else in
//! this tree, and nothing here is about standing.

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

/// The platform methods that reach back into a message already sent: the
/// edit family whole — text, caption, media, reply markup, and the live
/// location with the call that stops it — the two that delete, and the two
/// that stream a partial one. Named here, in a test, because their ABSENCE
/// is the fact.
///
/// The list is a named set, never a proof that the platform has no other:
/// a method the API adds later is uncovered until it is named here, which
/// is the same discipline the core error variants above are listed under.
/// Every name is matched as a SUBSTRING of the source, so a builder
/// composing one from pieces still trips the scan on the piece it spells
/// whole, and `deleteMessage` covers `deleteMessages` by construction —
/// both are listed anyway, because the list states what is refused, not
/// what the matcher needs.
const UNCALLED_MESSAGE_METHODS: &[&str] = &[
    "editMessageText",
    "editMessageCaption",
    "editMessageMedia",
    "editMessageReplyMarkup",
    "editMessageLiveLocation",
    "stopMessageLiveLocation",
    "deleteMessage",
    "deleteMessages",
    "sendMessageDraft",
    "sendRichMessageDraft",
];

/// AC13 of the editing unit (unit T3, 2026-08-31): the assistant edits none
/// of its own delivered messages and deletes no member's message, and the
/// proof is that no request builder in this crate names a method that
/// would. The self-edit refusal is decision 0079's equality — a delivered
/// answer would silently change under readers who already read it, and the
/// stored answer block would stop being what the channel saw — and the
/// deletion refusal is decision 0070's: every moderation effect keeps a
/// human in it, and the platform grants an administrator bot the power
/// this deployment deliberately leaves unused.
#[test]
fn the_adapter_calls_no_message_edit_deletion_or_draft_method() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);
    assert!(!files.is_empty(), "the adapter's sources are scanned");
    for file in files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("the source file {} reads: {e}", file.display()));
        for method in UNCALLED_MESSAGE_METHODS {
            assert!(
                !content.contains(method),
                "{} names {method}; the assistant edits none of its own \
                 delivered messages and deletes nobody's, so adding one is \
                 a decision a unit takes on purpose",
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
