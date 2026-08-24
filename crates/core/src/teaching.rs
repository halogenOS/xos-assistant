//! The prompt sections the assembly composes from its configuration
//! (unit 14, 2026-08-23): the name identity, the answering-mode teaching
//! with the abstention sentinel, and — since unit 15, 2026-08-24 — the
//! moderation teaching for the deployments that can act on it.
//!
//! The embedder's prompt files stay prose an operator edits; what depends
//! on configuration — the resolved name, the answering mode, the sentinel's
//! exact spelling, the moderation capability — is behavior and composes
//! here, in the core, so the wording cannot drift from the mechanism that
//! reads it. The composition joins the configured prompt first and the
//! composed sections after it, and the assembly records the result as
//! every new conversation's system prompt; like any prompt edit, a changed
//! name, mode or moderation handle reaches new conversations only.

use crate::abstention::{ABSTENTION_SENTINEL, MISS_SENTINEL};
use crate::assembly::AnsweringMode;

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

/// The whole system prompt the assembly records: the embedder's prompt,
/// then the name identity, then the answering teaching for the configured
/// mode, then — exactly when [`moderation_taught`] holds — the moderation
/// teaching. Public because the suites pin recorded prompts against
/// exactly this composition instead of restating it.
#[must_use]
pub fn composed_system_prompt(
    base: &str,
    name: &str,
    answering: AnsweringMode,
    moderation_handle_configured: bool,
) -> String {
    let mut prompt = format!(
        "{base}\n\n{identity}\n\n{teaching}",
        identity = identity_section(name),
        teaching = answering_section(answering),
    );
    if moderation_taught(moderation_handle_configured, answering) {
        prompt.push_str("\n\n");
        prompt.push_str(MODERATION_TEACHING);
    }
    prompt
}

/// The name identity: what the assistant is called, and that the
/// are-you-a-bot question about that name is a question about itself —
/// answered honestly, per decision 0080's teaching.
fn identity_section(name: &str) -> String {
    format!(
        "You are called {name}. When someone asks whether {name} is an AI, \
         a bot, or a machine, that question is about you: answer it honestly, \
         as the AI system you are."
    )
}

/// The answering teaching for one mode (rewritten by unit 16, 2026-08-24).
/// Both modes teach the sourcing discipline and both sentinels — silence
/// and the honest miss must have a mechanism wherever a turn runs — and
/// the helpful mode adds the silence-default judgment for messages that
/// never addressed the assistant.
fn answering_section(answering: AnsweringMode) -> String {
    let sourcing = sourcing_rules();
    let sentinels = sentinel_rules();
    match answering {
        AnsweringMode::Helpful => format!(
            "Every message in a group conversation reaches you, including \
             messages that do not address you, and you decide whether to \
             speak. Silence is the default: a statement that asks nothing, a \
             message setting up group content, members talking among \
             themselves — none of these warrant a reply, and if someone else \
             already answered a question well, stay silent or briefly defer \
             to them. An answer is the exception, and it must be one you can \
             back with a lookup. {sourcing} {sentinels}"
        ),
        AnsweringMode::Addressed => format!(
            "You are brought in when a message addresses you: a mention, a \
             reply to one of your messages, your name, or a direct chat. \
             Answer what was asked of you; when even an addressed message \
             leaves you nothing useful to say, you may stay silent. \
             {sourcing} {sentinels}"
        ),
    }
}

/// The sourcing discipline, shared by both modes so the operator's rule has
/// one spelling: the tools are the only source of substantive claims, the
/// lookup comes before the answer, an unanswering lookup is a miss, no
/// guesses and no hedged trained knowledge — and the miss sentinel as the
/// honest whole-answer signal, whose outcome the machinery decides.
fn sourcing_rules() -> String {
    format!(
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
         When you looked and could not confirm an answer, reply with exactly \
         {MISS_SENTINEL} and nothing else: whether the asker is told you \
         don't know, or nothing is said, is decided for you."
    )
}

