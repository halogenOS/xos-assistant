//! Telegram adapter for the halogenOS Group Assistant: translates between the Telegram
//! Bot API and the core's message model, in both directions.
//!
//! Invariant: an adapter contains no behavior. Decisions about what the assistant says or
//! does belong to the core; this crate only converts representations and moves messages.
//!
//! This crate is a skeleton: the Telegram client and the conversion code arrive with the
//! first adapter work, so there are no dependencies yet.

#[cfg(test)]
mod tests {
    #[test]
    fn test_harness_runs() {
        let sum: u32 = (1..=4).sum();
        assert_eq!(sum, 10);
    }
}
