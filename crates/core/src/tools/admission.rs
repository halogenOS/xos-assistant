//! The admission wrapper: one check, at the top of every tool's execute.
//!
//! The framework's own admission chain offers no consumer seam with ledger
//! access — its gate hook receives the input string alone — so the assistant
//! enforces its rule where a consumer can: a wrapper around every registered
//! handler whose execute first reads the palette block through the tool
//! context's ledger access, and declines before the tool body runs.
//! "Declined, never executed" means the tool's body; the wrapper itself is
//! technically entered.
//!
//! A decline is returned as the recorded tool error the model reads, worded
//! so the model does not retry: the palette gates admission, not exposure,
//! and the model may be offered a tool this wrapper will decline.
//!
//! The palette is this wrapper's whole check on purpose. Authority
//! enforcement is structural, not a ledger reading: [`ToolSet::admit`]
//! refuses any tool whose required authority is above member (decision
//! 0043, closed 2026-08-22), so every tool this wrapper can ever hold is
//! admissible to any sender the palette admits, and no stored shape has to
//! answer whose authority summoned the turn. The mechanism that lifts that
//! floor is the framework's dispatch anchor — the turn's summoning frontier
//! recorded onto the tool call at insert — tracked on the framework
//! improvements list.
//!
//! [`ToolSet::admit`]: crate::tools::ToolSet::admit

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::reactivity::ReadSignal;
use agent_ledger::{
    AgencyCtx, CoreEvent, FromBlock, GateDecision, ToolContext, ToolHandler, ToolOutcome,
};

use crate::kind::AssistantKind;

/// The retry-teaching close of every decline: the model may be offered a
/// tool the palette declines, so the wording itself must stop the loop.
const NO_RETRY: &str = "Do not call this tool again this turn; answer from what you already have.";

/// One registered tool behind the admission check. The wrapper implements
/// the framework's handler trait around the inner tool, so every handler the
/// assembly registers passes through exactly this one rule — one rule, one
/// place.
pub struct AdmittedTool {
    inner: Box<dyn ToolHandler<CoreEvent>>,
    /// The registered name, read once from the inner definition so the
    /// decline texts and the palette check speak the same string.
    name: String,
}

impl AdmittedTool {
    /// Wrap one handler.
    #[must_use]
    pub fn new(inner: impl ToolHandler<CoreEvent> + 'static) -> Self {
        let inner: Box<dyn ToolHandler<CoreEvent>> = Box::new(inner);
        let name = inner.definition().name;
        Self { inner, name }
    }

    /// The registered name, for the assembly building the palette list.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The admission read: the conversation's palette, failing closed. `Err`
    /// carries the decline the caller records as the tool error.
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
                return Err(format!(
                    "declined: the conversation's record could not be read, and admission \
                     fails closed. {NO_RETRY}"
                ));
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
            None => Err(format!(
                "declined: this conversation has no tool palette recorded, and a \
                 conversation without one admits no tools. {NO_RETRY}"
            )),
            Some(palette) if !palette.admits(&self.name) => Err(format!(
                "declined: the tool '{}' is not in this conversation's tool palette. \
                 {NO_RETRY}",
                self.name
            )),
            Some(_) => Ok(()),
        }
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
