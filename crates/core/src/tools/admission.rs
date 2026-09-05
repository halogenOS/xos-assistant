//! The assistant's own admission for one tool call: the anchor gate of
//! decision 0043, answered through the framework's admission hook.
//!
//! The framework asks every handler whether one call may go on, and it asks
//! over the ledger snapshot its admission pass already loaded — so the
//! reading below costs no store round-trip of its own and can never be
//! decided against a history the pass around it never saw. The answer is
//! admit or a refusal sentence; the framework records a refusal as the typed
//! fact a run of which ends the turn, and the tool's body never runs.
//!
//! One rule is asked here, and it is the anchor gate: the turn's provenance —
//! the minimum authority over its debt origins and co-summoners, read from
//! the loaded vector by the `provenance` module's `turn_reading` — must reach
//! the authority the tool requires. The reading is total and answers that
//! module's floor or higher, so a floor-level tool passes the same comparison
//! a privileged one faces, and a tool that answers the hook answers it here
//! or not at all — there is no second comparison beside this one.
//!
//! WHICH TOOLS a conversation has is not asked here. That is recorded in the
//! ledger as the framework's own tool choice, and the framework resolves a
//! call name against it before any handler is reached — so a tool this
//! conversation does not have is refused without this module hearing about it.
//!
//! WHAT HOLDS THIS TOGETHER, said plainly: the framework's hook has a
//! default, and the default ADMITS. A tool module that answers nothing
//! compiles, serves every authority, and leaves the authority constant it
//! declares a value nothing reads. So the check is opt-in per tool, one
//! line each module states for itself with the
//! `admits_at_required_authority` macro below, and what holds it for all of
//! them is a scan over the core's production source — the admission scan
//! in the cleanliness suite, which fails when a module implements the
//! framework's tool handler without that line. Nothing in the type system
//! says it; the test does, and this module is where the line it looks for
//! is written.
//!
//! The bar itself is stated once per tool, in the module that owns the
//! tool, and read once, here.

use agent_ledger::providers::BoxFuture;
use agent_ledger::{Admission, Block, CoreEvent, ToolContext};

use crate::message::Authority;
use crate::tools::provenance;

/// The retry-teaching close of a decline that holds for the whole turn: the
/// authority a turn reads is fixed for that turn, so a model that calls
/// again spends a round on the identical answer. Crate-visible because the
/// report tool's refusals close with the same teaching — one wording, one
/// spelling.
pub(crate) const NO_RETRY: &str =
    "Do not call this tool again this turn; answer from what you already have.";

/// The authority decline, recording the reading per decision 0043.
fn authority_decline(name: &str, required: Authority, reading: Authority) -> String {
    format!(
        "declined: the tool '{name}' needs {} authority and this turn's \
         provenance reads {} — the minimum over everyone who summoned it. \
         {NO_RETRY}",
        required.as_str(),
        reading.as_str()
    )
}

/// The answer every tool of this assistant gives the framework's admission
/// hook: admitted when the turn's provenance reaches `required`, declined
/// with the recorded sentence when it does not.
///
/// Public because [`ToolSet`](crate::tools::ToolSet) is: a tool an embedder
/// registers states its own bar through this one function, so a tool that
/// answers the hook has one wording to answer it with.
///
/// The decision is made before the future is handed back, over the snapshot
/// the admission pass loaded — there is nothing to await, and nothing here
/// reads the store.
#[must_use]
pub fn at_required_authority<'a>(
    name: &str,
    required: Authority,
    ctx: &ToolContext<'_, CoreEvent>,
    ledger: &[Block],
) -> BoxFuture<'a, Admission> {
    at_required_authority_and(name, required, ctx, ledger, |_| None)
}

/// The same answer with ONE further question asked behind it (unit 55,
/// 2026-09-02): the authority bar first, and then whatever bound the tool
/// itself reads off the very ledger the admission pass loaded.
///
/// The order is fixed, and it decides what the model is told. The authority
/// reading is the access model; a tool a turn may not reach at all is
/// declined for that reason and never for a spent allowance, which would
/// tell the model to wait for something that will never let it through.
///
/// `further` answers `Some(sentence)` to decline and `None` to admit. It is
/// a pure reading of the loaded vector, exactly as the authority reading is:
/// the hook is answered before the future is handed back, so nothing here
/// awaits and nothing reads the store. What it declines with is
/// [`Admission::Refuse`], which the framework records as a REFUSAL — a
/// standing no the model cannot re-plan around inside this turn, a run of
/// which ends the turn.
///
/// The sending tools' per-conversation caps are this parameter's first
/// consumer: the bound is shared across two tool names and counted over one
/// conversation's own blocks, which the framework's single-tier per-name
/// window cannot express.
#[must_use]
pub fn at_required_authority_and<'a>(
    name: &str,
    required: Authority,
    ctx: &ToolContext<'_, CoreEvent>,
    ledger: &[Block],
    further: impl FnOnce(&[Block]) -> Option<String>,
) -> BoxFuture<'a, Admission> {
    let reading = provenance::turn_reading(ledger, ctx.block_id);
    let answer = if reading < required {
        Admission::Refuse {
            reason: authority_decline(name, required, reading),
        }
    } else {
        match further(ledger) {
            Some(reason) => Admission::Refuse { reason },
            None => Admission::Admit,
        }
    };
    Box::pin(std::future::ready(answer))
}

