//! The plain send: put one message in the group's chat, threaded onto
//! nothing (unit 55, 2026-09-02).
//!
//! This is one of the two doors the model's words reach the group through,
//! and it is the ordinary one: a message addressed to the room. Its twin,
//! [`reply`](crate::tools::reply), is the same act aimed at one message.
//!
//! Everything behind the two — the parameters, the caps, the filing, the
//! sentences — lives in [`sending`], so the pair can
//! never drift into two behaviours. What this module owns is its registered
//! name, what the model is told the tool is for, and its own answer to the
//! admission hook.

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::json;
use tokio::sync::RwLock;

use crate::filing::FilingDoor;
use crate::message::Authority;
use crate::tools::sending::{self, Sender};

/// The registered name the model calls the tool by. The name says what the
/// tool does: it sends a message.
pub const NAME: &str = "send_message";

/// The authority a call requires, stated in the module that owns the tool:
/// member, per [`sending::REQUIRED_AUTHORITY`].
pub const REQUIRED_AUTHORITY: Authority = sending::REQUIRED_AUTHORITY;

/// The plain sending tool: member authority, one validated parameter, every
/// conversation. Constructed by the assembly unconditionally — sending
/// needs nothing but a chat — with the erasure fence and the shared filing
/// door injected here, at registration, so the tool never reaches into the
/// assembly.
pub(crate) struct SendMessage {
    sender: Sender,
}

impl SendMessage {
    pub(crate) fn new(fence: Arc<RwLock<()>>, door: FilingDoor) -> Self {
        Self {
            sender: Sender::new(fence, door),
        }
    }
}

impl ToolHandler<CoreEvent> for SendMessage {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "Send one message to this group chat. Your written text is private \
                 and reaches nobody; this tool is how the group hears from you. Call it once \
                 per message you want to post, and not at all when you have nothing to say. \
                 The result carries the id the message was posted under."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    sending::PARAMETER_TEXT: {
                        "type": "string",
                        "description": "The message to post, exactly as the group should \
                             read it"
                    }
                },
                "required": [sending::PARAMETER_TEXT]
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(
        NAME,
        REQUIRED_AUTHORITY,
        sending::cap_decline
    );

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            match sending::named_send(input) {
                Ok(named) => self.sender.file(&ctx, &named).await,
                Err(refusal) => ToolOutcome::Error(refusal.to_owned()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The definition teaches the contract the tool exists for: the written
    /// text is private, this is how the group hears from the assistant, one
    /// call per message, none when there is nothing to say — and the result
    /// carries the id, which is what makes a later reply aimable.
    #[test]
    fn the_definition_teaches_the_contract_and_takes_one_parameter() {
        let definition =
            SendMessage::new(Arc::new(RwLock::new(())), crate::filing::door()).definition();
        assert_eq!(definition.name, NAME);
        assert_eq!(definition.name, "send_message");
        for instruction in [
            "Send one message to this group chat",
            "Your written text is private and reaches nobody",
            "this tool is how the group hears from you",
            "Call it once per message you want to post",
            "not at all when you have nothing to say",
            "The result carries the id the message was posted under",
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
            &[json!(sending::PARAMETER_TEXT)],
            "a plain send is its text and nothing else"
        );
        assert!(
            definition.parameters["properties"]
                .get(sending::PARAMETER_REPLY_TO)
                .is_none(),
            "the plain send names no target: that is the reply tool's parameter"
        );
    }
}
