//! The admission wrapper: the palette check, then the anchor gate, at the
//! top of every tool's execute.
//!
//! The framework's own admission chain offers no consumer seam with ledger
//! access — its gate hook receives the input string alone — so the assistant
//! enforces its rule where a consumer can: a wrapper around every registered
//! handler whose execute first checks admission through the tool context's
//! ledger access, and declines before the tool body runs. "Declined, never
//! executed" means the tool's body; the wrapper itself is technically
//! entered.
//!
//! Two checks, in order, over one ledger load. The palette first: a
//! conversation admits only the tools its recorded palette names, failing
//! closed. Then the anchor gate of decision 0043: the turn's provenance —
//! the minimum authority over its debt origins and co-summoners, read
//! from the loaded vector by [`provenance::turn_reading`] — must reach
//! the tool's required authority. One admission path for every tool: the
//! reading is total and answers [`provenance::FLOOR`] or higher, so a
//! floor-level tool passes the same comparison a privileged one faces,
//! and no second path exists to drift from it.
//!
//! A decline is returned as the recorded tool error the model reads, and
//! its wording is split on what the decline states. The palette refusal
//! and the authority decline state facts that hold for the whole turn, so
//! both close with the no-retry line — the palette gates admission, not
//! exposure, and the model may be offered a tool this wrapper will
//! decline. A ledger load failure states a transient fact: the decline
//! says the turn's provenance could not be verified right now, and teaches
//! nothing about retrying, because a later turn may verify fine. The
//! authority decline records the reading in its text, per 0043.
//!
//! [`provenance::turn_reading`]: crate::tools::provenance::turn_reading
//! [`provenance::FLOOR`]: crate::tools::provenance::FLOOR

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::reactivity::ReadSignal;
use agent_ledger::{
    AgencyCtx, CoreEvent, FromBlock, GateDecision, ToolContext, ToolHandler, ToolOutcome,
};

use crate::kind::AssistantKind;
use crate::message::Authority;
use crate::tools::provenance;

/// The retry-teaching close of the declines that hold for the whole turn:
/// the model may be offered a tool the palette declines, so the wording
/// itself must stop the loop. Crate-visible because the report tool's
/// refusals close with the same teaching — one wording, one spelling.
pub(crate) const NO_RETRY: &str =
    "Do not call this tool again this turn; answer from what you already have.";

/// The transient decline: the ledger did not read, so neither check could
/// run. No no-retry line — the fact may not hold beyond this failure.
fn transient_decline() -> String {
    "declined: the turn's provenance could not be verified right now, and admission \
     fails closed."
        .to_owned()
}

/// The decline for a conversation carrying no palette block at all.
fn no_palette_decline() -> String {
    format!(
        "declined: this conversation has no tool palette recorded, and a \
         conversation without one admits no tools. {NO_RETRY}"
    )
}

/// The decline for a tool the conversation's palette does not name.
fn outside_palette_decline(name: &str) -> String {
    format!("declined: the tool '{name}' is not in this conversation's tool palette. {NO_RETRY}")
}

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

/// One registered tool behind the admission check. The wrapper implements
/// the framework's handler trait around the inner tool, so every handler the
/// assembly registers passes through exactly this one rule — one rule, one
/// place.
pub struct AdmittedTool {
    inner: Box<dyn ToolHandler<CoreEvent>>,
    /// The registered name, read once from the inner definition so the
    /// decline texts and the palette check speak the same string.
    name: String,
    /// The authority the turn's provenance must reach for this tool — the
    /// anchor gate's bar, compared against the reading at every call.
    required: Authority,
}

impl AdmittedTool {
    /// Wrap one handler at its required authority.
    #[must_use]
    pub fn new(required: Authority, inner: impl ToolHandler<CoreEvent> + 'static) -> Self {
        let inner: Box<dyn ToolHandler<CoreEvent>> = Box::new(inner);
        let name = inner.definition().name;
        Self {
            inner,
            name,
            required,
        }
    }

