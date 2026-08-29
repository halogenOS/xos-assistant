//! The prompt sections the assembly composes from its configuration
//! (unit 14, 2026-08-23): the name identity, the answering-mode teaching,
//! and — since unit 15, 2026-08-24 — the moderation teaching for the
//! deployments that can act on it.
//!
//! The embedder's prompt files stay prose an operator edits; what depends
//! on configuration — the resolved name, the answering mode, the
//! moderation capability — is behavior and composes here, in the core, so
//! the wording cannot drift from the mechanism that reads it. The
//! composition joins the configured prompt first and the composed sections
//! after it, and the assembly records the result as every new
//! conversation's system prompt; like any prompt edit, a changed name,
//! mode or moderation handle reaches new conversations only.
//!
//! Silence needs no vocabulary of its own (unit 22, 2026-08-24): the model
//! stays silent by ending its turn without writing any text, the framework
//! commits that turn as a real empty answer, and the outbound edge
//! delivers it as nothing. When addressed and unable to back an answer
//! with a lookup, the model says it doesn't know in its own words —
//! ordinary prose, no machine routing.

use crate::assembly::AnsweringMode;

/// What this deployment can actually do, as the composition needs to know
/// it: one field per capability whose teaching is gated on its own
/// mechanism existing (unit 27, 2026-08-29). Named fields rather than a row
/// of positional booleans — two adjacent flags at a call site are one
/// silent swap away from teaching a tool the palette does not carry, which
/// is the exact defect this gating exists to prevent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// A moderation handle is configured — the report tool's half of its
    /// predicate; the answering mode is the other half.
    pub moderation_handle: bool,
    /// A web-search key is configured, which is the search tool's whole
    /// predicate: with no key the tool is not admitted and its teaching is
    /// not composed.
    pub web_search: bool,
}

/// Whether the moderation teaching composes — and, in the assembly, whether
/// the report tool registers: a moderation handle is configured AND the
/// answering mode is helpful, the two conditions autonomous assessment
/// needs (the report line goes nowhere without a handle; only helpful
/// answering shows the model every message it would judge). One predicate
/// for the teaching and the registration, so the prompt can never instruct
/// a tool the palette does not carry, and a registered tool is never left
/// untaught (unit 15, 2026-08-24).
#[must_use]
pub fn moderation_taught(handle_configured: bool, answering: AnsweringMode) -> bool {
    handle_configured && answering == AnsweringMode::Helpful
}

/// The moderation teaching, verbatim (unit 15, 2026-08-24): the model
/// judges each message against the pinned rules and reports a clear
/// violation through the report tool, naming the message by the bracketed
/// id the projection shows — reasoned first, assessment only, the
/// administrators decide (decision 0070). The no-report cases are taught
/// beside the capability: borderline calls, rule-absent messages, and
/// every message while no rules are pinned. The pinned statement is named
/// the group's ONLY rules source, and its absence is taught as honestly as
/// its content: with no rules statement present the group has no rules
/// set — the model says so plainly, invents none, and reports nothing
/// (AC8's no-rules half, beside the base prose that carries no rules).
pub const MODERATION_TEACHING: &str = "You also assess each group message against the group's pinned rules, \
     shown to you as the newest rules statement. That statement is the \
     group's only rules source: when no rules statement is present, the \
     group has no rules set — say so plainly if asked, invent none, and \
     report nothing. When a message clearly \
     violates those rules, think it through first, then file a report with \
     the report_spam tool, naming the violating message by the bracketed id \
     shown ahead of it. Report only clear violations: do not report \
     borderline calls, messages no rule covers, or anything when no rules \
     are pinned. A report is an assessment for the group's administrators, \
     who decide; you never ban, mute or remove anyone. Reporting and \
     answering are independent: you may report and still answer, or report \
     and stay silent. The tool is the only way to report — never write the \
     report command into an answer yourself.";

