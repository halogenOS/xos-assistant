//! The core-spine integration suite: the assistant as the framework's
//! consumer, proven end to end against public API alone.
//!
//! One test binary on purpose: the scripted provider, the assembly helpers
//! and the ledger-polling helpers are shared by every module here, and a
//! single compilation keeps them all exercised. The modules split by concern:
//! acknowledgment (the rules acknowledgment's bounded one-shot generation
//! and its deterministic fallback),
//! assembly (the wiring contract), storage (the composed kind and the durable
//! registry), audience (the clarifying question's ordinary delivery and
//! the two-turn disambiguation),
//! addressing (the answer-due stamp, the notice, re-engagement),
//! bots (an automated sender summoned by address alone, its message
//! carrying no owing tail, and the debt waiting behind it for a carrier
//! entitled to it),
//! protection (the budgets, the limited stamp, the debt authority),
//! reasoning (the configured effort level on every created conversation
//! and on the provider's requests),
//! delivery (the delivery receipt: what one send records, the model
//! reading nothing of it, a receipt at the tail burying no debt, and the
//! reply-target column staying NULL),
//! `date_marker` (the framework's calendar row: written once per recorded
//! date, ahead of the message that tripped it, reaching the model as its
//! own system line — the fact every other module's consumer view filters),
//! `direct_chats` (the configuration switch refusing direct channels
//! before any write), disclosure (the first-interaction line and the
//! deterministic replies' exemption),
//! helpful (the answering mode's summons, the silent empty turn and the
//! unspent window),
//! joins (the join notice: its marked blocks through the observation seam,
//! its transparency on both walks, its reach into the report path and its
//! erasure by person and by event),
//! projection (role alternation under erasure),
//! quotes (a reply landing as a quote of the message it replies to: what
//! reaches the model, what quotes nothing, the erasure on either side,
//! the crash shape's debt and the refresh fork's parity),
//! speaker (the username
//! projection), erasure with its stream
//! ordering, the end-to-end turn, tools (the lookups against the scripted
//! forge and mirror in `lookup_wire`, the palette, the anchor gate over
//! the turn's provenance), `privacy_rights` (the suppression drop, the
//! self-service commands, the spawned deletion and the privacy tool),
//! `mirror` (the deletion mirror riding the moderation bot's reply
//! command), search (the web search's envelope over a scripted vendor, its
//! guard and person bound refusing before the wire, its taught failures,
//! and the one predicate deciding whether the tool exists), sourcing (the lookup-backed answer discipline: the literal
//! addressed fact beside the summons, the silent empty turn and the
//! model's own spoken don't-know), standing (the member-standing lookup an
//! ordinary member reaches in a group, stating what a person's last message
//! recorded, and declining outside a group),
//! threading (which message an answer is
//! delivered as a reply to, and when it goes out plain), `runtime_facts`
//! (the self-introspection tool an ordinary member reaches, stating the
//! model its own conversation runs on), and — behind the
//! `chat_completions`
//! feature — the framework's real `OpenRouter` module, the shared
//! chat-completions wire, against a loopback server.

mod acknowledgment;
mod addressing;
mod assembly;
mod audience;
mod bots;
#[cfg(feature = "chat_completions")]
mod chat_completions;
mod date_marker;
mod delivery;
mod direct_chats;
mod disclosure;
mod end_to_end;
mod erasure;
mod erasure_streams;
mod group_context;
mod helpful;
mod joins;
mod lookup_wire;
mod mirror;
mod privacy_rights;
mod projection;
mod protection;
mod quotes;
mod reasoning;
mod report;
mod runtime_facts;
mod search;
mod sourcing;
mod speaker;
mod standing;
mod storage;
mod support;
mod threading;
mod tools;
