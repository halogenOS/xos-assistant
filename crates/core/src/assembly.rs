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

use std::collections::HashMap;
use std::future::Future;
use std::num::{NonZeroU32, NonZeroU64};
use std::pin::Pin;
use std::sync::Arc;

use agent_ledger::store::ProviderInstance;
use agent_ledger::{
    CoreEvent, EventBus, FromBlock, ProviderRegistry, Role, RuntimeContext, Store, spawn_reactor,
};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::erasure::{self, ErasureOutcome};
use crate::error::CoreError;
use crate::kind::{self, AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage};
use crate::message::{
    ChannelKey, ChannelKind, DeliveryItem, InboundMessage, IngestOutcome, IngestReceipt,
    Observation, ObserveOutcome, ObservedFact, OutboundReply,
};
use crate::note::{self, ContextNote, NoteTopic};
use crate::outbound::RULES_ACKNOWLEDGMENT;
use crate::streams::StreamObserver;
use crate::tools::{ToolSet, palette::TOOL_PALETTE_KIND, palette::ToolPalette};
use crate::window::{AppendWindow, LineWindow};
use crate::{authorization, identity, mapping, outbound, privacy, streams};

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

/// The operator wiring of the group-context unit (decided 2026-08-23): who
/// may admit the assistant into a group, per adapter. It comes from the
/// embedder's configuration and is absent by default — with no operator
/// configured, every group add fails closed.
#[derive(Debug, Clone, Default)]
pub struct OperatorConfig {
    /// The operator's adapter-scoped external id, keyed by adapter name. A
    /// membership observation authorizes a group only when its adder's
    /// external id matches this entry for the observation's adapter.
    pub by_adapter: HashMap<String, String>,
}