/// The web search teaching, verbatim (unit 27, 2026-08-29), composed
/// exactly when the search tool is admitted — one predicate for both, so
/// the prompt never teaches a tool the palette does not carry. Three
/// things: a snippet is a hint and an answer built on one says where it
/// came from; a snippet that does not contain the claim is a miss, exactly
/// as the sourcing rule already says of a lookup; and the carve-out that
/// makes registering a web tool safe at all — project facts still come only
/// from the project lookups. Without that last sentence, the sourcing
/// rule's "your lookup tools are the only source of substantive claims"
/// would silently authorise a random web page to back a claim about the
/// project.
pub const SEARCH_TEACHING: &str = "You can also search the web with the search_web tool, for questions about \
     the world rather than about the project. A result's snippet is a hint, \
     not a source: when you answer from one, say where it came from and name \
     the page. A snippet that does not contain the claim is a miss, exactly \
     as an unanswering lookup is — say you don't know rather than filling the \
     gap from memory. Facts about the project itself still come only from the \
     project lookups: a web result is never the source for a claim about \
     halogenOS, its features, its procedures or its builds.";

/// The whole system prompt the assembly records: the embedder's prompt,
/// then the name identity, then the answering teaching for the configured
/// mode, then — each exactly when its own capability is there — the
/// moderation teaching and the web search teaching. Public because the
/// suites pin recorded prompts against exactly this composition instead of
/// restating it.
#[must_use]
pub fn composed_system_prompt(
    base: &str,
    name: &str,
    answering: AnsweringMode,
    capabilities: Capabilities,
) -> String {
    let mut prompt = format!(
        "{base}\n\n{identity}\n\n{teaching}",
        identity = identity_section(name),
        teaching = answering_section(answering),
    );
    if moderation_taught(capabilities.moderation_handle, answering) {
        prompt.push_str("\n\n");
        prompt.push_str(MODERATION_TEACHING);
    }
    if capabilities.web_search {
        prompt.push_str("\n\n");
        prompt.push_str(SEARCH_TEACHING);
    }
    prompt
}

/// The name identity: what the assistant is called, that the
/// are-you-a-bot question about that name is a question about itself —
/// answered honestly, per decision 0080's teaching — and, since unit 32
/// (2026-08-28), where the answer to what-are-you-running-on comes from:
/// the runtime-facts tool, never the model's memory and never the chat,
/// both of which state a model the deployment may have moved off. The
/// sentence composes in both modes and under every configuration, because
/// the tool it names registers the same way.
fn identity_section(name: &str) -> String {
    format!(
        "You are called {name}. When someone asks whether {name} is an AI, \
         a bot, or a machine, that question is about you: answer it honestly, \
         as the AI system you are. When someone asks which model you run on, \
         which version you are, or how long you have been running, call the \
         {tool} tool and answer from what it returns — never from memory and \
         never from what this conversation said earlier.",
        tool = crate::tools::runtime::NAME
    )
}

/// The answering teaching for one mode (rewritten by unit 16 and unit 22,
/// extended by unit 21, 2026-08-24). Both modes teach the sourcing
/// discipline, the audience discipline and the end-empty silence — a turn
/// with nothing to say ends with no text wherever it runs — and the
/// helpful mode adds the silence-default judgment for messages that never
/// addressed the assistant, while the addressed mode adds the follow-up
/// reach: an unaddressed reply never opens a turn there, so a clarifying
/// question invites the member to reply to it.
fn answering_section(answering: AnsweringMode) -> String {
    let sourcing = sourcing_rules();
    let audience = audience_rules();
    match answering {
        AnsweringMode::Helpful => format!(
            "Every message in a group conversation reaches you, including \
             messages that do not address you, and you decide whether to \
             speak. Silence is the default: a statement that asks nothing, a \
             message setting up group content, members talking among \
             themselves — none of these warrant a reply, and if someone else \
             already answered a question well, stay silent or briefly defer \
             to them. When you have nothing to add, end your turn without \
             writing any text — no placeholder. An answer is the exception, \
             and an answer that makes a substantive claim must be one you \
             can back with a lookup. {sourcing} {audience}"
        ),
        AnsweringMode::Addressed => format!(
            "You are brought in when a message addresses you: a mention, a \
             reply to one of your messages, your name, or a direct chat. \
             Answer what was asked of you; when even an addressed message \
             leaves you nothing useful to say, end your turn without \
             writing any text — no placeholder. \
             {sourcing} {audience} When you ask a clarifying question, \
             invite the member to reply to your message: only a message \
             that addresses you reaches you, so a plain follow-up would \
             otherwise go unseen."
        ),
    }
}

