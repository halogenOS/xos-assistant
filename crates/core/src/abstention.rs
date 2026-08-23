//! The abstention sentinel: how the model chooses silence (unit 14,
//! 2026-08-23).
//!
//! The prompt teaches the model to answer only when it can genuinely help
//! and to stay silent otherwise by emitting the fixed sentinel as its WHOLE
//! answer. Three readers share the recognition and must agree, so the
//! predicate lives here alone:
//!
//! - the outbound edge, which delivers a recognized abstention as nothing —
//!   no send, no first-interaction introduction — and accounts it delivered;
//! - the composed kind's projection, which keeps a recognized abstention out
//!   of every later model request;
//! - the budget counts, whose SQL excludes a debt answered by a recognized
//!   abstention, because the answer window bounds what the assistant SAYS
//!   and an abstained turn said nothing.
//!
//! Recognition is exact on the raw trimmed finalized content: the sentinel
//! is the whole answer or it is no abstention at all, so an ordinary answer
//! that merely quotes the sentinel's words as prose is never swallowed.

/// The fixed sentinel the model emits as its whole answer to stay silent.
/// A named constant on purpose: the prompt teaches this exact spelling, the
/// recognition compares against it, and the count's SQL parameter carries
/// it — one value, three readers.
pub const ABSTENTION_SENTINEL: &str = "[[abstain]]";

/// Whether a finalized answer is a recognized abstention: the raw content,
/// trimmed, equals the sentinel exactly.
#[must_use]
pub fn is_abstention(content: &str) -> bool {
    content.trim() == ABSTENTION_SENTINEL
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC4's recognition boundary: the sentinel is the whole answer or
    /// nothing — surrounding whitespace is tolerated, surrounding prose is
    /// not, and the sentinel's words inside an ordinary answer never match.
    #[test]
    fn recognition_is_exact_on_the_trimmed_whole_answer() {
        assert!(is_abstention(ABSTENTION_SENTINEL));
        assert!(is_abstention(&format!("  {ABSTENTION_SENTINEL}\n")));
        assert!(!is_abstention(&format!(
            "I will reply with {ABSTENTION_SENTINEL} when I have nothing to add."
        )));
        assert!(!is_abstention(&format!("{ABSTENTION_SENTINEL} — noted.")));
        assert!(!is_abstention("abstain"));
        assert!(!is_abstention(""));
    }
}
