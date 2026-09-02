//! The no-reply-needed tool (unit 54, 2026-09-02): the turn was asked
//! nothing, and calling this ends it with no message posted.
//!
//! A model bred to act keeps acting. When a turn has nothing to answer, the
//! taught default is to end it with no text at all — and the deployed
//! assistant instead wrote out its own silence, posting that a question was
//! aimed at someone else and it was staying out of it. The framework's
//! turn-ending capability exists to absorb that: a tool whose successful
//! resolution ends the turn, so a model that must do SOMETHING has a
//! something that costs a chat message nothing.
//!
//! This is a safety net, not a taught behaviour. Nothing in the composed
//! teaching directs the model here — plain silence stays the taught
//! default — so what this tool is for lives in its model-facing
//! description and nowhere else. The description carries the whole
//! meaning: nobody here is waiting on me.
//!
//! It names two concrete cases and no third, and a reaction is not among
//! them. Decision 0197 makes the reaction the terminal-message action — a
//! response to the assistant that needs no further response — so chatter
//! draws no reaction at all, and a turn that DID place one is
//! [`work_is_done`](crate::tools::work_is_done)'s fact, whose description
//! names it.
//!
//! No parameters, no reads, no writes past its own resolution. The tool's
//! identity IS the reason the turn ended, which is why no free-text
//! parameter carries one: a reason field would invite exactly the prose
//! this tool replaces. Its sibling [`work_is_done`](crate::tools::work_is_done)
//! names the other fact — the turn acted, and the actions are the whole
//! answer.
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
/// fact the call records: no reply was needed here.
pub const NAME: &str = "no_reply_needed";

/// The authority this tool requires — member: ending one's own turn asks
/// nothing of anyone, and the turns it ends are summoned by ordinary
/// members' messages. The admission check supplies no extra protection at
/// this bar; the tool sits under it because every tool does (stated, not
/// implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The stored close: the one-line result the resolution carries, so the
/// ledger reads why the turn ended and the model reads it back on the next
/// turn's replay. Byte-fixed here and asserted by test.
pub const CLOSE: &str = "Turn ended: no reply was needed.";

/// The no-reply-needed tool. Constructed by the assembly, which admits it
/// unconditionally: a turn that was asked nothing happens wherever the
/// assistant runs, and there is no configuration for it to be absent
/// under. It holds nothing — the whole tool is its name, its description
/// and the fact that resolving it ends the turn.
pub(crate) struct NoReplyNeeded;

impl NoReplyNeeded {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ToolHandler<CoreEvent> for NoReplyNeeded {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "End this turn without posting anything, because nothing here \
                 was asked of you. Call it when the messages you are reading are other \
                 people's conversation, or when a question names someone who is not you. \
                 Calling it says: nobody here is waiting on me. It takes no arguments and \
                 posts nothing. Call it on its own, with nothing written ahead of the \
                 call: whatever you write before a tool call is posted to the group as \
                 its own message."
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
        assert_eq!(CLOSE, "Turn ended: no reply was needed.");
    }

    /// The authority is member, stated where it is declared: ending one's
    /// own turn is not a privileged act.
    #[test]
    fn the_required_authority_is_member() {
        assert_eq!(REQUIRED_AUTHORITY, Authority::Member);
    }

    /// The definition: the registered name, the turn-ending property, no
    /// parameters at all, and a description carrying the meaning the
    /// teaching deliberately does not — what the call is for, that it
    /// posts nothing, and that it is made bare.
    ///
    /// The reaction is asserted ABSENT from it. Decision 0197 gave the
    /// reaction the terminal message, so no chatter case is left for a
    /// reaction to close, and the turn that placed one belongs to the
    /// sibling tool: a description licensing it here would send the model
    /// to two tools for one fact.
    #[test]
    fn the_definition_declares_the_end_and_takes_no_parameters() {
        let tool = NoReplyNeeded::new();
        let definition = tool.definition();
        assert_eq!(definition.name, NAME);
        assert_eq!(definition.name, "no_reply_needed");
        assert!(tool.ends_turn(), "a resolved call ends the turn");
        assert_eq!(definition.parameters["properties"], json!({}));
        assert_eq!(definition.parameters["additionalProperties"], json!(false));
        assert!(
            definition.parameters.get("required").is_none(),
            "a tool with no parameters requires none"
        );
        for meaning in [
            "End this turn without posting anything",
            "nothing here was asked of you",
            "a question names someone who is not you",
            "nobody here is waiting on me",
            "It takes no arguments and posts nothing",
            "Call it on its own, with nothing written ahead of the call",
        ] {
            assert!(
                definition.description.contains(meaning),
                "the description carries: {meaning}"
            );
        }
        for absent in ["reaction", "chatter"] {
            assert!(
                !definition.description.contains(absent),
                "the description licenses no reaction case: the reaction closes a \
                 response to the assistant, and the turn that placed one is the \
                 sibling tool's fact: {absent}"
            );
        }
    }

    /// Every input answers the stored close: there is nothing to read, so
    /// an empty call, an object with strange fields and unparsable text
    /// all resolve the same way.
    #[tokio::test]
    async fn every_input_resolves_with_the_stored_close() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let agency: AgencyCtx<CoreEvent> = AgencyCtx {
            conversation_id: 7,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = NoReplyNeeded::new();
        for input in ["", "{}", r#"{"reason":"nobody asked"}"#, "not json"] {
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
