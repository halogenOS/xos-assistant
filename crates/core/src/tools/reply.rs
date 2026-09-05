//! The threaded send: put one message in the group's chat as a reply to a
//! message the model named (unit 55, 2026-09-02).
//!
//! The twin of [`send`](crate::tools::send), differing in exactly one
//! place: the target. Every message the model reads carries an envelope
//! naming who wrote it, when it was sent and its id, and this tool is what
//! that id is for — the model decides on its own which message it answers,
//! and can answer several in one turn or none at all.
//!
//! The target is validated against the serving conversation's own ledger,
//! and an id it does not hold is refused rather than sent plain: a silently
//! dropped thread would hide an invented id from the model. The validation,
//! the caps, the filing and the sentences all live in
//! [`sending`]; what this module owns is its
//! registered name, what the model is told the tool is for, and its own
//! answer to the admission hook.

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{Block, CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::json;
use tokio::sync::RwLock;

use crate::composing::SendStops;
use crate::message::Authority;
use crate::tools::sending::{self, Sender};

/// The registered name the model calls the tool by. The name says what the
/// tool does: it replies to a message.
pub const NAME: &str = "reply_message";

/// The authority a call requires, stated in the module that owns the tool:
/// member, per [`sending::REQUIRED_AUTHORITY`].
pub const REQUIRED_AUTHORITY: Authority = sending::REQUIRED_AUTHORITY;

/// The threaded sending tool: member authority, two validated parameters,
/// every conversation. Constructed by the assembly unconditionally — the
/// erasure fence and the composing cue's stop channel injected here, at
/// registration, so the tool never reaches into the assembly.
pub(crate) struct ReplyMessage {
    sender: Sender,
}

impl ReplyMessage {
    pub(crate) fn new(fence: Arc<RwLock<()>>, stops: SendStops) -> Self {
        Self {
            sender: Sender::new(fence, stops),
        }
    }

    /// The caps this tool's admission asks behind the authority bar, read
    /// once over the ledger the admission pass loaded. The reading itself
    /// is the pair's, so both tools decline on the same count.
    fn declined_by_the_caps(&self, conversation_id: i64, ledger: &[Block]) -> Option<String> {
        self.sender.declined_by_the_caps(conversation_id, ledger)
    }
}

impl ToolHandler<CoreEvent> for ReplyMessage {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "Send one message to this group chat as a reply to a message you \
                 name. Your written text is private and reaches nobody; this tool and \
                 send_message are how the group hears from you. Name the message you are \
                 answering by the msgid shown in its envelope; it can be any message this \
                 conversation holds, of any age, one of your own included. An id this \
                 conversation does not hold is declined rather than posted without the \
                 reply. The result carries the id your message was posted under."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    sending::PARAMETER_TEXT: {
                        "type": "string",
                        "description": "The message to post, exactly as the group should \
                             read it"
                    },
                    sending::PARAMETER_REPLY_TO: {
                        "type": "string",
                        "description": "The msgid of the message this one answers, exactly \
                             as its envelope shows it"
                    }
                },
                "required": [sending::PARAMETER_TEXT, sending::PARAMETER_REPLY_TO]
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(
        NAME,
        REQUIRED_AUTHORITY,
        declined_by_the_caps
    );

    /// The sends run IN ORDER (unit 55, 2026-09-02): the framework parks a
    /// ready call of this tool while an earlier in-order call of the same
    /// conversation is unresolved, so the messages reach the group in the
    /// order the model issued them and a pending send never has a sibling
    /// in flight. It is also what makes the caps' count exact — the ledger
    /// the admission pass loaded holds every earlier send — and what
    /// leaves this tool with no filing lock of its own.
    fn runs_in_order(&self) -> bool {
        true
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move { self.sender.answer(&ctx, sending::named_reply(input)).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The definition teaches the contract and the aiming rule: the written
    /// text is private, the id comes from the envelope, any message this
    /// conversation holds may be answered, and an unheld id is declined
    /// instead of quietly losing the thread.
    #[test]
    fn the_definition_teaches_the_target_and_takes_both_parameters() {
        let definition =
            ReplyMessage::new(Arc::new(RwLock::new(())), crate::composing::stops()).definition();
        assert_eq!(definition.name, NAME);
        assert_eq!(definition.name, "reply_message");
        for instruction in [
            "as a reply to a message you name",
            "Your written text is private and reaches nobody",
            "Name the message you are answering by the msgid shown in its envelope",
            "any message this conversation holds, of any age, one of your own included",
            "An id this conversation does not hold is declined rather than posted without \
             the reply",
            "The result carries the id your message was posted under",
        ] {
            assert!(
                definition.description.contains(instruction),
                "the description carries: {instruction}"
            );
        }
        assert_eq!(
            definition.parameters["required"]
                .as_array()
                .expect("the schema names its required list"),
            &[
                json!(sending::PARAMETER_TEXT),
                json!(sending::PARAMETER_REPLY_TO)
            ],
            "both parameters are required: a reply is words and a target"
        );
    }
}