/// The two sentinels with their distinct meanings, shared by both modes:
/// social silence and the unresolved lookup are different facts, and the
/// mechanism that routes them can only tell them apart if the model never
/// uses one for the other.
fn sentinel_rules() -> String {
    format!(
        "To stay silent, reply with exactly {ABSTENTION_SENTINEL} and \
         nothing else: that reply is swallowed and no message reaches the \
         chat. The two sentinels mean different things — {ABSTENTION_SENTINEL} \
         is social silence, nothing to add; {MISS_SENTINEL} is an unresolved \
         lookup, you looked and found nothing — never use one for the other, \
         and never put either inside an ordinary answer."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The composition order and the three facts the sections must carry:
    /// the base leads, the name reaches the identity, and each mode's
    /// teaching names the sentinel exactly once as the silence mechanism.
    #[test]
    fn the_prompt_composes_base_identity_and_mode_teaching() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("The base prose.", "Probe", mode, false);
            assert!(
                prompt.starts_with("The base prose.\n\n"),
                "the embedder's prompt leads"
            );
            assert!(
                prompt.contains("You are called Probe."),
                "the identity names the assistant"
            );
            assert!(
                prompt.contains(ABSTENTION_SENTINEL),
                "the teaching carries the sentinel's exact spelling"
            );
        }
        let helpful = composed_system_prompt("b", "n", AnsweringMode::Helpful, false);
        assert!(
            helpful.contains("including messages that do not address you"),
            "helpful mode teaches the undirected reach"
        );
        let addressed = composed_system_prompt("b", "n", AnsweringMode::Addressed, false);
        assert!(
            addressed.contains("when a message addresses you"),
            "addressed mode teaches the summons shape"
        );
    }

    /// AC7 (unit 16): both modes' teaching carries the sourcing discipline
    /// verbatim — the tools as the only source of substantive claims, the
    /// lookup before the answer, the sufficiency rule that an unanswering
    /// lookup is a miss, the no-guessing and no-hedged-knowledge
    /// prohibition, and the miss sentinel as the whole-answer signal. The
    /// addressed mode's gain is exactly this discipline; the
    /// silence-default framing and the sentinel distinction are pinned in
    /// the test below.
    #[test]
    fn both_modes_teach_the_sourcing_discipline() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, false);
            for fact in [
                "Your lookup tools are the only source of substantive claims".to_owned(),
                "never from your trained knowledge, so look it up before you answer".to_owned(),
                "a result that is empty, off-topic, or missing the specific \
                 claim is a miss, not a licence to fill the gap from memory"
                    .to_owned(),
                "Never guess and never answer from hedged memory".to_owned(),
                "every project-specific claim is either confirmed by a lookup or dropped"
                    .to_owned(),
                format!(
                    "When you looked and could not confirm an answer, reply \
                     with exactly {MISS_SENTINEL} and nothing else"
                ),
            ] {
                assert!(
                    prompt.contains(&fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
    }

    /// AC7's silence and sentinel half (unit 16): helpful mode leads with
    /// silence as the default, and both modes name the two sentinels with
    /// their distinct meanings — social silence against the unresolved
    /// lookup — plus the never-inside-an-answer rule.
    #[test]
    fn silence_is_the_default_and_the_sentinels_carry_distinct_meanings() {
        let helpful = composed_system_prompt("b", "n", AnsweringMode::Helpful, false);
        assert!(
            helpful.contains("Silence is the default"),
            "helpful mode leads with silence"
        );
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, false);
            assert!(
                prompt.contains(&format!(
                    "{ABSTENTION_SENTINEL} is social silence, nothing to add; \
                     {MISS_SENTINEL} is an unresolved lookup, you looked and \
                     found nothing"
                )),
                "the {mode:?} teaching tells the sentinels apart"
            );
            assert!(
                prompt.contains(
                    "never use one for the other, and never put \
                 either inside an ordinary answer"
                ),
                "the {mode:?} teaching bounds both sentinels to the whole answer"
            );
        }
    }

    /// AC7's prompt half at the composition itself: the moderation teaching
    /// rides the prompt exactly when a handle is configured AND the mode is
    /// helpful — absent either condition, no moderation instruction exists
    /// for a tool that is not there.
    #[test]
    fn the_moderation_teaching_composes_only_with_a_handle_and_helpful_mode() {
        let taught = composed_system_prompt("b", "n", AnsweringMode::Helpful, true);
        assert!(
            taught.ends_with(MODERATION_TEACHING),
            "handle plus helpful composes the moderation teaching last"
        );
        for (name, prompt) in [
            (
                "helpful without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Helpful, false),
            ),
            (
                "addressed with a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, true),
            ),
            (
                "addressed without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, false),
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
