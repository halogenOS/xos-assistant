//! The core assembly: the runtime wiring, the ingestion entry point, the
//! outbound subscription and the erasure operation, in one place.
//!
//! The assembly is constructed with its runtime wiring — the store opened on
//! the assistant's configuration, the event bus, the provider and tool
//! registries — and the model binding under which first-message conversation
//! creation happens. The entry point draws the binding from here, never from
//! a message. The constructor's caller owns the store, the bus and the
//! registries it passed in; the adapter edges below are the only surface an
//! adapter touches.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use agent_ledger::store::ProviderInstance;
use agent_ledger::{
    CoreEvent, EventBus, ProviderRegistry, Role, RuntimeContext, Store, ToolRegistry, spawn_reactor,
};
use tokio::sync::{RwLock, mpsc};

use crate::erasure::{self, ErasureOutcome};
use crate::error::CoreError;
use crate::kind::{AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage};
use crate::message::{ChannelKey, ChannelKind, InboundMessage, OutboundReply};
use crate::{identity, mapping, outbound};

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
    /// The conversations whose boot latch this process has already released.
    /// The latch is per process, so a durable store's conversations return
    /// latched after a restart; the first successful ingestion into each one
    /// releases it, exactly once — a conversation a stream error re-latched
    /// is already in this set and stays latched.
    unlatched: Mutex<HashSet<i64>>,
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
        spawn_reactor(ctx.clone());
        Ok(Self {
            ctx,
            binding,
            unlatched: Mutex::new(HashSet::new()),
            erasure_fence: RwLock::new(()),
        })
    }

    /// The ingestion edge: record one inbound message and return the ids it
    /// resolved to.
    ///
    /// Resolves or creates the sender's principal, maps the channel —
    /// creating the conversation under the assembly's binding on first
    /// message — appends the message block through the framework's consumer
    /// write path, and only then releases the conversation's boot latch, on
    /// this process's first successful ingestion into it. The append is what
    /// wakes the runtime; the unlatch is what lets the woken conversation
    /// take a turn at all.
    ///
    /// A conversation that an earlier stream error re-latched stays latched:
    /// this process already released its boot latch once, and failure
    /// behavior on the outbound side is the live-model unit's decision, so
    /// no second unlatch fires here.
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

        let fields = ChatMessage::stored_fields(
            &message.text,
            principal_id,
            message.authority,
            message.origin.as_deref(),
            &message.timestamp.to_rfc3339(),
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
        self.release_boot_latch(conversation_id);
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
    /// — the outbound module's doc states the exact delivery contract.
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
    /// projects nothing to the model), the principal's direct conversations are
    /// removed entirely with their channel mappings, and the identity rows
    /// are deleted. Reports [`ErasureOutcome::NotFound`] — touching nothing
    /// — when no identity row matches, a completed earlier erasure included.
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
    /// [`CoreError::Store`] if a read, a write or a deletion fails.
    pub async fn erase_principal(&self, principal_id: i64) -> Result<ErasureOutcome, CoreError> {
        let _no_ingestion_mid_erasure = self.erasure_fence.write().await;
        let outcome = erasure::erase_principal(self.ctx.store(), principal_id).await?;
        if let ErasureOutcome::Erased {
            deleted_conversations,
        } = &outcome
        {
            // The store reissues a deleted conversation's id; a stale entry
            // here would suppress the reissued conversation's unlatch.
            // A poisoned lock is recoverable here: the set holds plain ids
            // and every holder only inserts or removes.
            let mut unlatched = self
                .unlatched
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for id in deleted_conversations {
                unlatched.remove(id);
            }
        }
        Ok(outcome)
    }

    /// First message on a channel: create the conversation under the
    /// assembly's binding and claim the mapping. The winner's boot latch is
    /// released by the caller, like every mapped conversation's.
    ///
    /// Two ingestions can race here; the mapping's claim decides, and the
    /// loser's conversation is deleted before anything referenced it.
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
        let winner = mapping::claim(&store.tx(), channel, kind, created).await?;
        if winner != created {
            store.delete_conversation(created).await?;
        }
        Ok(winner)
    }

    /// Release a conversation's boot latch with the explicit unlatch intent
    /// — without it no turn ever fires — the first time this process ingests
    /// into the conversation, and never again: the set remembers, so an
    /// in-process stream error's re-latch is not undone here.
    fn release_boot_latch(&self, conversation_id: i64) {
        let first = self
            .unlatched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(conversation_id);
        if first {
            self.ctx
                .bus()
                .emit(CoreEvent::UnlatchRequested { conversation_id });
        }
    }
}
