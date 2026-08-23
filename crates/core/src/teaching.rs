//! The prompt sections the assembly composes from its configuration
//! (unit 14, 2026-08-23): the name identity, and the answering-mode
//! teaching with the abstention sentinel.
//!
//! The embedder's prompt files stay prose an operator edits; what depends
//! on configuration — the resolved name, the answering mode, the sentinel's
//! exact spelling — is behavior and composes here, in the core, so the
//! wording cannot drift from the mechanism that reads it. The composition
//! joins the configured prompt first and the composed sections after it,
//! and the assembly records the result as every new conversation's system
//! prompt; like any prompt edit, a changed name or mode reaches new
//! conversations only.

use crate::abstention::ABSTENTION_SENTINEL;
use crate::assembly::AnsweringMode;

/// The whole system prompt the assembly records: the embedder's prompt,
/// then the name identity, then the answering teaching for the configured
/// mode. Public because the suites pin recorded prompts against exactly
/// this composition instead of restating it.
#[must_use]
pub fn composed_system_prompt(base: &str, name: &str, answering: AnsweringMode) -> String {
    format!(
        "{base}\n\n{identity}\n\n{teaching}",
        identity = identity_section(name),
        teaching = answering_section(answering),
    )
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

/// The answering teaching for one mode. Both modes teach the sentinel —
/// silence must have a mechanism wherever a turn runs — and the helpful
/// mode adds the judgment for messages that never addressed the assistant.
fn answering_section(answering: AnsweringMode) -> String {
    let sentinel = format!(
        "To stay silent, reply with exactly {ABSTENTION_SENTINEL} and nothing \
         else: that reply is swallowed and no message reaches the chat. Never \
         put {ABSTENTION_SENTINEL} inside an ordinary answer."
    );
    match answering {
        AnsweringMode::Helpful => format!(
            "Every message in a group conversation reaches you, including \
             messages that do not address you, and you decide whether to \
             speak. Answer only when you can genuinely help: a real question \
             you can answer from your sources or your own knowledge, or a \
             message that otherwise warrants a reply. Stay silent for members \
             talking among themselves, for anything you have no information \
             on, and when a lookup comes back empty — never guess an \
             answer. If someone else already answered a question well, stay \
             silent or briefly defer to them. {sentinel}"
        ),
        AnsweringMode::Addressed => format!(
            "You are brought in when a message addresses you: a mention, a \
             reply to one of your messages, your name, or a direct chat. \
             Answer what was asked of you; when even an addressed message \
             leaves you nothing useful to say, you may stay silent. {sentinel}"
        ),
    }
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
            let prompt = composed_system_prompt("The base prose.", "Probe", mode);
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
        let helpful = composed_system_prompt("b", "n", AnsweringMode::Helpful);
        assert!(
            helpful.contains("including messages that do not address you"),
            "helpful mode teaches the undirected reach"
        );
        let addressed = composed_system_prompt("b", "n", AnsweringMode::Addressed);
        assert!(
            addressed.contains("when a message addresses you"),
            "addressed mode teaches the summons shape"
        );
    }
}
