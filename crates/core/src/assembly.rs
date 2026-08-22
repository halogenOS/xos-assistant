//! The core assembly: the runtime wiring, the ingestion entry point, the
//! outbound subscription and the erasure operation, in one place.
//!
//! The assembly is constructed with its runtime wiring — the store opened on
//! the assistant's configuration, the event bus, the provider and tool
//! registries — the model binding under which first-message conversation
//! creation happens, and the system prompt every new conversation is created
//! with. The entry point draws the binding and the prompt from here, never
//! from a message. The constructor's caller owns the store, the bus and the
//! registries it passed in; the adapter edges below are the only surface an
//! adapter touches.

use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use agent_ledger::store::ProviderInstance;
use agent_ledger::{
    CoreEvent, EventBus, ProviderRegistry, Role, RuntimeContext, Store, ToolRegistry, spawn_reactor,
};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::erasure::{self, ErasureOutcome};
use crate::error::CoreError;
use crate::kind::{self, AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage};
use crate::message::{ChannelKey, ChannelKind, InboundMessage, OutboundReply};
use crate::streams::StreamObserver;
use crate::{identity, mapping, outbound, streams};

/// The model every new conversation is created under: one provider instance
/// and one model, named by the assembly and never by a message.
#[derive(Debug, Clone)]
pub struct ModelBinding {
    /// The provider instance's id, as registered in the store.
    pub provider_instance: String,
    /// The name a human reads for that instance.
    pub provider_display_name: String,
    /// The provider module's type id — what resolves the instance to the
    /// registered module.
    pub vendor: String,
    /// The provider's own identifier for the model.
    pub model: String,
    /// The name a human reads for the model.
    pub model_display_name: String,
}

/// The answering budgets the entry point enforces at the write — the flood
/// protection of decision 0030. Protection limits answering, never
/// recording: an over-limit addressed message is still recorded, with the
/// refusing budget named in its limited fact, and only the debt the message
/// itself would open is refused — a propagated debt passes unchanged.
///
/// A disabled budget (`None`) admits every debt. The defaults are the
/// stated product knobs of decision 0035; the embedder's configuration file
/// overrides them.
#[derive(Debug, Clone)]
pub struct ProtectionConfig {
    /// The per-sender budget, counted globally across conversations — spend
    /// is global, so heavy direct-chat use and group use share one budget.
    pub principal: Option<Budget>,
    /// The per-conversation budget, counted across that conversation's
    /// senders.
    pub channel: Option<Budget>,
}

impl ProtectionConfig {
    /// The default principal budget: answers per window.
    pub const DEFAULT_PRINCIPAL_ANSWERS: u32 = 6;
    /// The default principal window, in seconds.
    pub const DEFAULT_PRINCIPAL_WINDOW_SECONDS: u64 = 600;
    /// The default channel budget: answers per window.
    pub const DEFAULT_CHANNEL_ANSWERS: u32 = 20;
    /// The default channel window, in seconds.
    pub const DEFAULT_CHANNEL_WINDOW_SECONDS: u64 = 600;
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            principal: Some(Budget {
                answers: NonZeroU32::new(Self::DEFAULT_PRINCIPAL_ANSWERS)
                    .expect("the stated default is nonzero"),
                window_seconds: NonZeroU64::new(Self::DEFAULT_PRINCIPAL_WINDOW_SECONDS)
                    .expect("the stated default is nonzero"),
            }),
            channel: Some(Budget {
                answers: NonZeroU32::new(Self::DEFAULT_CHANNEL_ANSWERS)
                    .expect("the stated default is nonzero"),
                window_seconds: NonZeroU64::new(Self::DEFAULT_CHANNEL_WINDOW_SECONDS)
                    .expect("the stated default is nonzero"),
            }),
        }
    }
}

/// One budget: how many debts may open per window. The answer count is
/// nonzero by type — an assistant configured to answer no one is a
/// misconfiguration the embedder's parse refuses, and this type makes the
/// refusal structural. Disabling a budget is the enclosing `Option`, never
/// a zero.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// How many debts may open inside one window.
    pub answers: NonZeroU32,
    /// How far back the count looks, by receipt time, in whole seconds —
    /// the window's one unit end to end, decided with this unit: the
    /// configuration file speaks seconds, this field stores them, and the
    /// count's SQL cutoff subtracts exactly them, so a sub-second window
    /// is unrepresentable and nothing is silently truncated. Nonzero by
    /// type: disabling a budget is the enclosing `Option`, never a zero
    /// here.
    pub window_seconds: NonZeroU64,
}

/// What one accepted ingestion reports back: the ids the core resolved on
/// the way in. The principal id is the handle a later
/// [`Assistant::erase_principal`] call needs, so an operator surface built
/// on the adapter has a lawful path to it without reading the core's tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestReceipt {
    /// The principal the sender resolved or created.
    pub principal_id: i64,
    /// The conversation the message was recorded in.
    pub conversation_id: i64,
}

