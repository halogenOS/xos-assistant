//! The answer sentinels: how the model chooses silence (unit 14,
//! 2026-08-23) and how it admits an unresolved lookup (unit 16,
//! 2026-08-24).
//!
//! Two sentinels, two meanings, kept distinct on purpose:
//!
//! - the ABSTENTION is social silence — members talking among themselves,
//!   nothing to add — and always delivers nothing;
//! - the MISS is an unresolved lookup — the model looked and could not
//!   confirm an answer — and the outbound edge routes it by the stored
//!   literal-addressed fact of the turn's dispatch-anchor message:
//!   unaddressed delivers nothing, addressed delivers the fixed
//!   don't-know line. One sentinel for both would let an addressed "lol"
//!   draw a "don't know", which is why the meanings never share a
//!   spelling.
//!
//! The prompt teaches each sentinel as the model's WHOLE answer, and the
//! readers share the recognition, so the predicates live here alone. For
//! the abstention those readers are the outbound edge, the composed
//! kind's projection, and the budget counts' SQL exclusion; for the miss
//! the reader is the outbound edge's routing.
//!
//! Recognition is exact on the raw trimmed finalized content: a sentinel
//! is the whole answer or it is no sentinel at all, so an ordinary answer
//! that merely quotes a sentinel's words as prose is never swallowed.

/// The fixed sentinel the model emits as its whole answer to stay silent.
/// A named constant on purpose: the prompt teaches this exact spelling, the
/// recognition compares against it, and the count's SQL parameter carries
/// it — one value, three readers.
pub const ABSTENTION_SENTINEL: &str = "[[abstain]]";

/// The fixed sentinel the model emits as its whole answer when it looked
/// and could not confirm an answer (unit 16, 2026-08-24). The model's only
/// job is honesty about the unresolved lookup; whether the asker is owed a
/// spoken "don't know" or silence is the machine's decision, made at the
/// outbound edge from the stored literal-addressed fact — never the
/// model's, which cannot see whether a message addressed it.
pub const MISS_SENTINEL: &str = "[[miss]]";

/// Whether a finalized answer is a recognized abstention: the raw content,
/// trimmed, equals the sentinel exactly.
#[must_use]
pub fn is_abstention(content: &str) -> bool {
    content.trim() == ABSTENTION_SENTINEL
}

/// Whether a finalized answer is a recognized miss: the raw content,
/// trimmed, equals the miss sentinel exactly — judged like the abstention,
/// before any disclosure prepend.
#[must_use]
pub fn is_miss(content: &str) -> bool {
    content.trim() == MISS_SENTINEL
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

    /// The miss recognition holds the same boundary (unit 16), and the two
    /// sentinels stay distinct: neither spelling is recognized as the
    /// other, so the mechanism can always tell a "nothing to add" from a
    /// "found nothing".
    #[test]
    fn the_miss_recognition_is_exact_and_the_sentinels_stay_distinct() {
        assert!(is_miss(MISS_SENTINEL));
        assert!(is_miss(&format!("  {MISS_SENTINEL}\n")));
        assert!(!is_miss(&format!("{MISS_SENTINEL} — but here is a guess.")));
        assert!(!is_miss("miss"));
        assert!(!is_miss(""));

        assert_ne!(ABSTENTION_SENTINEL, MISS_SENTINEL);
        assert!(!is_miss(ABSTENTION_SENTINEL));
        assert!(!is_abstention(MISS_SENTINEL));
    }
}
