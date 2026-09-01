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
//! a privileged one faces and no second path exists to drift from it.
//!
//! WHICH TOOLS a conversation has is not asked here. That is recorded in the
//! ledger as the framework's own tool choice, and the framework resolves a
//! call name against it before any handler is reached — so a tool this
//! conversation does not have is refused without this module hearing about it.
//!
//! Every tool of this assistant answers the framework's hook with
//! [`at_required_authority`], naming the authority its own module declares.
//! The bar is stated once per tool, in the module that owns the tool, and
//! read once, here.

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
/// registers states its own bar through this one function, so there is no
/// second way to answer the hook and no second wording to drift from.
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
    let reading = provenance::turn_reading(ledger, ctx.block_id);
    let answer = if reading < required {
        Admission::Refuse {
            reason: authority_decline(name, required, reading),
        }
    } else {
        Admission::Admit
    };
    Box::pin(std::future::ready(answer))
}

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