/// The running core: the framework runtime spawned over the assistant's
/// composed kind, plus the assistant's two edges and the erasure operation.
pub struct Assistant {
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    binding: ModelBinding,
    /// The system prompt recorded into every conversation at its creation,
    /// through the framework's system-prompt kind. The framework records a
    /// conversation's prompt exactly once, so an edited prompt reaches new
    /// conversations only.
    system_prompt: String,
    /// The streaming state the erasure ordering reads; the observation's
    /// contract and its lossy edges are stated on the streams module.
    streams: Arc<StreamObserver>,
    /// The answering budgets the stamp consults for addressed messages.
    /// Read-only after start; the limits themselves are derived from the
    /// ledger at every write, never tallied here.
    protection: ProtectionConfig,
    /// Serializes the answer-due stamp against other ingestions: the tail
    /// read and the append it feeds are a read-then-write, and two
    /// concurrent ingestions into one conversation could both observe the
    /// pre-append tail — the later, unaddressed write would then be stamped
    /// false, cancelling exactly the owed answer decision 0021 exists to
    /// protect. Ingestion against ingestion is all this lock orders: the
    /// runtime commits its answer blocks outside it, so a tail read can see
    /// an answer-due tail whose answer commits a moment later, and the
    /// stamp then summons one extra turn over an already-answered tail —
    /// never a lost answer. One lock across all conversations, because
    /// ingestion runs at chat scale and a per-conversation lock map would
    /// buy contention nobody has.
    stamp_lock: Mutex<()>,
    /// Orders erasure against ingestion. An erasure reads, nulls and deletes
    /// in several store operations, and an ingestion interleaved between
    /// them could record a new message or map a new direct channel for the
    /// person being erased, leaving personal data behind after the erasure
    /// returns. Ingestions hold this shared, an erasure holds it
    /// exclusively, so the erasure's steps see no half-recorded message.
    erasure_fence: RwLock<()>,
}

