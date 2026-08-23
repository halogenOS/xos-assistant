//! AC8's documentation pins: the prompt's returned report bullet with its
//! tool teaching, the 0046 closure, the 0044 amendment, the 0045
//! narrowing, the unit-5 no-write amendment, and the four privacy drafts'
//! dated updates. Each pin reads the committed file the way the repository
//! ships it, so a drifted edit fails loudly here.

use std::path::Path;

/// One repository file, read relative to this crate.
fn repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the file {} reads: {error}", path.display()))
}

#[test]
fn the_prompt_regains_the_report_bullet_tied_to_the_tool() {
    let prompt = repo_file("prompts/assistant.md");
    assert!(
        prompt.contains("* /report when it needs human judgment"),
        "the gated bullet returned verbatim, per decision 0046"
    );
    assert!(
        prompt.contains("use the report_spam tool"),
        "the tool teaching names the report tool"
    );
    assert!(
        prompt.contains("it is the only way to report"),
        "the tool is taught as the only way to report"
    );
    assert!(
        prompt.contains("Never write the /report command into an answer yourself"),
        "the model is told never to write the moderation command in prose"
    );
}

#[test]
fn the_0046_closure_records_the_gate_closed_with_the_prose_residual() {
    let record = repo_file("docs/decisions/0046-the-system-prompt-is-the-maintainers.md");
    assert!(
        record.contains("Gate 2 closed 2026-08-23"),
        "the closure is dated"
    );
    assert!(
        record.contains("acts only on command REPLIES"),
        "the residual's reasoning is recorded"
    );
    assert!(
        record.contains("Gates 1, 3 and 4 stay held"),
        "the other gates stay held"
    );
}

#[test]
fn the_0044_amendment_and_the_0045_narrowing_are_recorded() {
    let amended = repo_file("docs/decisions/0044-tool-failures-speak-to-the-model-not-the-chat.md");
    assert!(
        amended.contains("Amended 2026-08-23"),
        "the 0044 amendment is dated"
    );
    assert!(
        amended.contains("FAILURE still speaks to the model alone"),
        "the amendment keeps the failure rule intact"
    );

    let narrowed = repo_file("docs/decisions/0045-erasure-does-not-reach-tool-blocks-yet.md");
    assert!(
        narrowed.contains("Narrowed 2026-08-23"),
        "the 0045 narrowing is dated"
    );
    assert!(
        narrowed.contains("decision 0063"),
        "the narrowing names its decision"
    );
}

#[test]
fn the_unit_five_no_write_rule_carries_its_dated_amendment() {
    let unit = repo_file("docs/units/05-tools.md");
    assert!(
        unit.contains("Amended 2026-08-23 (unit 8, decision 0060)"),
        "the amendment is dated and sourced"
    );
    assert!(
        unit.contains("Lookups still write nothing."),
        "the rule's remainder stands"
    );
}

#[test]
fn the_four_privacy_drafts_carry_their_dated_report_updates() {
    for (draft, marker) in [
        (
            "docs/privacy/privacy-policy.md",
            "It can pass a report to the group's\nmoderation bot when a member replies to a message and asks for one",
        ),
        (
            "docs/privacy/dpia.md",
            "## 12. Addendum, 2026-08-23: the report and the wiki fetch",
        ),
        (
            "docs/privacy/lia.md",
            "Amended 2026-08-23: the report feature lets a member ask",
        ),
        (
            "docs/privacy/records-of-processing.md",
            "Report record (added 2026-08-23)",
        ),
    ] {
        let content = repo_file(draft);
        assert!(
            content.contains(marker),
            "{draft} carries its dated update: expected {marker:?}"
        );
    }
    let records = repo_file("docs/privacy/records-of-processing.md");
    assert!(
        records.contains("The group's administrators, via the group's moderation bot"),
        "the recipients table names the report event's recipients"
    );
}