/// A scripted pause between the on-delta newest-note read and its append —
/// the observation race's test seam, mirroring the adapter's injectable
/// sleep: a suite pins the stamp lock's serialization without racing the
/// scheduler. Production installs nothing and nothing pauses.
pub type NoteReadPause = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Everything the embedder decides about the assistant beyond its runtime
/// wiring: the model binding, the system prompt, the answering budgets, the
/// operator wiring and the privacy policy address — one carrier, so a later
/// policy knob joins here instead of growing the constructor.
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// The model every new conversation is created under.
    pub binding: ModelBinding,
    /// The system prompt recorded into every conversation at its creation.
    pub system_prompt: String,
    /// The answering budgets the stamp consults for addressed messages.
    pub protection: ProtectionConfig,
    /// The operator wiring: who may admit the assistant into a group.
    pub operators: OperatorConfig,
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line.
    pub privacy_policy_address: Option<String>,
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
    /// The tool names every new conversation's palette block records —
    /// exactly the set the assembly registered, derived from the one
    /// [`ToolSet`] so the palette and the registry cannot name different
    /// tools. Written at creation beside the system prompt; a conversation
    /// created before the palette existed carries no block and admits
    /// nothing.
    palette: Vec<String>,
    /// The streaming state the erasure ordering reads; the observation's
    /// contract and its lossy edges are stated on the streams module.
    streams: Arc<StreamObserver>,
    /// The answering budgets the stamp consults for addressed messages.
    /// Read-only after start; the budget counts themselves are derived
    /// from the ledger at every write — the acknowledgment windows below
    /// are the one in-memory tally, and they bound courtesy lines, never
    /// budgets.
    protection: ProtectionConfig,
    /// The operator wiring: who may admit the assistant into a group.
    /// Read-only after start.
    operators: OperatorConfig,
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line. Read-only after start.
    privacy_policy_address: Option<String>,
    /// The rules acknowledgment's per-channel window — at most one inside
    /// it; a further rules delta appends its note silently. The window
    /// module states why the bookkeeping is in-memory.
    rules_acknowledged: LineWindow,
    /// The privacy answer's per-channel window, sharing the acknowledgment
    /// mechanism (refined 2026-08-23): the command stamp keeps the command
    /// out of both budget counts, so without this a quiet channel would
    /// answer every repeat.
    command_answered: LineWindow,
    /// The per-topic note append cap within the window (refined
    /// 2026-08-23): the ledger is bounded like the chat line, and a capped
    /// delta lands on the next observation after the window.
    note_appends: AppendWindow<(i64, NoteTopic)>,
    /// The observation race's test seam; `None` in production.
    note_read_pause: Option<NoteReadPause>,
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
        tools: ToolSet,
        config: AssemblyConfig,
    ) -> Result<Self, CoreError> {
        let AssemblyConfig {
            binding,
            system_prompt,
            protection,
            operators,
            privacy_policy_address,
        } = config;
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
        // One source for what tools exist: the registry the runtime resolves
        // calls against and the palette every new conversation records are
        // both derived from the set right here.
        let (registry, palette) = tools.into_registry();
        let ctx: RuntimeContext<AssistantKind, CoreEvent> =
            RuntimeContext::new(store, bus, providers, Arc::new(registry));
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
            palette,
            streams,
            protection,
            operators,
            privacy_policy_address,
            rules_acknowledged: LineWindow::new(),
            command_answered: LineWindow::new(),
            note_appends: AppendWindow::new(),
            note_read_pause: None,
            stamp_lock: Mutex::new(()),
            erasure_fence: RwLock::new(()),
        })
    }

    /// Install the observation race's test seam: the given pause runs
    /// between the on-delta newest-note read and its append, inside the
    /// stamp lock — which is exactly why a suite can prove the lock holds
    /// the read-then-append window. Production never calls this.
    pub fn pause_between_note_read_and_append(&mut self, pause: NoteReadPause) {
        self.note_read_pause = Some(pause);
    }

    /// The ingestion edge: record one inbound message and answer with what
    /// the write decided — the resolved ids, and any deterministic item the
    /// adapter carries out.
    ///
    /// A group message is admitted first: a group channel with no
    /// authorization row is refused fail-closed with
    /// [`IngestOutcome::Withdraw`], touching neither the ledger nor the
    /// identity tables — the row-or-refusal shape is what makes the check
    /// survive lost leave calls and restarts alike. Direct channels are
    /// untouched by the check.
    ///
    /// An admitted message resolves or creates the sender's principal, maps
    /// the channel — creating the conversation under the assembly's
    /// binding, with the assembly's system prompt recorded first, on first
    /// message — stamps the message, and appends the message block through
    /// the framework's consumer write path. The stamp order is fixed:
    /// addressing first; the privacy command stamped with the command kind
    /// ahead of any budget — a command takes no debt by its nature; budgets
    /// consulted only for addressed non-command messages, principal before
    /// channel, the first refusing budget naming the limited fact; then
    /// answer-due by the composition rule — due when the message's own debt
    /// was taken (addressed, not limited) or when the tail owes, so a
    /// refused sender's message can be denied its own answer but can never
    /// cancel a debt it merely propagates; then the debt authority by the
    /// minimum rule. Nothing past admission ever drops a message:
    /// protection limits answering, never recording.
    ///
    /// Authority is read only past admission (refined 2026-08-23): an
    /// unadmitted group is refused with the withdraw directive before this
    /// method ever looks at the sender's standing, and an admitted message
    /// whose authority arrived unresolved is refused with the transient
    /// [`CoreError::AuthorityUnresolved`] — never recorded with a default.
    ///
    /// The privacy command's fixed answer rides the returned outcome — the
    /// return-value transport of decision 2026-08-23, never the event edge
    /// — and is granted only while no budget refuses the sender AND the
    /// channel's answer window admits it: the command shares the
    /// acknowledgment-window mechanism (refined 2026-08-23), at most one
    /// answer per channel per window, recorded silence within it — the
    /// notice discipline of the protection unit.
    ///
    /// A message whose own debt was taken then emits the unlatch intent,
    /// always: a person addressing the assistant IS the deliberate
    /// re-engagement, the intent is idempotent, and the same emission
    /// releases a fresh conversation's boot latch and a stream error's
    /// re-latch alike. An unaddressed message never unlatches, and neither
    /// does a limited or command-stamped one — neither is re-engagement.
    ///
    /// # Errors
    ///
    /// [`CoreError::ChannelKindMismatch`] if the channel is already mapped
    /// under a different kind than the message claims.
    /// [`CoreError::ClaimLost`] if a first-message claim lost its mapping
    /// row mid-claim. [`CoreError::Store`] if identity resolution, mapping
    /// or the append fails.
    pub async fn ingest(&self, message: InboundMessage) -> Result<IngestOutcome, CoreError> {
        let _no_erasure_mid_message = self.erasure_fence.read().await;
        let tx = self.ctx.store().tx();

        // The mapping's stored kind refuses a mis-claimed channel before
        // anything else: the mapping knows what the channel is, and every
        // later step decides personal-data handling by the kind.
        let mapped = mapping::find(&tx, &message.channel).await?;
        if let Some((_, stored_kind)) = mapped
            && stored_kind != message.channel_kind
        {
            return Err(CoreError::ChannelKindMismatch {
                stored: stored_kind,
                claimed: message.channel_kind,
            });
        }
        if message.channel_kind == ChannelKind::Group
            && !authorization::is_authorized(&tx, &message.channel).await?
        {
            return Ok(IngestOutcome::Withdraw);
        }
        // Past admission, the write needs the sender's standing; delivered
        // unresolved, the message is refused transient and redelivers —
        // the never-default rule, without letting a stranger group's
        // failing authority source starve the batch (see the error's doc).
        let Some(authority) = message.authority else {
            return Err(CoreError::AuthorityUnresolved);
        };

        let principal_id = identity::resolve_principal(
            &tx,
            message.channel.adapter.clone(),
            message.sender.clone(),
        )
        .await?;

        let conversation_id = match mapped {
            Some((existing, _)) => existing,
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
        let is_command = privacy::is_privacy_command(message.command.as_ref());
        let limited = if is_command {
            Some(kind::LimitedBy::Command)
        } else if message.addressed {
            self.refusing_budget(principal_id, conversation_id).await?
        } else {
            None
        };
        // The composition rule and the minimum rule live on the kind, as
        // one pure composition beside the stamp's readers.
        let stamp = kind::Stamp::compose(message.addressed, authority, limited, owing_tail);
        // The command's budget half is decided inside the stamp
        // serialization, so the consultation it shares with the stamp sees
        // the same recorded history: answered only while every budget
        // admits the sender — recorded silence otherwise.
        let command_admitted = is_command
            && self
                .refusing_budget(principal_id, conversation_id)
                .await?
                .is_none();
        let fields = ChatMessage::stored_fields(
            &message.text,
            principal_id,
            authority,
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
        // The window is consulted last, after the append stands: a
        // budget-refused command never spends it, and neither does one
        // whose append failed transiently — a grant spent before the
        // append would answer the redelivered command with silence.
        let deliver = if command_admitted && self.command_answered.grants(conversation_id).await {
            Some(DeliveryItem::CommandAnswer(privacy::privacy_answer(
                self.privacy_policy_address.as_deref(),
            )))
        } else {
            None
        };
        if stamp.own_debt_taken() {
            self.ctx
                .bus()
                .emit(CoreEvent::UnlatchRequested { conversation_id });
        }
        Ok(IngestOutcome::Recorded {
            receipt: IngestReceipt {
                principal_id,
                conversation_id,
            },
            deliver,
        })
    }

    /// The observation edge: judge one platform-neutral observation and
    /// answer with what was decided — the fail-closed refusal, or the
    /// appended note's acknowledgment. Everything deterministic rides this
    /// return; the event edge stays the model's answers and the failure
    /// notice (decided 2026-08-23).
    ///
    /// A membership observation is the authorization table's one writer: an
    /// add whose adder matches the configured operator records the group's
    /// admission and stands across restarts. A foreign adder, a missing
    /// adder, or no configured operator answers [`ObserveOutcome::Withdraw`]
    /// and records nothing — and so does any title or announcement
    /// observation for a group without an authorization row, so a lost
    /// leave call is healed by the group's next contact.
    ///
    /// An admitted title or announcement observation derives its note — the
    /// rules contract reads the pinned announcement here, in the core — and
    /// appends it on-delta: only when the observed text differs from the
    /// newest stored note of the same topic, and only within the topic's
    /// per-window append cap (refined 2026-08-23) — a capped delta is not
    /// queued, it lands on the next observation after the window through
    /// the on-delta rule itself. The read-then-append is serialized under
    /// the stamp lock, so two equal racing observations append one note; an
    /// authorized, unmapped group channel takes the same winner-only
    /// creation path a first message does, system prompt and palette
    /// included. A fresh rules note outside the acknowledgment window
    /// carries the fixed acknowledgment back to the adapter; a title note
    /// is never acknowledged.
    ///
    /// A direct-channel observation observes nothing: group facts belong to
    /// groups, and the direct path is untouched by this unit.
    ///
    /// # Errors
    ///
    /// [`CoreError::ChannelKindMismatch`] if the channel is already mapped
    /// under a different kind than the observation claims.
    /// [`CoreError::ClaimLost`] if a first-contact claim lost its mapping
    /// row mid-claim. [`CoreError::Store`] if a read or the append fails.
    pub async fn observe(&self, observation: Observation) -> Result<ObserveOutcome, CoreError> {
        let _no_erasure_mid_observation = self.erasure_fence.read().await;
        let tx = self.ctx.store().tx();

        if let Some((_, stored_kind)) = mapping::find(&tx, &observation.channel).await?
            && stored_kind != observation.channel_kind
        {
            return Err(CoreError::ChannelKindMismatch {
                stored: stored_kind,
                claimed: observation.channel_kind,
            });
        }
        if observation.channel_kind != ChannelKind::Group {
            tracing::debug!("a direct-channel observation observes nothing");
            return Ok(ObserveOutcome::Observed { deliver: None });
        }

        match observation.fact {
            ObservedFact::Added { by } => {
                let operator = self
                    .operators
                    .by_adapter
                    .get(&observation.channel.adapter)
                    .map(String::as_str);
                if !authorization::operator_admits(operator, by.as_ref()) {
                    return Ok(ObserveOutcome::Withdraw);
                }
                authorization::authorize(&tx, &observation.channel).await?;
                Ok(ObserveOutcome::Observed { deliver: None })
            }
            ObservedFact::Title(_) | ObservedFact::PinnedAnnouncement(_) => {
                if !authorization::is_authorized(&tx, &observation.channel).await? {
                    return Ok(ObserveOutcome::Withdraw);
                }
                let Some((topic, text)) = note::note_of(&observation.fact) else {
                    return Ok(ObserveOutcome::Observed { deliver: None });
                };
                // The on-delta read-then-append, serialized against every
                // other stamp-locked write: without the lock two equal
                // observations would both read the pre-append newest note
                // and both append. Mapping resolution sits inside the lock
                // for the same reason — the loser of a creation race must
                // see the winner's note, not its own empty conversation.
                let _one_stamp_at_a_time = self.stamp_lock.lock().await;
                let conversation_id = match mapping::find(&tx, &observation.channel).await? {
                    Some((existing, _)) => existing,
                    None => {
                        self.map_new_channel(&observation.channel, ChannelKind::Group)
                            .await?
                    }
                };
                let newest = note::newest_text(self.ctx.store(), conversation_id, topic).await?;
                if let Some(pause) = &self.note_read_pause {
                    pause().await;
                }
                if newest.as_deref() == Some(text.as_str()) {
                    return Ok(ObserveOutcome::Observed { deliver: None });
                }
                if !self.note_appends.admits((conversation_id, topic)).await {
                    tracing::info!(
                        conversation_id,
                        topic = topic.as_str(),
                        "the topic's appends are capped for this window; \
                         the delta lands on the next observation after it"
                    );
                    return Ok(ObserveOutcome::Observed { deliver: None });
                }
                self.ctx
                    .store()
                    .append_consumer_block(
                        conversation_id,
                        None,
                        note::CONTEXT_NOTE_KIND,
                        ContextNote::stored_fields(topic, &text),
                        None,
                    )
                    .await?;
                // The cap slot and the acknowledgment window are spent only
                // past the successful append: a transiently failed append
                // redelivers its observation, and spending either before
                // the append would cap or silence the redelivery for a
                // note that never landed.
                self.note_appends.record((conversation_id, topic)).await;
                let deliver = match topic {
                    NoteTopic::Rules => self
                        .rules_acknowledged
                        .grants(conversation_id)
                        .await
                        .then(|| DeliveryItem::Acknowledgment(RULES_ACKNOWLEDGMENT.to_owned())),
                    NoteTopic::Title => None,
                };
                Ok(ObserveOutcome::Observed { deliver })
            }
        }
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
    /// assembly's binding, record the system prompt and the tool palette as
    /// its first blocks, and claim the mapping. Direct and group channels
    /// take the identical path, so both get the same palette.
    ///
    /// Two ingestions can race here; the mapping's claim decides, and the
    /// loser's conversation is deleted — its prompt and palette blocks with
    /// it — before anything referenced it. Recording both before the claim
    /// is what makes them the winner's first blocks: the losing racer's
    /// message arrives in the winning conversation only after the winner's
    /// records are in.
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
        store
            .append_consumer_block(
                created,
                None,
                TOOL_PALETTE_KIND,
                ToolPalette::stored_fields(&self.palette),
                None,
            )
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
    /// own semantics for those — an addressed message absorbed mid-turn
    /// opens a fresh debt at its own authority, correct for answering (the
    /// next turn pays it; decision 0021), while tool admission folds such
    /// a co-summoner in by its stored sender authority under the
    /// opened-debt predicate, not through this debt stamp (decision 0043).
    /// An owing tail hands over the authority its debt carries, folded
    /// through the kind's pre-migration rule.
    ///
    /// The read walks past context notes exactly (refined 2026-08-23): a
    /// note appended over an unanswered message leaves the turn owed, so
    /// the debt it carries must propagate through the note to the next
    /// message's stamp — while the framework's other transparent kinds,
    /// the turn-closure markers above all, stay a settled tail here: the
    /// framework's own walk governs turn liveness, and reading debt
    /// through a closed turn's marker would widen propagation past failed
    /// turns. Every read is bounded — one row behind the tail at most,
    /// never a conversation hydration — because this sits on ingestion's
    /// hot path and the framework leaves a transparent turn-end marker as
    /// the tail of every answered conversation.
    async fn owing_tail_debt(
        &self,
        conversation_id: i64,
    ) -> Result<Option<kind::TailDebt>, CoreError> {
        let store = self.ctx.store();
        let Some(tail) = store.latest_block(conversation_id).await? else {
            return Ok(None);
        };
        let tail = if tail.block_type == note::CONTEXT_NOTE_KIND {
            match note::newest_block_id_past_notes(store, conversation_id).await? {
                Some(behind_the_notes) => store.find_block(behind_the_notes).await?,
                None => None,
            }
        } else {
            Some(tail)
        };
        Ok(match tail.map(|block| AssistantKind::from_block(&block)) {
            Some(AssistantKind::ChatMessage(message)) if message.owes_answer() => {
                Some(kind::TailDebt {
                    authority: message.carried_debt_authority(),
                })
            }
            Some(
                AssistantKind::ChatMessage(_)
                | AssistantKind::Core(_)
                | AssistantKind::ToolPalette(_)
                | AssistantKind::ContextNote(_),
            )
            | None => None,
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