    /// The registered name, for the assembly building the palette list.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The admission read: the conversation's palette, then the anchor
    /// gate's provenance reading, both failing closed over one ledger
    /// load. `Err` carries the decline the caller records as the tool
    /// error.
    async fn admit(&self, ctx: &ToolContext<'_, CoreEvent>) -> Result<(), String> {
        let ledger = match ctx
            .agency
            .store
            .list_blocks(ctx.agency.conversation_id)
            .await
        {
            Ok(ledger) => ledger,
            Err(error) => {
                tracing::warn!(
                    conversation_id = ctx.agency.conversation_id,
                    %error,
                    "tool admission: reading the ledger failed; declining"
                );
                return Err(transient_decline());
            }
        };
        // The newest palette block speaks. Today a conversation carries at
        // most one, written at creation; reading the newest keeps the rule
        // stable if a later unit ever supersedes a palette by appending.
        let palette =
            ledger
                .iter()
                .rev()
                .find_map(|block| match AssistantKind::from_block(block) {
                    AssistantKind::ToolPalette(palette) => Some(palette),
                    _ => None,
                });
        match palette {
            None => return Err(no_palette_decline()),
            Some(palette) if !palette.admits(&self.name) => {
                return Err(outside_palette_decline(&self.name));
            }
            Some(_) => {}
        }
        // The anchor gate (decision 0043): the turn's provenance must reach
        // the tool's required authority.
        let reading = provenance::turn_reading(&ledger, ctx.block_id);
        if reading < self.required {
            return Err(authority_decline(&self.name, self.required, reading));
        }
        Ok(())
    }
}

impl ToolHandler<CoreEvent> for AdmittedTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    fn gated(&self) -> bool {
        self.inner.gated()
    }

    fn interactive(&self) -> bool {
        self.inner.interactive()
    }

    fn gate<'a>(&'a self, input: &'a str) -> BoxFuture<'a, GateDecision> {
        self.inner.gate(input)
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            match self.admit(&ctx).await {
                Ok(()) => self.inner.execute(input, ctx).await,
                // The recorded tool error, before the body ran and before
                // any network was touched — the runner records it and the
                // model reads it.
                Err(decline) => ToolOutcome::Error(decline),
            }
        })
    }

    fn spawn_reactor(
        &self,
        ctx: AgencyCtx<CoreEvent>,
        latched: ReadSignal<bool>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        self.inner.spawn_reactor(ctx, latched)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use agent_ledger::{EventBus, Store};

    use super::*;

    /// The wording split, pinned where the strings are built: the palette
    /// refusals and the authority decline close with the no-retry line —
    /// their facts hold for the whole turn — while the transient decline
    /// names "right now" and carries no retry teaching at all.
    #[test]
    fn the_declines_split_on_the_no_retry_line() {
        let transient = transient_decline();
        assert!(
            transient.contains("could not be verified right now"),
            "the transient decline names the moment: {transient}"
        );
        assert!(
            !transient.contains(NO_RETRY),
            "a transient fact teaches no never-again: {transient}"
        );
        for durable in [
            no_palette_decline(),
            outside_palette_decline("probe"),
            authority_decline("probe", Authority::Admin, Authority::Member),
        ] {
            assert!(
                durable.ends_with(NO_RETRY),
                "a whole-turn fact closes with the no-retry line: {durable}"
            );
        }
        let authority = authority_decline("probe", Authority::Admin, Authority::Member);
        assert!(
            authority.contains("needs admin") && authority.contains("reads member"),
            "the authority decline records the requirement and the reading: {authority}"
        );
    }

    /// A handler that records whether its body ever ran — the probe behind
    /// the fail-closed pin below.
    struct Probe(Arc<AtomicBool>);

    impl ToolHandler<CoreEvent> for Probe {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "probe".into(),
                description: "a probe".into(),
                parameters: serde_json::json!({ "type": "object" }),
            }
        }

        fn execute<'a>(
            &'a self,
            _input: &'a str,
            _ctx: ToolContext<'a, CoreEvent>,
        ) -> BoxFuture<'a, ToolOutcome> {
            self.0.store(true, Ordering::SeqCst);
            Box::pin(async { ToolOutcome::Done("ran".into()) })
        }
    }

    /// The fail-closed rule, behaviorally: a conversation carrying no
    /// palette block — one the palette reconciliation never touched —
    /// draws the recorded no-palette decline, and the wrapped body
    /// provably never runs. The assembly's reconciliation writes a palette
    /// before any turn, so this arm is unreachable through the full
    /// assembly; the wrapper still refuses on its own, and this pin keeps
    /// that refusal a behavior instead of a wording.
    #[tokio::test]
    async fn a_conversation_without_a_palette_declines_before_the_body_runs() {
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
        let ran = Arc::new(AtomicBool::new(false));
        let tool = AdmittedTool::new(Authority::Member, Probe(Arc::clone(&ran)));

        let outcome = tool
            .execute(
                "{}",
                ToolContext {
                    agency: &agency,
                    tool_call_id: "call-0",
                    block_id: 0,
                },
            )
            .await;

        match outcome {
            ToolOutcome::Error(decline) => assert_eq!(
                decline,
                no_palette_decline(),
                "the no-palette arm speaks its own recorded decline"
            ),
            ToolOutcome::Done(_) | ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                panic!("a conversation without a palette admits no tool")
            }
        }
        assert!(
            !ran.load(Ordering::SeqCst),
            "declined means the wrapped body never ran"
        );
    }
}