impl Assistant {
    /// Assemble and start the core: check the wiring, record the binding's
    /// provider instance in the store, and spawn the runtime over the given
    /// registries.
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingContentTable`] if the store's effective
    /// content-table list lacks the message kind's table — the store was
    /// opened without [`crate::schema::store_config`], and every append
    /// would fail later and further from the cause.
    /// [`CoreError::UnknownVendor`] if no registered provider module answers
    /// to the binding's vendor — the vendor is what resolves a conversation
    /// to its module, so a mismatch accepted here would silently strand
    /// every conversation. [`CoreError::Store`] if recording the provider
    /// instance fails.
    pub async fn start(
        store: Store,
        bus: Arc<EventBus<CoreEvent>>,
        providers: Arc<ProviderRegistry>,
        tools: Arc<ToolRegistry<CoreEvent>>,
        binding: ModelBinding,
        system_prompt: String,
        protection: ProtectionConfig,
    ) -> Result<Self, CoreError> {
        if !store.content_tables().contains(&CHAT_MESSAGE_TABLE) {
            return Err(CoreError::MissingContentTable {
                table: CHAT_MESSAGE_TABLE,
            });
        }
        if providers.get(&binding.vendor).is_none() {
            return Err(CoreError::UnknownVendor {
                vendor: binding.vendor,
            });
        }
        let ctx: RuntimeContext<AssistantKind, CoreEvent> =
            RuntimeContext::new(store, bus, providers, tools);
        ctx.store()
            .save_provider_instance(ProviderInstance {
                id: binding.provider_instance.clone(),
                provider_type: binding.vendor.clone(),
                name: binding.provider_display_name.clone(),
            })
            .await?;
        let streams = streams::spawn_observer(ctx.bus());
        spawn_reactor(ctx.clone());
        Ok(Self {
            ctx,
            binding,
            system_prompt,
            streams,
            protection,
            stamp_lock: Mutex::new(()),
            erasure_fence: RwLock::new(()),
        })
    }

    /// The ingestion edge: record one inbound message and return the ids it
    /// resolved to.
    ///
    /// Resolves or creates the sender's principal, maps the channel —
    /// creating the conversation under the assembly's binding, with the
    /// assembly's system prompt recorded first, on first message — stamps
    /// the message, and appends the message block through the framework's
    /// consumer write path. The stamp order is fixed: addressing first;
    /// budgets consulted only for addressed messages, principal before
    /// channel, the first refusing budget naming the limited fact; then
    /// answer-due by the composition rule — due when the message's own debt
    /// was taken (addressed, not limited) or when the tail owes, so a
    /// refused sender's message can be denied its own answer but can never
    /// cancel a debt it merely propagates; then the debt authority by the
    /// minimum rule. Nothing here ever drops a message: protection limits
    /// answering, never recording.
    ///
    /// A message whose own debt was taken then emits the unlatch intent,
    /// always: a person addressing the assistant IS the deliberate
    /// re-engagement, the intent is idempotent, and the same emission
    /// releases a fresh conversation's boot latch and a stream error's
    /// re-latch alike. An unaddressed message never unlatches, and neither
    /// does a limited one — a refused debt is not re-engagement, so a
    /// limited flood cannot wake an error-latched conversation.
    ///
    /// # Errors
    ///
    /// [`CoreError::ChannelKindMismatch`] if the channel is already mapped
    /// under a different kind than the message claims.
    /// [`CoreError::ClaimLost`] if a first-message claim lost its mapping
    /// row mid-claim. [`CoreError::Store`] if identity resolution, mapping
    /// or the append fails.
    pub async fn ingest(&self, message: InboundMessage) -> Result<IngestReceipt, CoreError> {
        let _no_erasure_mid_message = self.erasure_fence.read().await;
        let tx = self.ctx.store().tx();
        let principal_id = identity::resolve_principal(
            &tx,
            message.channel.adapter.clone(),
            message.sender.clone(),
        )
        .await?;

        let conversation_id = match mapping::find(&tx, &message.channel).await? {
            Some((existing, stored_kind)) => {
                if stored_kind != message.channel_kind {
                    return Err(CoreError::ChannelKindMismatch {
                        stored: stored_kind,
                        claimed: message.channel_kind,
                    });
                }
                existing
            }
            None => {
                self.map_new_channel(&message.channel, message.channel_kind)
                    .await?
            }
        };

        // Held from the tail read and the budget counts through the append:
        // the stamp is decided against the tail this write is appended
        // behind, and the counts must see every earlier taken debt — so no
        // concurrent ingestion may slide a block in between, and two racing
        // messages cannot both take the last budget slot. The lock's
        // contract is on its field.
        let _one_stamp_at_a_time = self.stamp_lock.lock().await;
        let owing_tail = self.owing_tail_debt(conversation_id).await?;
        let limited = if message.addressed {
            self.refusing_budget(principal_id, conversation_id).await?
        } else {
            None
        };
        // The composition rule and the minimum rule live on the kind, as
        // one pure composition beside the stamp's readers.
        let stamp = kind::Stamp::compose(message.addressed, message.authority, limited, owing_tail);
        let fields = ChatMessage::stored_fields(
            &message.text,
            principal_id,
            message.authority,
            message.origin.as_deref(),
            &message.timestamp.to_rfc3339(),
            stamp,
        );
        self.ctx
            .store()
            .append_consumer_block(
                conversation_id,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                fields,
                None,
            )
            .await?;
        if stamp.own_debt_taken() {
            self.ctx
                .bus()
                .emit(CoreEvent::UnlatchRequested { conversation_id });
        }
        Ok(IngestReceipt {
            principal_id,
            conversation_id,
        })
    }

    /// The outbound edge for one adapter: a subscription yielding the
    /// assistant's replies on that adapter's channels, each bound to its
    /// channel key. Each adapter takes one edge under its own name and never
    /// sees another adapter's replies. Answers already stored when the
    /// subscription is taken are history and stay off it; every answer
    /// stored afterwards is delivered at least once, re-read from the ledger
    /// — the outbound module's doc states the exact delivery contract,
    /// including the failure notice's at-most-once nature.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if reading the stored state that marks the
    /// history boundary fails.
    pub async fn replies(
        &self,
        adapter: &str,
    ) -> Result<mpsc::UnboundedReceiver<OutboundReply>, CoreError> {
        Ok(outbound::spawn_edge(self.ctx.clone(), adapter.to_owned()).await?)
    }

    /// Erase one principal, in one call, per decision 0012: the personal
    /// columns of the principal's messages — text, origin reference and
    /// platform send time — are nulled in every conversation (the block
    /// headers keep their positions and references, and an erased message
    /// projects none of its prose to the model), the principal's direct
    /// conversations are removed entirely with their channel mappings, and
    /// the identity rows are deleted. Reports [`ErasureOutcome::NotFound`]
    /// — touching nothing — when no identity row matches, a completed
    /// earlier erasure included.
    ///
    /// A direct conversation showing an open stream — observed on the bus,
    /// or holding a stored streaming tail a gone runtime left behind — is
    /// settled first, per the streams module's protocol: the interrupt goes
    /// out and a bounded stored-state re-read confirms the interrupt's
    /// ledger writes have finished before anything is deleted, so the
    /// stream's appends cannot race the deletion. Past the bound the erasure
    /// fails loudly with [`CoreError::ErasureUnsettled`], deleting nothing.
    /// An idle principal pays no wait.
    ///
    /// Not covered, recorded OPEN in decision 0012: a group conversation's
    /// derived title may have been shaped by since-erased prose and is not
    /// regenerated here.
    ///
    /// The call holds the erasure fence exclusively, so no ingestion can
    /// record a message or a mapping for the person between the steps.
    ///
    /// # Errors
    ///
    /// [`CoreError::ErasureUnsettled`] if an open stream did not settle
    /// before the bound; [`CoreError::Store`] if a read, a write or a
    /// deletion fails.
    pub async fn erase_principal(&self, principal_id: i64) -> Result<ErasureOutcome, CoreError> {
        let _no_ingestion_mid_erasure = self.erasure_fence.write().await;
        let store = self.ctx.store();
        let Some(plan) = erasure::plan(store, principal_id).await? else {
            return Ok(ErasureOutcome::NotFound);
        };
        // The plan's conversations are exactly the deletion set, so settling
        // them is settling everything the execute step will remove.
        for &conversation_id in plan.direct_conversations() {
            streams::settle_for_deletion(store, self.ctx.bus(), &self.streams, conversation_id)
                .await?;
        }
        let outcome = erasure::execute(store, plan).await?;
        if let ErasureOutcome::Erased {
            deleted_conversations,
        } = &outcome
        {
            // The store reissues conversation ids: a deleted conversation's
            // stream observation must not survive to shadow the id's next
            // holder.
            for &deleted in deleted_conversations {
                self.streams.forget(deleted);
            }
        }
        Ok(outcome)
    }

    /// First message on a channel: create the conversation under the
    /// assembly's binding, record the system prompt as its first block, and
    /// claim the mapping.
    ///
    /// Two ingestions can race here; the mapping's claim decides, and the
    /// loser's conversation is deleted — its prompt block with it — before
    /// anything referenced it. Recording the prompt before the claim is what
    /// makes it the winner's first block: the losing racer's message arrives
    /// in the winning conversation only after the winner's prompt is in.
    async fn map_new_channel(
        &self,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<i64, CoreError> {
        let store = self.ctx.store();
        let created = store
            .create_conversation(
                self.binding.provider_instance.clone(),
                self.binding.model.clone(),
                self.binding.model_display_name.clone(),
                self.binding.vendor.clone(),
            )
            .await?;
        store
            .insert_system_prompt(created, self.system_prompt.clone())
            .await?;
        let winner = mapping::claim(&store.tx(), channel, kind, created).await?;
        if winner != created {
            store.delete_conversation(created).await?;
        }
        Ok(winner)
    }

    /// The conversation's owing tail, if any — the one-block read behind the
    /// write-time stamp, deciding through the kind's own
    /// [`ChatMessage::owes_answer`] so this read and the awaiting hook can
    /// never disagree about one stamp: an erased tail, whose debt the hook
    /// cancels, propagates nothing here either. The tail carrying the debt
    /// IS it being unanswered: an answer, a streaming tail or any later
    /// block would be the tail instead, and mid-turn absorption keeps its
    /// own semantics for those. An owing tail hands over the authority its
    /// debt carries, folded through the kind's pre-migration rule.
    async fn owing_tail_debt(
        &self,
        conversation_id: i64,
    ) -> Result<Option<kind::TailDebt>, CoreError> {
        use agent_ledger::FromBlock;
        let Some(tail) = self.ctx.store().latest_block(conversation_id).await? else {
            return Ok(None);
        };
        Ok(match AssistantKind::from_block(&tail) {
            AssistantKind::ChatMessage(message) if message.owes_answer() => Some(kind::TailDebt {
                authority: message.carried_debt_authority(),
            }),
            AssistantKind::ChatMessage(_) | AssistantKind::Core(_) => None,
        })
    }

    /// The first budget refusing this message's own debt, principal before
    /// channel, or `None` when every enabled budget admits it. Consulted for
    /// addressed messages only, inside the stamp serialization; each count
    /// is derived from the ledger at this write — no counter table, no
    /// in-memory tally — so the budget's whole state is the recent recorded
    /// history itself.
    async fn refusing_budget(
        &self,
        principal_id: i64,
        conversation_id: i64,
    ) -> Result<Option<kind::LimitedBy>, CoreError> {
        let tx = self.ctx.store().tx();
        if let Some(budget) = &self.protection.principal {
            let opened =
                kind::opened_debts_by_principal(&tx, principal_id, budget.window_seconds).await?;
            if opened >= i64::from(budget.answers.get()) {
                return Ok(Some(kind::LimitedBy::Principal));
            }
        }
        if let Some(budget) = &self.protection.channel {
            let opened =
                kind::opened_debts_in_conversation(&tx, conversation_id, budget.window_seconds)
                    .await?;
            if opened >= i64::from(budget.answers.get()) {
                return Ok(Some(kind::LimitedBy::Channel));
            }
        }
        Ok(None)
    }
}
