//! AC8's documentation pins: the report teaching's move out of the prompt
//! (unit 15; the bullet's earlier return is recorded in decision 0046 and
//! its removal in decision 0091), the 0046 closure, the 0044 amendment, the 0045
//! narrowing, the unit-5 no-write amendment, and the four privacy drafts'
//! dated updates — joined by the username-projection unit's AC4 pins (the
//! prompt's mention line, the DPIA's dated note and the three decision
//! records that close 0056's implementation debt), the minimization
//! pins of decision 0077 (the record, the shrunken policy sections and the
//! four documents' dated notes), and the privacy-self-service unit's AC6
//! pins: the prompt's tool teaching with its verbatim relay, the six
//! decision records, the erasure decision's idempotency refinement, the
//! no-write amendment's second clause, every fixed line verbatim, and the
//! four policy edits with the two assessment notes — and the
//! first-interaction-disclosure unit's AC4 pins: the prompt's honest-answer
//! teaching, the operator's disclosure copy verbatim, the AI Act compliance
//! record's role conclusion, obligations map, marking position and notes,
//! the DPIA's dated role correction, and the unit's four decision records
//! — and the deletion-mirror unit's AC5 pins: the policy's
//! administrator-deletion sentence with its reply-route scope, the DPIA's
//! mirror paragraph with the in-flight-turn window, the operator
//! reference's piggyback section with its reply-only and bare-token
//! bounds, and the unit's five decision records — and the
//! autonomous-moderation unit's AC7 pins: the policy's assessment
//! sentence, the DPIA's purpose and false-positive residual, the
//! compliance page's Article-22 note, and the unit's four decision
//! records with the removed member report among them — and the
//! rules-acknowledgment unit's AC7 pins: the operator contract's
//! generated-with-fallback wording, the 0051 refinement, the two
//! no-disclosure records (decision 0079 and unit 12) that no longer group the
//! now-model-generated acknowledgment among human-written fixed lines, and the
//! unit's two decision records.
//! Each pin reads the
//! committed file the way the repository ships it, so a drifted edit fails
//! loudly here.

use std::path::Path;

/// One repository file, read relative to this crate.
fn repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the file {} reads: {error}", path.display()))
}

