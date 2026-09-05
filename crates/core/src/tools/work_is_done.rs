//! The work-is-done tool (unit 54, 2026-09-02): the turn acted, the
//! actions are the whole answer, and calling this ends it with no message
//! posted.
//!
//! A model bred to act keeps acting, and a turn that already did its work
//! is where that shows: a report is filed, a reaction is placed, a lookup
//! ran for the assistant's own orientation, and the model then writes prose
//! narrating what the group can already see. The framework's turn-ending
//! capability absorbs that the same way its sibling absorbs an unasked
//! turn: a model that must do SOMETHING has a something that costs a chat
//! message nothing.
//!
//! This is a safety net, not a taught behaviour. Nothing in the composed
//! teaching directs the model here — plain silence stays the taught
//! default — so what this tool is for lives in its model-facing
//! description and nowhere else. The description carries the whole
//! meaning: what I did is complete, and prose would only narrate it.
//!
//! No parameters, no reads, no writes past its own resolution. The tool's
//! identity IS the reason the turn ended, which is why no free-text
//! parameter carries one: a reason field would invite exactly the prose
//! this tool replaces. Its sibling
//! [`no_reply_needed`](crate::tools::no_reply_needed) names the other fact
//! — the turn was asked nothing at all. Two tools and not one, because the
//! ledger keeping WHICH of the two the model meant is the point of
//! recording a decision.
//!
//! It is not moderation and suppresses nobody: it ends the assistant's own
//! turn and reaches nothing else.
//!
//! The turn's end is the framework's, declared by
//! [`ToolHandler::ends_turn`]: the resolution row carries the stamp, no
//! continuation round is dispatched, and a sibling call of the same round
//! keeps its own debt. The stored [`CLOSE`] sentence is what the ledger
//! reads back, and the model reads it on the next turn's replay, so it
//! addresses the model.

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::json;

use crate::message::Authority;

/// The registered name the model calls the tool by. The name states the
/// fact the call records: the work of this turn is done.
pub const NAME: &str = "work_is_done";

/// The authority this tool requires — member: ending one's own turn asks
/// nothing of anyone, and the turns it ends are summoned by ordinary
/// members' messages. The admission check supplies no extra protection at
/// this bar; the tool sits under it because every tool does (stated, not
/// implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The stored close: the one-line result the resolution carries, so the
/// ledger reads why the turn ended and the model reads it back on the next
/// turn's replay. Byte-fixed here and asserted by test.
pub const CLOSE: &str = "Turn ended: the actions taken are the whole answer.";

/// The work-is-done tool. Constructed by the assembly, which admits it
/// unconditionally: a turn whose actions are the whole answer happens
/// wherever the assistant runs, and there is no configuration for it to be
/// absent under. It holds nothing — the whole tool is its name, its
/// description and the fact that resolving it ends the turn.
pub(crate) struct WorkIsDone;

impl WorkIsDone {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ToolHandler<CoreEvent> for WorkIsDone {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "End this turn without posting anything, because what you have \
                 already done is the whole answer. Call it when this turn's actions — a \
                 report filed, a reaction placed, a lookup you ran to orient yourself — \
                 leave a closing message nothing to add. Calling it says: what I did is \
                 complete, and prose would only narrate it. It takes no arguments and \
                 posts nothing."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

    /// The framework's turn-ending capability, declared here: a resolved
    /// call of this tool ends the turn, and the machinery reads the
    /// property off this handler at the resolution write.
    fn ends_turn(&self) -> bool {
        true
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async { ToolOutcome::Done(CLOSE.to_owned()) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;
    use crate::schema::store_config;

    /// The stored close, byte for byte where it is defined: the sentence
    /// the resolution carries and the model reads back.
    #[test]
    fn the_stored_close_is_the_units_sentence() {
        assert_eq!(CLOSE, "Turn ended: the actions taken are the whole answer.");
    }

    /// The authority is member, stated where it is declared: ending one's
    /// own turn is not a privileged act.
    #[test]
    fn the_required_authority_is_member() {
        assert_eq!(REQUIRED_AUTHORITY, Authority::Member);
    }

    /// The definition: the registered name, the turn-ending property, no
    /// parameters at all, and a description carrying the meaning the
    /// teaching deliberately does not — what the call is for, and that it
    /// posts nothing.
    ///
    /// The pre-call posting sentence is asserted GONE (unit 55,
    /// 2026-09-02). It warned that prose written ahead of a call is posted
    /// as its own message, which was true while text was relayed; from unit
    /// 55 written text reaches nobody at all, so the warning would describe
    /// a mechanism that no longer exists.
    #[test]
    fn the_definition_declares_the_end_and_takes_no_parameters() {
        let tool = WorkIsDone::new();
        let definition = tool.definition();
        assert_eq!(definition.name, NAME);
        assert_eq!(definition.name, "work_is_done");
        assert!(tool.ends_turn(), "a resolved call ends the turn");
        assert_eq!(definition.parameters["properties"], json!({}));
        assert_eq!(definition.parameters["additionalProperties"], json!(false));
        assert!(
            definition.parameters.get("required").is_none(),
            "a tool with no parameters requires none"
        );
        for meaning in [
            "End this turn without posting anything",
            "what you have already done is the whole answer",
            "a report filed, a reaction placed, a lookup you ran to orient yourself",
            "what I did is complete, and prose would only narrate it",
            "It takes no arguments and posts nothing",
        ] {
            assert!(
                definition.description.contains(meaning),
                "the description carries: {meaning}"
            );
        }
        assert!(
            !definition
                .description
                .contains("posted to the group as its own message"),
            "the pre-call posting warning is gone: written text reaches nobody now"
        );
    }

    /// Every input answers the stored close: there is nothing to read, so
    /// an empty call, an object with strange fields and unparsable text
    /// all resolve the same way.
    #[tokio::test]
    async fn every_input_resolves_with_the_stored_close() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let agency: AgencyCtx<CoreEvent> = AgencyCtx {
            conversation_id: 9,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = WorkIsDone::new();
        for input in ["", "{}", r#"{"reason":"the report is filed"}"#, "not json"] {
            match tool
                .execute(
                    input,
                    ToolContext {
                        agency: &agency,
                        tool_call_id: "call-0",
                        block_id: 1,
                    },
                )
                .await
            {
                ToolOutcome::Done(result) => assert_eq!(result, CLOSE, "the input {input:?}"),
                ToolOutcome::Error(error) | ToolOutcome::Refused(error) => {
                    panic!("the input {input:?} resolves, it does not decline: {error}")
                }
                ToolOutcome::Pending => {
                    panic!("a turn-ending tool resolves at once; it never defers")
                }
            }
        }
    }
}