/// One tool's whole answer to the framework's admission hook, written once
/// here and invoked inside each tool's `impl ToolHandler`.
///
/// The ten modules used to carry the identical ten-line body and the
/// identical doc comment; ten copies of one sentence are ten places for it
/// to stop being true. The two arguments are the invoking module's own
/// constants — the tool's name and the authority it requires — so the bar
/// stays declared where the tool is, and only the reading of it lives here.
///
/// A macro and not a wrapping handler type: the answer belongs to the
/// handler the tool already implements. A type that forwarded to it would
/// be the shape unit 52 deleted, silently dropping whatever trait method is
/// added after the forwarding was written.
///
/// The three-argument form (unit 55, 2026-09-02) names ONE further reading
/// asked behind the bar, through [`at_required_authority_and`]: a closure
/// over the loaded ledger answering the decline sentence or nothing. It
/// exists for a bound the framework's own windows cannot express, and it
/// stays the same one line each module states for itself, so the
/// cleanliness suite's admission scan reads both forms.
macro_rules! admits_at_required_authority {
    ($name:expr, $required:expr) => {
        /// The authority a call of this tool requires (decision 0043),
        /// answered through the framework's admission hook over the ledger
        /// snapshot the runner's admission pass already loaded.
        fn admit<'a>(
            &'a self,
            ctx: &'a ::agent_ledger::ToolContext<'a, ::agent_ledger::CoreEvent>,
            ledger: &'a [::agent_ledger::Block],
        ) -> ::agent_ledger::providers::BoxFuture<'a, ::agent_ledger::Admission> {
            $crate::tools::admission::at_required_authority($name, $required, ctx, ledger)
        }
    };
    ($name:expr, $required:expr, $further:expr) => {
        /// The authority a call of this tool requires (decision 0043) and
        /// the tool's own further bound behind it, both answered through
        /// the framework's admission hook over the ledger snapshot the
        /// runner's admission pass already loaded.
        fn admit<'a>(
            &'a self,
            ctx: &'a ::agent_ledger::ToolContext<'a, ::agent_ledger::CoreEvent>,
            ledger: &'a [::agent_ledger::Block],
        ) -> ::agent_ledger::providers::BoxFuture<'a, ::agent_ledger::Admission> {
            $crate::tools::admission::at_required_authority_and(
                $name, $required, ctx, ledger, $further,
            )
        }
    };
}

pub(crate) use admits_at_required_authority;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;

    /// The decline's wording, asserted whole where the string is built: it
    /// names the requirement, records the reading per decision 0043, and
    /// closes with the no-retry line, because the fact it states holds for
    /// the whole turn.
    #[test]
    fn the_authority_decline_names_the_bar_the_reading_and_the_no_retry_line() {
        let decline = authority_decline("probe", Authority::Admin, Authority::Member);
        assert_eq!(
            decline,
            "declined: the tool 'probe' needs admin authority and this turn's \
             provenance reads member — the minimum over everyone who summoned it. \
             Do not call this tool again this turn; answer from what you already have."
        );
        assert!(decline.ends_with(NO_RETRY));
    }

    /// The hook itself, over a ledger holding no call block at all: the
    /// reading folds to the floor, so an above-floor tool is declined with
    /// the recorded sentence and a floor-level one is admitted. Every
    /// unreadable shape folds the same way, which is what makes the anchor
    /// gate fail closed.
    #[tokio::test]
    async fn an_unreadable_turn_declines_above_the_floor_and_admits_at_it() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let agency = AgencyCtx {
            conversation_id: conversation,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let ctx = ToolContext {
            agency: &agency,
            tool_call_id: "call-0",
            block_id: 0,
        };

        match at_required_authority("probe", Authority::Admin, &ctx, &[]).await {
            Admission::Refuse { reason } => assert_eq!(
                reason,
                authority_decline("probe", Authority::Admin, provenance::FLOOR)
            ),
            Admission::Admit => panic!("an unreadable turn admits no admin tool"),
        }
        assert!(matches!(
            at_required_authority("probe", provenance::FLOOR, &ctx, &[]).await,
            Admission::Admit
        ));
    }
}
