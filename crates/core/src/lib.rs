//! Platform-neutral core of the halogenOS Group Assistant: conversation handling,
//! knowledge lookup, command semantics, and rate and abuse protection all live here.
//!
//! Invariant: the core contains no platform vocabulary. Platform names, API types,
//! identifiers and wire formats stay inside the adapters; the core sees only its own
//! message model.
//!
//! This crate is a skeleton: it carries no behavior yet.

#[cfg(test)]
mod tests {
    #[test]
    fn test_harness_runs() {
        let sum: u32 = (1..=4).sum();
        assert_eq!(sum, 10);
    }
}