/// The sourcing discipline, shared by both modes so the operator's rule has
/// one spelling: the tools are the only source of substantive claims, the
/// lookup comes before the answer, an unanswering lookup is a miss, no
/// guesses and no hedged trained knowledge — and the honest outcomes in
/// the model's own voice: a plain "I don't know" when addressed, no text
/// at all otherwise. The addressed-versus-silent choice is the model's
/// own judgment.
fn sourcing_rules() -> String {
    "Your lookup tools are the only source of substantive claims: any \
     claim about the project — a feature, a procedure, a project fact — \
     must come from a lookup you made in this turn, never from your \
     trained knowledge, so look it up before you answer. A lookup \
     answers a question only when its result actually contains the \
     answer: a result that is empty, off-topic, or missing the specific \
     claim is a miss, not a licence to fill the gap from memory. Never \
     guess and never answer from hedged memory — no \"as far as I \
     know\", no \"probably\" — and in a compound answer, every \
     project-specific claim is either confirmed by a lookup or dropped. \
     When you were addressed and a lookup cannot back the answer, say \
     you don't know, plainly and in your own words — never guess from \
     memory or offer a hedged recollection. When you were not addressed \
     and have nothing to add, end the turn with no text."
        .to_owned()
}

/// The audience discipline, shared by both modes so it applies wherever a
/// question is answered (unit 21, 2026-08-24): the same question reads one
/// way to an end user and another to a builder, the audience is read from
/// the message and the conversation — never from a profile of the person —
/// genuine ambiguity draws exactly one brief clarifying question, a clear
/// question is answered directly, and clarifying questions never chain.
/// The lookup-backing rule is reconciled here, narrowly: a question back
/// to the asker makes no substantive claim and needs no lookup, while the
/// real answer it unlocks needs its lookup exactly as before.
fn audience_rules() -> &'static str {
    "The same question often reads one way from an end user who wants to \
     use a feature on their device and another from a developer who wants \
     to build it into a ROM or integrate it, and the right answer differs \
     sharply between the two. Read the audience from what the message and \
     the conversation show — the words, the level, the prior turns — never \
     from assumptions about who the person is. When the intent is clear, \
     answer that reading directly. When a question is genuinely ambiguous \
     between using and building, ask one brief clarifying question — \"are \
     you asking how to use it on your device, or how to build it into a \
     ROM?\" — instead of committing to an assumption. A clarifying question \
     back to the asker is a warranted reply, not silence and not a \
     don't-know: it makes no substantive claim, so it needs no lookup, \
     while the real answer that follows the member's reply needs its \
     lookup like any substantive claim. Never chain clarifying questions: \
     when the reply still leaves the intent unclear, answer the likeliest \
     reading as well as your lookups allow instead of asking again."
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capabilities of a deployment with a moderation handle and no
    /// search key.
    fn moderating() -> Capabilities {
        Capabilities {
            moderation_handle: true,
            web_search: false,
        }
    }

    /// The capabilities of a deployment with a search key and no moderation
    /// handle.
    fn searching() -> Capabilities {
        Capabilities {
            moderation_handle: false,
            web_search: true,
        }
    }

    /// The composition order and the three facts the sections must carry:
    /// the base leads, the name reaches the identity, and each mode's
    /// teaching states the end-empty silence mechanism.
    #[test]
    fn the_prompt_composes_base_identity_and_mode_teaching() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt =
                composed_system_prompt("The base prose.", "Probe", mode, Capabilities::default());
            assert!(
                prompt.starts_with("The base prose.\n\n"),
                "the embedder's prompt leads"
            );
            assert!(
                prompt.contains("You are called Probe."),
                "the identity names the assistant"
            );
            assert!(
                prompt.contains("end your turn without writing any text — no placeholder"),
                "the teaching states silence as the empty turn"
            );
        }
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains("including messages that do not address you"),
            "helpful mode teaches the undirected reach"
        );
        let addressed =
            composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default());
        assert!(
            addressed.contains("when a message addresses you"),
            "addressed mode teaches the summons shape"
        );
    }

    /// AC6 (unit 32): both modes' composed prompt routes the
    /// what-are-you-running-on question to the runtime-facts tool, under
    /// every configuration — the sentence composes on nothing, because
    /// the tool it names registers on nothing.
    #[test]
    fn both_modes_route_identity_questions_to_the_runtime_tool() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for handle in [false, true] {
                let prompt = composed_system_prompt("b", "n", mode, handle);
                assert!(
                    prompt.contains(
                        "When someone asks which model you run on, which version you \
                         are, or how long you have been running, call the runtime_facts \
                         tool and answer from what it returns — never from memory and \
                         never from what this conversation said earlier."
                    ),
                    "the {mode:?} teaching routes the question to the tool"
                );
            }
        }
    }

    /// AC6 (unit 22, continuing unit 16's AC7): both modes' teaching
    /// carries the sourcing discipline verbatim — the tools as the only
    /// source of substantive claims, the lookup before the answer, the
    /// sufficiency rule that an unanswering lookup is a miss, the
    /// no-guessing and no-hedged-knowledge prohibition, and the honest
    /// outcomes in the model's own voice: the plain "I don't know" when
    /// addressed, the empty turn otherwise — the choice between them being
    /// the model's own reading of the message. The addressed mode's gain
    /// is exactly this discipline; the silence-default framing is pinned
    /// in the test below.
    #[test]
    fn both_modes_teach_the_sourcing_discipline() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for fact in [
                "Your lookup tools are the only source of substantive claims",
                "never from your trained knowledge, so look it up before you answer",
                "a result that is empty, off-topic, or missing the specific \
                 claim is a miss, not a licence to fill the gap from memory",
                "Never guess and never answer from hedged memory",
                "every project-specific claim is either confirmed by a lookup or dropped",
                "When you were addressed and a lookup cannot back the answer, \
                 say you don't know, plainly and in your own words — never \
                 guess from memory or offer a hedged recollection",
                "When you were not addressed and have nothing to add, end \
                 the turn with no text",
            ] {
                assert!(
                    prompt.contains(fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
    }

    /// AC2 (unit 21): both modes' teaching carries the audience discipline
    /// verbatim — the use-versus-build distinction, the read-the-message
    /// never-profile-the-person rule, the answer-directly-when-clear rule,
    /// the ask-one-brief-clarifying-question-on-genuine-ambiguity rule, the
    /// do-not-chain rule, and the reconciled lookup statement: a clarifying
    /// question is a warranted reply needing no lookup, while a substantive
    /// claim still needs one. The helpful mode's silence framing binds the
    /// lookup duty to the substantive claim, and the addressed mode names
    /// its follow-up reach.
    #[test]
    fn both_modes_teach_the_audience_discipline() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for fact in [
                "one way from an end user who wants to use a feature on \
                 their device and another from a developer who wants to \
                 build it into a ROM or integrate it",
                "Read the audience from what the message and the \
                 conversation show — the words, the level, the prior turns \
                 — never from assumptions about who the person is",
                "When the intent is clear, answer that reading directly.",
                "When a question is genuinely ambiguous between using and \
                 building, ask one brief clarifying question",
                "A clarifying question back to the asker is a warranted \
                 reply, not silence and not a don't-know: it makes no \
                 substantive claim, so it needs no lookup",
                "the real answer that follows the member's reply needs its \
                 lookup like any substantive claim",
                "Never chain clarifying questions: when the reply still \
                 leaves the intent unclear, answer the likeliest reading as \
                 well as your lookups allow instead of asking again",
            ] {
                assert!(
                    prompt.contains(fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains(
                "an answer that makes a substantive claim must be one you \
                 can back with a lookup"
            ),
            "the reconciled helpful sentence binds the lookup to the claim"
        );
        let addressed =
            composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default());
        assert!(
            addressed.contains(
                "When you ask a clarifying question, invite the member to \
                 reply to your message"
            ),
            "the addressed teaching names its follow-up reach"
        );
    }

    /// AC6's silence half (unit 22): helpful mode leads with silence as
    /// the default and keeps the unit-16 lookup-backed sentence verbatim —
    /// the new end-empty rule completes it, it does not contradict it —
    /// and no sentinel vocabulary survives anywhere in either mode's
    /// prompt.
    #[test]
    fn silence_is_the_default_and_no_sentinel_vocabulary_survives() {
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains("Silence is the default"),
            "helpful mode leads with silence"
        );
        assert!(
            helpful.contains(
                "an answer that makes a substantive claim must be one you \
                 can back with a lookup"
            ),
            "the unit-16 lookup-backed sentence stands verbatim"
        );
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for token in ["[[", "]]", "abstain", "abstention", "sentinel"] {
                assert!(
                    !prompt.to_lowercase().contains(token),
                    "the {mode:?} teaching carries no sentinel vocabulary: {token}"
                );
            }
            assert!(
                prompt.contains("end your turn without writing any text — no placeholder"),
                "the {mode:?} teaching states the empty turn as the silence mechanism"
            );
        }
    }

    /// AC7's prompt half at the composition itself: the moderation teaching
    /// rides the prompt exactly when a handle is configured AND the mode is
    /// helpful — absent either condition, no moderation instruction exists
    /// for a tool that is not there.
    #[test]
    fn the_moderation_teaching_composes_only_with_a_handle_and_helpful_mode() {
        let taught = composed_system_prompt("b", "n", AnsweringMode::Helpful, moderating());
        assert!(
            taught.ends_with(MODERATION_TEACHING),
            "handle plus helpful composes the moderation teaching last"
        );
        for (name, prompt) in [
            (
                "helpful without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default()),
            ),
            (
                "addressed with a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, moderating()),
            ),
            (
                "addressed without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default()),
            ),
        ] {
            assert!(
                !prompt.contains("report_spam"),
                "{name} teaches no moderation"
            );
        }
        assert!(moderation_taught(true, AnsweringMode::Helpful));
        assert!(!moderation_taught(false, AnsweringMode::Helpful));
        assert!(!moderation_taught(true, AnsweringMode::Addressed));
    }

    /// AC5's prompt half and AC9's carve-out (unit 27): the search teaching
    /// rides the prompt exactly when a search key is configured, in either
    /// answering mode — and with no key, no sentence of it exists for a
    /// tool that is not there.
    #[test]
    fn the_search_teaching_composes_only_with_a_configured_key() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let taught = composed_system_prompt("b", "n", mode, searching());
            assert!(
                taught.ends_with(SEARCH_TEACHING),
                "a configured key composes the search teaching last in {mode:?} mode"
            );
            let untaught = composed_system_prompt("b", "n", mode, Capabilities::default());
            assert!(
                !untaught.contains("search_web"),
                "no key teaches no search in {mode:?} mode"
            );
        }
        // Both capabilities at once compose both teachings, each whole.
        let both = composed_system_prompt(
            "b",
            "n",
            AnsweringMode::Helpful,
            Capabilities {
                moderation_handle: true,
                web_search: true,
            },
        );
        assert!(both.contains(MODERATION_TEACHING) && both.contains(SEARCH_TEACHING));
    }

    /// The search teaching's copy, pinned on the three things it must
    /// carry — above all the carve-out, without which registering a web
    /// tool would silently authorise a web page to back a project claim.
    #[test]
    fn the_search_teaching_carries_the_snippet_rules_and_the_project_carve_out() {
        for fact in [
            "the search_web tool",
            "for questions about the world rather than about the project",
            "A result's snippet is a hint, not a source",
            "say where it came from and name the page",
            "A snippet that does not contain the claim is a miss",
            "Facts about the project itself still come only from the project lookups",
            "a web result is never the source for a claim about halogenOS",
        ] {
            assert!(
                SEARCH_TEACHING.contains(fact),
                "the search teaching carries: {fact}"
            );
        }
    }

    /// The moderation teaching's copy, pinned on the facts it must carry:
    /// the tool, the bracketed-id naming, the reasoned judgment, the
    /// only-rules-source rule with its no-rules honesty (AC8's no-rules
    /// half), the no-report cases (AC4's judgment half), the decision
    /// boundary of 0070, the report-answer independence, and the
    /// never-type-the-command rule.
    #[test]
    fn the_moderation_teaching_carries_its_facts() {
        for fact in [
            "the report_spam tool",
            "naming the violating message by the bracketed id",
            "think it through first",
            "That statement is the group's only rules source",
            "when no rules statement is present, the group has no rules \
             set — say so plainly if asked, invent none, and report nothing",
            "do not report borderline calls, messages no rule covers, or \
             anything when no rules are pinned",
            "an assessment for the group's administrators, who decide",
            "you never ban, mute or remove anyone",
            "Reporting and answering are independent",
            "never write the report command into an answer yourself",
        ] {
            assert!(
                MODERATION_TEACHING.contains(fact),
                "the moderation teaching carries: {fact}"
            );
        }
    }
}