/// The file's prose with every whitespace run folded to one space — the
/// wrap-independent reading for pins that are about the words, so a
/// re-wrapped paragraph cannot fail them.
fn flattened(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The report teaching left the operator's prompt with unit 15: the
/// member-initiated flow is removed (decision 0091), and the autonomous
/// teaching composes in the core, exactly where the tool registers — so
/// the prompt file carries no report instruction at all, and the composed
/// constant carries the whole of it.
#[test]
fn the_report_teaching_moved_from_the_prompt_to_the_composition() {
    let prompt = repo_file("prompts/assistant.md");
    assert!(
        !prompt.contains("/report"),
        "the prompt no longer names the report command"
    );
    assert!(
        !prompt.contains("report_spam"),
        "the prompt no longer teaches the report tool; the composition does"
    );
    assert!(
        flattened(&prompt).contains("rule enforcement with a light reminder in text"),
        "the prose reminder stays the prompt's own enforcement voice"
    );

    let teaching = assistant_core::MODERATION_TEACHING;
    assert!(
        teaching.contains("the report_spam tool"),
        "the composed teaching names the tool"
    );
    assert!(
        teaching.contains("never write the report command into an answer yourself"),
        "the composed teaching keeps the never-type-the-command rule"
    );
    assert!(
        teaching.contains("The tool is the only way to report"),
        "the composed teaching keeps the only-way rule"
    );
}

/// AC8's content pin: the shipped base prose lost the hardcoded
/// community-rules list and the addressed-only stay-quiet framing, and the
/// composed system prompt built from it inherits neither. The group's rules
/// reach the model through exactly one channel — the pinned rules note —
/// and the composed answering teaching alone governs when the assistant
/// speaks.
#[test]
fn the_base_prose_carries_no_rules_list_and_no_stay_quiet_framing() {
    use assistant_core::AnsweringMode;

    let base = repo_file("prompts/assistant.md");
    let composed = [
        assistant_core::composed_system_prompt(&base, "Probe", AnsweringMode::Helpful, true),
        assistant_core::composed_system_prompt(&base, "Probe", AnsweringMode::Addressed, false),
    ];
    for (name, prose) in std::iter::once(("the shipped base prose", &base)).chain(
        composed
            .iter()
            .map(|prompt| ("a composed system prompt", prompt)),
    ) {
        let flat = flattened(prose);
        for leak in [
            "Community rules",
            "applies to everyone",
            "Do not ask for ETA repeatedly",
            "stay quiet during regular conversations",
            "stay quiet",
        ] {
            assert!(!flat.contains(leak), "{name} still carries: {leak:?}");
        }
    }
}

#[test]
fn the_username_projection_ships_its_prompt_line_dpia_note_and_decision_records() {
    let prompt = repo_file("prompts/assistant.md");
    assert!(
        flattened(&prompt).contains(
            "You may mention a person by the handle shown with their message, \
             and never guess a handle you were not shown."
        ),
        "the prompt teaches the mention permission and its bound in one line"
    );

    let dpia = repo_file("docs/privacy/dpia.md");
    assert!(
        dpia.contains("Amended 2026-08-23: the username projection shipped."),
        "the DPIA's transmitted-identifier line carries its dated note"
    );

    let column = repo_file("docs/decisions/0065-the-speaker-is-a-column-on-the-message-row.md");
    assert!(
        column.contains("Date: 2026-08-23"),
        "the column record is dated"
    );
    assert!(
        column.contains("closes decision 0056's implementation debt"),
        "the column record names the debt it closes"
    );
    assert!(
        column.contains("## Rejected alternatives"),
        "the column record carries its rejected alternatives"
    );

    let projection =
        repo_file("docs/decisions/0066-the-projection-prefixes-the-speaker-only-for-people.md");
    assert!(
        projection.contains("Date: 2026-08-23"),
        "the projection record is dated"
    );
    assert!(
        flattened(&projection).contains(
            "A user-voiced message with a speaker projects as the speaker, \
             a colon and a space, then the text."
        ),
        "the projection record states the prefix rule"
    );
    assert!(
        projection.contains("## Rejected alternatives"),
        "the projection record carries its rejected alternatives"
    );

    let teaching =
        repo_file("docs/decisions/0067-the-prompt-may-address-people-by-the-shown-handle.md");
    assert!(
        teaching.contains("Date: 2026-08-23"),
        "the teaching record is dated"
    );
    assert!(
        flattened(&teaching).contains("must never guess a handle it was not shown"),
        "the teaching record states the mention bound"
    );
    assert!(
        teaching.contains("## Rejected alternatives"),
        "the teaching record carries its rejected alternatives"
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

/// Decision 0077's documentation pins: the decision record with its
/// rejected alternatives, the policy's shrunken author and language-model
/// sections, the DPIA's category note and defect closure, the records'
/// narrowed rows, and the LIA's amended transfer line.
#[test]
fn the_minimization_decision_ships_its_record_and_dated_doc_updates() {
    let record = repo_file(
        "docs/decisions/0077-the-display-name-is-not-stored-and-titles-are-not-derived.md",
    );
    assert!(record.contains("Date: 2026-08-23"), "the record is dated");
    assert!(
        record.contains("## Rejected alternatives"),
        "the record carries its rejected alternatives"
    );
    assert!(
        record.contains("Keeping the dead column."),
        "the record rejects keeping the dead column"
    );
    assert!(
        record.contains("Keeping title derivation because it is cheap."),
        "the record rejects keeping titles nobody reads"
    );

    let policy = repo_file("docs/privacy/bot-assistant-privacy-policy.md");
    let policy_flat = flattened(&policy);
    assert!(
        policy_flat.contains("We do not store your display name."),
        "the policy's author section states the display name is not stored"
    );
    assert!(
        !policy_flat.contains("smaller model") && !policy_flat.contains("smaller naming model"),
        "the policy lost the smaller-model sentence and its transfer mention"
    );

    let dpia = repo_file("docs/privacy/dpia.md");
    let dpia_flat = flattened(&dpia);
    assert!(
        dpia_flat.contains("the identity category shrinks"),
        "the DPIA's identity category carries its dated narrowing note"
    );
    assert!(
        dpia_flat.contains("Closed 2026-08-23: the conversation-naming feature is switched off"),
        "the DPIA's title-derivation defect note carries its closure"
    );

    let records = repo_file("docs/privacy/records-of-processing.md");
    let records_flat = flattened(&records);
    assert!(
        records_flat.contains("Narrowed 2026-08-23 (decision 0077)"),
        "the records' identity row is narrowed with its date"
    );
    assert!(
        records_flat.contains("no display name exists to send"),
        "the records' processor row reflects the removal"
    );

    let lia = repo_file("docs/privacy/lia.md");
    assert!(
        flattened(&lia).contains("the display name is not even stored"),
        "the LIA's transfer prose carries its dated narrowing"
    );
    assert!(
        flattened(&lia).contains(
            "the display name is not\nstored (narrowed 2026-08-23"
                .replace('\n', " ")
                .as_str()
        ) || flattened(&lia).contains("the display name is not stored (narrowed 2026-08-23"),
        "the LIA's necessity paragraph carries the narrowing too"
    );
    assert!(
        !flattened(&lia).contains("All three live"),
        "the necessity paragraph counts two stored identity fields, not three"
    );
}

// ─── The privacy-self-service unit's pins (AC6, 2026-08-23) ──────────────

#[test]
fn the_prompt_teaches_the_privacy_tool_and_the_verbatim_relay() {
    let prompt = flattened(&repo_file("prompts/assistant.md"));
    assert!(
        prompt.contains("use the privacy_request tool with action opt_out"),
        "the prompt names the tool and the opt-out action"
    );
    assert!(
        prompt.contains("use it with action request_deletion"),
        "the prompt names the deletion action"
    );
    assert!(
        prompt.contains("Relay the quoted text in the tool's result to the person verbatim"),
        "the prompt orders the verbatim relay"
    );
    assert!(
        prompt.contains("tell the person to send /privacyout or /privacydelete themselves"),
        "the prompt routes an ambiguity decline to the commands"
    );
    assert!(
        prompt.contains(
            "The commands /privacyout, /privacydelete, /confirmdelete and /unblockprivacy \
             always work directly"
        ),
        "the prompt names the four commands as the direct path"
    );
    assert!(
        prompt.contains("you never perform a privacy change by just saying you did"),
        "the prompt forbids a claimed change without the tool"
    );
}

#[test]
fn the_units_decisions_are_recorded_with_dates_and_rejected_alternatives() {
    for record in [
        "docs/decisions/0071-opt-out-is-a-suppression-stub.md",
        "docs/decisions/0072-the-privacy-command-family-is-exempt-from-suppression.md",
        "docs/decisions/0073-deletion-confirms-programmatically-and-runs-outside-the-fence.md",
        "docs/decisions/0074-erasure-keeps-the-stub-when-the-flag-stands.md",
        "docs/decisions/0075-plain-language-reaches-the-rights-through-one-tool.md",
        "docs/decisions/0076-the-rights-replies-are-bounded-per-person.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-23"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
}

#[test]
fn the_erasure_idempotency_refinement_and_the_second_clauses_are_recorded() {
    let erasure = repo_file("docs/decisions/0012-erasure-reaches-the-prose.md");
    assert!(
        erasure.contains("Refined 2026-08-23, with the privacy-self-service unit"),
        "the erasure decision's refinement is dated"
    );
    assert!(
        flattened(&erasure)
            .contains("re-runs over emptiness and reports completion rather than not-found"),
        "the refinement states the changed idempotency"
    );

    let amended = repo_file("docs/decisions/0044-tool-failures-speak-to-the-model-not-the-chat.md");
    assert!(
        amended.contains("Amended 2026-08-23, second clause"),
        "the 0044 amendment's second clause is dated"
    );
    assert!(
        flattened(&amended).contains(
            "a tool may also write the consumer's own identity-table fact when the write \
             IS the honored right"
        ),
        "the second clause states the identity-write allowance"
    );

    let unit = repo_file("docs/units/05-tools.md");
    assert!(
        unit.contains("Amended 2026-08-23, second clause (unit 11, decision 0075)"),
        "the unit-5 no-write amendment carries its second clause"
    );
}

#[test]
fn the_fixed_lines_match_the_spec_copy_verbatim() {
    use assistant_core::privacy;
    use assistant_core::tools::rights as privacy_tool;

    assert_eq!(
        privacy::OPT_OUT_DONE,
        "Understood. From now on your messages here are not collected and not answered on \
         this platform. What was stored before stays until you ask for deletion with \
         /privacydelete. Undo with /unblockprivacy."
    );
    assert_eq!(
        privacy::OPT_OUT_ALREADY,
        "You are already opted out. Undo with /unblockprivacy, or delete stored data with \
         /privacydelete."
    );
    assert_eq!(
        privacy::OPT_IN_DONE,
        "Collection is on again for you. Nothing that was deleted comes back."
    );
    assert_eq!(
        privacy::OPT_IN_ALREADY,
        "You were not opted out. Nothing changed."
    );
    assert_eq!(
        privacy::CONFIRM_INSTRUCTION,
        "To delete your stored data, reply /confirmdelete within five minutes. This \
         removes your messages and identity data and cannot be undone."
    );
    assert_eq!(
        privacy::DELETION_STARTED,
        "Deletion is underway. Your messages and identity data are being removed."
    );
    assert_eq!(
        privacy::NOTHING_PENDING,
        "There is no deletion waiting for confirmation. Start one with /privacydelete."
    );
    assert_eq!(
        privacy_tool::AMBIGUOUS_RESULT,
        "Several people spoke in this turn, so the request is not acted on. The person \
         concerned should send /privacyout or /privacydelete themselves."
    );
    assert_eq!(
        privacy_tool::INVALID_ACTION_RESULT,
        "The privacy tool accepts opt_out or request_deletion. Nothing was changed. Do \
         not retry with other words."
    );
    assert_eq!(
        privacy_tool::TRANSIENT_RESULT,
        "The change did not take effect. Nothing was recorded. The person can use \
         /privacyout or /privacydelete directly."
    );
}

#[test]
fn the_policy_edits_and_the_two_assessment_notes_ship() {
    let policy = flattened(&repo_file("docs/privacy/bot-assistant-privacy-policy.md"));
    assert!(
        policy.contains("You can also delete and object in the group."),
        "the rights section carries the in-chat sentence"
    );
    assert!(
        policy.contains(
            "`/privacydelete`, confirmed with `/confirmdelete` within five minutes, \
             removes your stored data"
        ),
        "the deletion command carries its confirmation window"
    );
    assert!(
        policy.contains(
            "the commands are the one place a machine acts, and only on your own \
             confirmed instruction"
        ),
        "the automated-decision sentence carries its carve-out"
    );
    assert!(
        policy.contains(
            "In practice we weigh nothing: `/privacyout` stops collection from that \
             moment"
        ),
        "the objection sentence records the in-place honoring"
    );
    assert!(
        policy.contains("we keep your account identifier with the opt-out mark on purpose"),
        "the deletion section names the surviving stub"
    );
    assert!(
        policy.contains("forgetting it would mean collecting your messages again"),
        "the stub is named as what remembering the objection costs"
    );
    assert!(
        policy.contains("Opt back in and ask for deletion once more, and that mark goes too."),
        "the stub's own exit is stated"
    );

    let records = flattened(&repo_file("docs/privacy/records-of-processing.md"));
    assert!(
        records.contains("Suppression flag (added 2026-08-23)"),
        "the record of processing gains the suppression flag as a data item"
    );
    assert!(
        records.contains("honoring the objection going forward"),
        "the flag's row states its purpose"
    );

    let dpia = flattened(&repo_file("docs/privacy/dpia.md"));
    assert!(
        dpia.contains(
            "Added 2026-08-23, with the privacy-self-service unit: an objection to \
             collection going forward is now honored by machine, in place"
        ),
        "the impact assessment records the honored-by-machine path"
    );
    assert!(
        dpia.contains("This is a safeguard, not an Article 22 decision"),
        "the path is recorded as the safeguard it is"
    );
}

#[test]
fn the_four_privacy_drafts_carry_their_dated_report_updates() {
    for (draft, marker) in [
        (
            "docs/privacy/bot-assistant-privacy-policy.md",
            "it reports that message to the group's moderation bot — the group's \
             administrators decide what happens, and the assistant itself takes no action",
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
            flattened(&content).contains(&flattened(marker)),
            "{draft} carries its dated update: expected {marker:?}"
        );
    }
    let records = repo_file("docs/privacy/records-of-processing.md");
    assert!(
        records.contains("The group's administrators, via the group's moderation bot"),
        "the recipients table names the report event's recipients"
    );
}

// ─── The first-interaction disclosure unit's pins (AC4, 2026-08-23) ──────

#[test]
fn the_prompt_teaches_the_honest_ai_answer() {
    let prompt = flattened(&repo_file("prompts/assistant.md"));
    assert!(
        prompt.contains("You are an AI system."),
        "the prompt names what the assistant is"
    );
    assert!(
        prompt.contains(
            "When someone asks whether you are an AI, a bot, or a machine, say yes plainly \
             and never claim to be human"
        ),
        "the prompt teaches the honest answer to the AI question"
    );
}

#[test]
fn the_disclosure_default_composes_from_the_name() {
    // The operator's original copy (decision 0079) became the composition's
    // shape with the name as its slot (unit 14): the configured `disclosure`
    // key overrides it whole, and unset composes exactly this.
    assert_eq!(
        assistant_core::composed_disclosure_line("Xenia"),
        "Hi, I'm Xenia, an AI system, made to assist members of the community."
    );
}

#[test]
fn the_compliance_record_carries_the_role_the_map_the_marking_position_and_the_notes() {
    // Blockquote markers fold away so the quoted line reads contiguously.
    let record = flattened(&repo_file("docs/compliance/ai-act.md").replace("\n> ", "\n"));

    // The role conclusion with its grounds.
    assert!(
        record.contains("The operator is the **provider** of this AI system"),
        "the record concludes the provider role"
    );
    assert!(
        record.contains("third example"),
        "the conclusion cites the guidelines' example"
    );
    assert!(
        record.contains("Article 2(10)") && record.contains("Article 2(12)"),
        "both scope exits are named and refused"
    );

    // The obligations map: minimal risk, article-cited.
    assert!(
        record.contains("Article 5 (prohibited practices): clear"),
        "the prohibited practices are cleared by name"
    );
    assert!(
        record.contains("no Annex III category")
            || record.contains("matches no Annex III category"),
        "the high-risk classes are excluded by Annex III"
    );

    // The disclosure duty and its discharge.
    assert!(
        record.contains("at the latest at the first interaction"),
        "the duty's timing is stated"
    );
    assert!(
        record.contains(&flattened(&assistant_core::composed_disclosure_line(
            "<name>"
        ))),
        "the record quotes the shipped default with its name slot"
    );

    // The Article 50(2) marking position with its two notes.
    assert!(
        record.contains("Upstream marking relied on"),
        "the marking position is the permitted reliance path"
    );
    assert!(
        record.contains("Due-diligence check: pending the first live turn"),
        "the reliance check is recorded as pending"
    );
    assert!(
        record.contains("Sub-measure 1.1.2") && record.contains("200 tokens"),
        "the practice bound is noted with its source"
    );
    assert!(
        record.contains("public industry-standard detector"),
        "the detection route is named"
    );

    // The gap analysis and the literacy note.
    assert!(
        record.contains("without signing the Code of Practice") && record.contains("para 148"),
        "the gap analysis stands in place of accession"
    );
    assert!(
        record.contains("Article 4: AI literacy") && record.contains("ongoing literacy record"),
        "the literacy note names the repository's documents as the record"
    );
}

#[test]
fn the_dpia_role_correction_is_dated_and_the_units_decisions_are_recorded() {
    let dpia = flattened(&repo_file("docs/privacy/dpia.md"));
    assert!(
        dpia.contains("noted 2026-08-23 as deployer, corrected 2026-08-23 to provider"),
        "the role paragraph corrects deployer to provider with its dates"
    );

    for record in [
        "docs/decisions/0078-the-first-answer-discloses-the-machine-from-the-ledgers-memory.md",
        "docs/decisions/0079-the-disclosure-line-is-stored-into-the-answer-block.md",
        "docs/decisions/0080-the-prompt-answers-the-ai-question-honestly.md",
        "docs/decisions/0081-the-operator-is-the-ai-act-provider.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-23"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
}

// ─── The deletion-mirror unit's pins (AC5, 2026-08-23) ───────────────────

#[test]
fn the_deletion_mirror_ships_its_three_document_updates() {
    let policy = flattened(&repo_file("docs/privacy/bot-assistant-privacy-policy.md"));
    assert!(
        policy.contains(
            "One exception: when the group's administrators delete a message through \
             the moderation bot's reply command, that message is removed from our \
             store as well."
        ),
        "the policy's deletion section carries the administrator-deletion sentence"
    );
    assert!(
        policy.contains("Only that reply form reaches us")
            && policy.contains("asking remains the way to clear those from the store"),
        "the policy scopes the exception to the reply route and names what stays outside it"
    );

    let dpia = flattened(&repo_file("docs/privacy/dpia.md"));
    assert!(
        dpia.contains("Added 2026-08-23, with the deletion-mirror unit."),
        "the DPIA's mirror paragraph is dated"
    );
    assert!(
        dpia.contains("reactive bookkeeping of an administrator's own act"),
        "the mirror is assessed as bookkeeping, not a capability"
    );
    assert!(
        dpia.contains("removes stored personal data instead of creating or disclosing any"),
        "the assessment names the mirror's direction"
    );
    assert!(
        dpia.contains("can fold the pre-erasure prose of the deleted message into a public answer"),
        "the assessment names the in-flight-turn window honestly"
    );

    let contract = flattened(&repo_file("docs/reference/group-operator-contract.md"));
    assert!(
        contract.contains("## The deletion mirror"),
        "the operator reference gains the mirror section"
    );
    assert!(
        contract.contains("the assistant must SEE the command")
            && contract.contains("only deletions issued as a reply `/del` reach it"),
        "the reference states the piggyback's constraint"
    );
    assert!(
        contract.contains("the assistant strips only its own handle from a command"),
        "the reference states the handle-suffix bound: a `/del` aimed at the \
         moderation bot by name mirrors nothing"
    );
    assert!(
        !contract.contains("always on"),
        "the reference claims no unconditional availability the bounds contradict"
    );
    assert!(
        contract.contains("bulk purges") && contract.contains("leave the stored copy in place"),
        "the reference names the forms outside the bound plainly"
    );
    assert!(
        contract.contains("the person-wide deletion commands of the privacy route remain"),
        "the reference points at the remaining route"
    );
}

#[test]
fn the_deletion_mirrors_decisions_are_recorded_with_dates_and_rejected_alternatives() {
    for record in [
        "docs/decisions/0082-the-deletion-mirror-rides-the-moderation-bots-command.md",
        "docs/decisions/0083-non-administrators-deletion-commands-mirror-nothing.md",
        "docs/decisions/0084-the-mirror-runs-inline-under-the-fence-with-the-command-stamp.md",
        "docs/decisions/0085-the-mirror-scrubs-reply-references-the-command-row-keeps-its-own.md",
        "docs/decisions/0086-the-owing-tail-walk-reads-through-erased-rows.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-23"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
    assert!(
        flattened(&repo_file(
            "docs/decisions/0082-the-deletion-mirror-rides-the-moderation-bots-command.md"
        ))
        .contains("SILENT: no reply, no acknowledgment."),
        "the mirror's silence is recorded as decided"
    );
}

// ─── The helpful-mode unit's pins (AC5, 2026-08-23) ──────────────────────

#[test]
fn the_helpful_mode_unit_ships_its_compliance_note_and_policy_sentence() {
    let record = flattened(&repo_file("docs/compliance/ai-act.md").replace("\n> ", "\n"));
    assert!(
        record.contains("Amended 2026-08-23, with the helpful-mode unit"),
        "the compliance page's disclosure amendment is dated"
    );
    assert!(
        record.contains("The duty holds under every answering mode"),
        "the note states the duty holds under every mode"
    );
    assert!(
        record.contains("the first SPOKEN answer to a person still carries the line"),
        "the note names what still discharges the duty"
    );
    assert!(
        record.contains("an abstained turn speaks nothing and therefore introduces no one"),
        "the note covers the abstention honestly"
    );

    let policy = flattened(&repo_file("docs/privacy/bot-assistant-privacy-policy.md"));
    assert!(
        policy.contains(
            "The assistant also reads group messages that do not address it, to offer \
             help when it can answer a question"
        ),
        "the policy's processing description names the helpful reading"
    );
    assert!(
        policy.contains("on the same basis and under the same limits"),
        "the sentence keeps the processing under the stated basis"
    );
}

#[test]
fn the_helpful_mode_units_decisions_are_recorded_with_dates_and_rejected_alternatives() {
    for record in [
        "docs/decisions/0087-answering-is-a-mode-and-the-summons-is-stamped-at-the-write.md",
        "docs/decisions/0088-the-model-abstains-through-a-fixed-sentinel.md",
        "docs/decisions/0089-the-name-is-one-configuration-key-with-three-effects.md",
        "docs/decisions/0090-the-disclosure-line-is-a-configured-value.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-23"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
    assert!(
        flattened(&repo_file(
            "docs/decisions/0088-the-model-abstains-through-a-fixed-sentinel.md"
        ))
        .contains("the sentinel is the whole answer or it is no abstention"),
        "the sentinel's whole-answer rule is recorded as decided"
    );
}

// ─── The autonomous-moderation unit's pins (AC7, 2026-08-24) ─────────────

#[test]
fn the_autonomous_moderation_unit_ships_its_policy_dpia_and_compliance_updates() {
    let policy = flattened(&repo_file("docs/privacy/bot-assistant-privacy-policy.md"));
    assert!(
        policy.contains(
            "It does read the group's messages and judge them against the group's \
             pinned rules"
        ),
        "the policy's moderation sentence is the assistant's own assessment"
    );
    assert!(
        policy.contains(
            "the group's administrators decide what happens, and the assistant itself \
             takes no action"
        ),
        "the policy keeps the human decision and the no-action bound"
    );
    assert!(
        policy.contains("it can misfire and report a message that broke no rule"),
        "the policy names the false positive honestly"
    );
    assert!(
        !policy.contains("when a member replies to a message and asks for one"),
        "the member-initiated description left the policy with the flow"
    );

    let dpia = flattened(&repo_file("docs/privacy/dpia.md"));
    assert!(
        dpia.contains("## 13. Addendum, 2026-08-24: the autonomous moderation assessment"),
        "the impact assessment carries the unit's addendum"
    );
    assert!(
        dpia.contains(
            "Autonomous assessment joins the stated purposes under the same legitimate \
             interest"
        ),
        "the addendum records the processing purpose under the standing basis"
    );
    assert!(
        dpia.contains("**The false-positive residual.**"),
        "the addendum records the false-positive residual by name"
    );
    assert!(
        dpia.contains("the configured reasoning level sizes the model's thinking"),
        "the addendum names the reasoning dependency"
    );
    assert!(
        dpia.contains("the output is a report to humans who decide, not an effect on the member"),
        "the addendum states the Article 22 conclusion's reason"
    );

    let record = flattened(&repo_file("docs/compliance/ai-act.md").replace("\n> ", "\n"));
    assert!(
        record.contains("Amended 2026-08-24: that standing-capability trigger fired"),
        "the compliance page answers the standing-capability trigger with its date"
    );
    assert!(
        record.contains(
            "it is not an automated decision producing legal or similarly significant \
             effects on the member"
        ),
        "the compliance page states the not-an-Article-22-decision conclusion"
    );
    assert!(
        record.contains("no reasoning trace is kept, and this record claims no audit trail"),
        "the compliance page claims no reasoning-audit trail the artifact does not keep"
    );
    assert!(
        record.contains("What the system stores is the report itself"),
        "the compliance page states what is stored instead"
    );
}

#[test]
fn the_autonomous_moderation_units_decisions_are_recorded() {
    for record in [
        "docs/decisions/0091-the-report-names-its-target-validated-against-the-assessment-set.md",
        "docs/decisions/0092-a-message-is-reported-at-most-once-per-origin.md",
        "docs/decisions/0093-the-moderation-teaching-composes-only-where-it-can-act.md",
        "docs/decisions/0094-the-rules-note-is-guaranteed-in-the-models-context.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-24"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
    let target = flattened(&repo_file(
        "docs/decisions/0091-the-report-names-its-target-validated-against-the-assessment-set.md",
    ));
    assert!(
        target.contains("Member-initiated reporting is removed as redundant"),
        "the removed member report is recorded as decided"
    );
    assert!(
        target.contains("must belong to a message the model is actually assessing this turn"),
        "the validation bound is recorded as decided"
    );
    let dedup = flattened(&repo_file(
        "docs/decisions/0092-a-message-is-reported-at-most-once-per-origin.md",
    ));
    assert!(
        dedup.contains("Per-origin dedup replaces the report window"),
        "the window's replacement is recorded as decided"
    );

    // The sibling drafts carry their dated changes beside the policy's.
    let lia = flattened(&repo_file("docs/privacy/lia.md"));
    assert!(
        lia.contains("Amended 2026-08-24: the assessment is now the assistant's own"),
        "the balancing carries its dated amendment"
    );
    let records = flattened(&repo_file("docs/privacy/records-of-processing.md"));
    assert!(
        records.contains(
            "Changed 2026-08-24: written when the assistant's own assessment finds a \
             message in clear violation"
        ),
        "the report record's row carries its dated change"
    );
}

/// The rules-acknowledgment unit's AC7 pins: no shipped document still
/// names the fixed acknowledgment as the line a rules change draws — the
/// operator contract teaches the model-generated confirmation with the
/// fixed line as its fallback, decision 0051 carries the dated refinement,
/// and the unit's two decision records exist with their dates and rejected
/// alternatives.
#[test]
fn the_rules_acknowledgment_units_docs_teach_the_generated_line_with_its_fallback() {
    let contract = flattened(&repo_file("docs/reference/group-operator-contract.md"));
    assert!(
        contract.contains("a short confirmation in its own voice, generated from the new rules"),
        "the operator contract teaches the generated acknowledgment"
    );
    assert!(
        contract.contains(
            "the deterministic fallback line delivers instead: \"Rules noted. \
             The assistant follows the pinned rules of this group.\""
        ),
        "the operator contract keeps the fixed line as the fallback, verbatim"
    );
    assert!(
        !contract.contains("with one fixed line"),
        "the contract no longer claims the fixed line as the primary"
    );

    let origin = flattened(&repo_file(
        "docs/decisions/0051-a-rules-change-is-acknowledged-with-fixed-wording.md",
    ));
    assert!(
        origin.contains("Refined 2026-08-24, with the rules-acknowledgment unit"),
        "decision 0051 carries the dated refinement toward the generated line"
    );

    // The no-disclosure records that grouped the acknowledgment among
    // human-written fixed lines are corrected: it is model output since unit
    // 20, and its no-disclosure property now rests on the structural reason
    // (it rides the observation return, never the answer edge), not on a
    // human having written it.
    let stored = flattened(&repo_file(
        "docs/decisions/0079-the-disclosure-line-is-stored-into-the-answer-block.md",
    ));
    assert!(
        stored.contains("Refined 2026-08-24, with the rules-acknowledgment unit")
            && stored.contains("rides the observation return, never the answer edge"),
        "decision 0079 corrects the acknowledgment's grouping with the structural reason"
    );
    let disclosure_unit = flattened(&repo_file("docs/units/12-first-interaction-disclosure.md"));
    assert!(
        disclosure_unit.contains("Refined 2026-08-24, unit 20")
            && disclosure_unit.contains("rides the observation return, never the answer edge"),
        "unit 12 no longer groups the acknowledgment among human-written fixed lines"
    );

    for record in [
        "docs/decisions/0104-the-rules-acknowledgment-is-a-bounded-one-shot-completion.md",
        "docs/decisions/0105-the-fixed-line-is-the-acknowledgments-fallback.md",
    ] {
        let content = repo_file(record);
        assert!(
            content.contains("Date: 2026-08-24"),
            "{record} carries its date"
        );
        assert!(
            content.contains("## Rejected alternatives"),
            "{record} carries its rejected alternatives"
        );
    }
    let generation = flattened(&repo_file(
        "docs/decisions/0104-the-rules-acknowledgment-is-a-bounded-one-shot-completion.md",
    ));
    assert!(
        generation.contains("a request timeout, an output cap"),
        "the generation record names its bounds"
    );
    let fallback = flattened(&repo_file(
        "docs/decisions/0105-the-fixed-line-is-the-acknowledgments-fallback.md",
    ));
    assert!(
        fallback.contains("ALWAYS draws a visible acknowledgment"),
        "the fallback record states the delivery guarantee"
    );
}
