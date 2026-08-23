//! The report tool and the report block kind: a member's ask, filed as a
//! block, delivered with the turn's answer (decided 2026-08-23).
//!
//! The flow: a member replies to an offending message and addresses the
//! assistant asking for a report. The tool takes NO target parameter — it
//! resolves the reply target through the debt origin walk decision 0043
//! settled, never the bare anchor, because the anchor can be a bystander's
//! line. Executing the tool appends a [`Report`] block carrying the target
//! origin, the reported message's principal id and the fixed report line;
//! the consumer's outbound edge delivers the line threaded onto the
//! reported message, before the answer, on the turn's completion and on its
//! failure alike. The block projects nothing: the filed report is
//! machinery, and the model's knowledge of it is the tool result.
//!
//! This crosses unit 5's "no tool writes anywhere" rule under its dated
//! amendment: a tool may append blocks of kinds that exist for tool-driven
//! delivery; lookups still write nothing.
//!
//! Filings are bounded per channel by the atomic line window under
//! [`REPORT_WINDOW`]: the grant is taken atomically — a second call in the
//! same round loses it and gets the declined result — and revoked when the
//! append fails transiently, so the slot is spent only once the append
//! stands, mirroring the unit-7 ordering fix. The append holds the erasure
//! fence, so a report cannot re-materialize an origin an erasure just
//! nulled; the reported message's principal id is stored precisely so
//! erasure can reach the block, through the crate-private
//! `erase_reported_origin` pass the erasure operation composes.
//!
//! [`REPORT_WINDOW`]: crate::window::REPORT_WINDOW

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, CoreEvent, FromBlock, LeafKind,
    Projection, StoreError, ToolContext, ToolHandler, ToolOutcome,
};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{Authority, ChannelKind};
use crate::tools::provenance::{StoredReply, newest_co_summoner_reply};
use crate::window::LineWindow;

// ─── The block kind ──────────────────────────────────────────────────────

/// The stored type string of the report kind.
pub const REPORT_KIND: &str = "report";

/// The content table the kind's descriptor owns.
pub const REPORT_TABLE: &str = "block_report";

/// The reported message's platform origin — what the edge threads the
/// report line onto. Nullable for exactly one reason: the reported
/// person's erasure nulls it, and the edge skips the targetless report.
pub const COLUMN_TARGET_ORIGIN: &str = "target_origin";
/// The reported message's sender in the identity tables — stored precisely
/// so erasure can reach this block by the reported principal.
pub const COLUMN_REPORTED_PRINCIPAL_ID: &str = "reported_principal_id";
/// The fixed report line the edge delivers. It names nobody, so erasure
/// leaves it.
pub const COLUMN_LINE: &str = "line";

/// One filed report awaiting delivery. Absences are typed per the kind
/// contract: a nulled target origin is the one absence with stored meaning
/// — erased, and therefore undeliverable.
#[derive(Debug, Clone)]
pub struct Report {
    /// The reported message's origin. `None` after the reported person's
    /// erasure — the edge skips the report — or for a row the store did
    /// not produce.
    pub target_origin: Option<String>,
    /// The reported message's sender. `None` only for a row the store did
    /// not produce (the schema stores it NOT NULL).
    pub reported_principal_id: Option<i64>,
    /// The fixed line the edge delivers. `None` only for a row the store
    /// did not produce.
    pub line: Option<String>,
}

impl Report {
    /// The stored shape of one report block: the field map the tool's
    /// append carries, encoded by the module that decodes it back.
    #[must_use]
    pub fn stored_fields(
        target_origin: &str,
        reported_principal_id: i64,
        line: &str,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TARGET_ORIGIN.into(), json!(target_origin));
        fields.insert(
            COLUMN_REPORTED_PRINCIPAL_ID.into(),
            json!(reported_principal_id),
        );
        fields.insert(COLUMN_LINE.into(), json!(line));
        fields
    }
}

impl LeafKind for Report {
    const KINDS: &'static [&'static str] = &[REPORT_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: REPORT_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[REPORT_KIND],
        columns: &[
            Column::new(COLUMN_TARGET_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_REPORTED_PRINCIPAL_ID, ColumnType::Integer),
            Column::new(COLUMN_LINE, ColumnType::Text),
        ],
        reference_columns: &[],
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            target_origin: block
                .fields
                .get(COLUMN_TARGET_ORIGIN)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            reported_principal_id: block
                .fields
                .get(COLUMN_REPORTED_PRINCIPAL_ID)
                .and_then(Value::as_i64),
            line: block
                .fields
                .get(COLUMN_LINE)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }
}

/// Agency-inert, and frontier-transparent on purpose: the block is written
/// INTO a live turn's window by the tool, so the owed-turn decision must
/// read through it — a report over an unanswered message buries nothing.
impl Agency for Report {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Invisible to the model in every mode: the filed report is machinery,
/// and the model's knowledge of it is the tool result.
impl Projection for Report {}

/// Null the target origin of every report naming this principal as the
/// reported person — erasure's reach into the report block (decided
/// 2026-08-23, narrowing the 0045 lineage): the line goes undeliverable
/// and the edge skips it. The line text stays; it names nobody. Nulling
/// already-null columns is a no-op, so the step is idempotent.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_reported_origin(
    tx: &StoreTx,
    principal_id: i64,
) -> Result<(), StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {REPORT_TABLE} SET {COLUMN_TARGET_ORIGIN} = NULL \
                 WHERE {COLUMN_REPORTED_PRINCIPAL_ID} = ?1"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

// ─── The tool ────────────────────────────────────────────────────────────

/// The registered name the model calls the tool by.
pub const NAME: &str = "report_spam";

/// The authority this tool requires — member: reporting is every member's
/// ask. The admission gate supplies no extra protection at this bar; the
/// tool sits under it because every tool does (stated, not implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// What the fixed report line opens with; the configured moderation handle
/// follows directly. The moderation bot acts on replies carrying exactly
/// this command shape.
pub const REPORT_LINE_LEAD: &str = "/report@";

/// The filed result the model reads. It claims filing, not arrival: the
/// report goes out with this turn, and a failed platform send is logged
/// and not retried.
pub const FILED_RESULT: &str = "The report is filed and goes out with this turn, as a reply \
     to the reported message. Do not call this tool again this turn; answer from what you \
     already have.";

/// The declined result: the channel's report window is spent — a second
/// ask inside it, or a second call in the same round, loses the atomic
/// grant.
pub const DECLINED_RESULT: &str = "declined: this conversation already filed a report \
     inside the report window. Do not call this tool again this turn; answer from what \
     you already have.";

/// The no-reply error: the turn's origin set carries no reply target, and
/// a report needs the member's reply as its ground truth.
pub const NEEDS_REPLY_ERROR: &str = "a report needs a reply: the ask does not reply to the \
     message being reported. Tell the member to reply to the offending message and ask \
     again. Do not call this tool again this turn; answer from what you already have.";

/// The self-report refusal: the reply points at one of the assistant's own
/// messages.
pub const SELF_REPORT_ERROR: &str = "declined: the reply points at one of the assistant's \
     own messages, and the assistant does not report itself. Do not call this tool again \
     this turn; answer from what you already have.";

/// The direct-conversation refusal: reports belong to groups, where the
/// moderation bot and its administrators are.
pub const GROUP_ONLY_ERROR: &str = "declined: reports are filed in group conversations \
     only. Do not call this tool again this turn; answer from what you already have.";

/// The unrecorded-target refusal: the replied-to message resolves to no
/// recorded principal — a message the assistant never recorded, or one
/// whose origin an erasure nulled — so no report can name a person erasure
/// could later reach, and filing one would ship an identifier out of
/// erasure's reach (the exact gap decision 0003 exists to prevent).
///
/// Noted 2026-08-23: the unit spec's result enumeration predates this
/// refusal and does not yet name it, though the spec forces it by storing
/// the reported principal NOT NULL. The spec line naming it is still owed;
/// the wording pin below holds the shipped refusal exact meanwhile.
pub const UNRECORDED_TARGET_ERROR: &str = "declined: the replied-to message is not in the \
     assistant's records, so no report can name it. Do not call this tool again this \
     turn; answer from what you already have.";

/// The transient failure: a read or the append did not stand, so nothing
/// was filed and nothing was spent. No no-retry line — the fact may not
/// hold beyond this failure.
fn transient_error() -> String {
    "the report could not be filed right now; nothing was filed.".to_owned()
}

/// The fixed report line for one configured moderation handle.
#[must_use]
pub fn report_line(handle: &str) -> String {
    format!("{REPORT_LINE_LEAD}{handle}")
}

/// The report tool: member authority, group conversations only, no
/// parameters. Constructed by the assembly when a moderation handle is
/// configured — the report window and the erasure fence are injected here,
/// at registration, so the tool never reaches into the assembly.
pub(crate) struct ReportTool {
    /// The configured moderation handle, already trimmed with its leading
    /// `@` stripped by the configuration layer.
    handle: String,
    /// The per-channel filing bound — process memory under
    /// [`crate::window::REPORT_WINDOW`], injected at construction.
    window: LineWindow,
    /// The erasure fence, held shared across the resolution and the append
    /// so a report cannot re-materialize an origin an erasure just nulled.
    /// Taken as the bare shared lock, not as the assembly's own alias for
    /// it — a leaf tool names nothing in the module that registers it.
    fence: Arc<RwLock<()>>,
}

impl ReportTool {
    pub(crate) fn new(
        handle: impl Into<String>,
        window: LineWindow,
        fence: Arc<RwLock<()>>,
    ) -> Self {
        Self {
            handle: handle.into(),
            window,
            fence,
        }
    }

    /// The whole filing, under the erasure fence. `Err` carries the tool
    /// error the runner records and the model reads.
    async fn file(&self, ctx: &ToolContext<'_, CoreEvent>) -> Result<&'static str, String> {
        let _no_erasure_mid_filing = self.fence.read().await;
        let conversation_id = ctx.agency.conversation_id;
        let tx = ctx.agency.store.tx();
        // Group conversations only: the moderation bot and the admins its
        // report pings live in groups; a direct chat has neither.
        match mapping::kind_for_conversation(&tx, conversation_id).await {
            Ok(Some(ChannelKind::Group)) => {}
            Ok(Some(ChannelKind::Direct) | None) => return Err(GROUP_ONLY_ERROR.to_owned()),
            Err(error) => {
                tracing::warn!(conversation_id, %error, "the report's mapping read failed");
                return Err(transient_error());
            }
        }
        let ledger = match ctx.agency.store.list_blocks(conversation_id).await {
            Ok(ledger) => ledger,
            Err(error) => {
                tracing::warn!(conversation_id, %error, "the report's ledger read failed");
                return Err(transient_error());
            }
        };
        // The target: the newest co-summoner's stored reply, through the
        // debt origin walk — never the bare anchor, whose line can be a
        // bystander's (decision 0043 records the exact shape).
        let target = match newest_co_summoner_reply(&ledger, ctx.block_id) {
            None => return Err(NEEDS_REPLY_ERROR.to_owned()),
            Some(StoredReply::ToAssistant) => return Err(SELF_REPORT_ERROR.to_owned()),
            Some(StoredReply::Target(origin)) => origin,
        };
        // The reported principal: the recorded message the reply points
        // at. An unrecorded or erased target files nothing — a report
        // erasure could never reach must not exist.
        let Some(reported_principal_id) = reported_principal(&ledger, &target) else {
            return Err(UNRECORDED_TARGET_ERROR.to_owned());
        };
        // The atomic grant, taken before the append so a second call in
        // the same round is declined, and revoked below when the append
        // does not stand — the slot is spent only once the filing is real.
        if !self.window.grants(conversation_id).await {
            return Err(DECLINED_RESULT.to_owned());
        }
        let appended = ctx
            .agency
            .store
            .append_consumer_block(
                conversation_id,
                None,
                REPORT_KIND,
                Report::stored_fields(&target, reported_principal_id, &report_line(&self.handle)),
                None,
            )
            .await;
        if let Err(error) = appended {
            self.window.revoke(conversation_id).await;
            tracing::warn!(conversation_id, %error, "the report's append failed; nothing spent");
            return Err(transient_error());
        }
        Ok(FILED_RESULT)
    }
}

/// The principal behind the recorded message this origin names, newest
/// match first. `None` when no recorded message carries the origin — never
/// invented.
fn reported_principal(ledger: &[Block], origin: &str) -> Option<i64> {
    ledger
        .iter()
        .rev()
        .find_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) if message.origin.as_deref() == Some(origin) => {
                message.principal_id
            }
            _ => None,
        })
}

impl ToolHandler<CoreEvent> for ReportTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "File a spam report with the group's moderation bot. Use it only \
                 when a member replies to an offending message and asks for a report; the \
                 reported message is the one the member replied to, so the tool takes no \
                 arguments. This tool is the only way to report — never write the report \
                 command into an answer yourself."
                .into(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        // The input is ignored on purpose: the tool takes no target
        // parameter — projection carries no message handles, and the
        // member's reply is the ground truth.
        Box::pin(async move {
            match self.file(&ctx).await {
                Ok(filed) => ToolOutcome::Done(filed.into()),
                Err(error) => ToolOutcome::Error(error),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::admission::NO_RETRY;

    fn report_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: REPORT_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    #[test]
    fn the_stored_fields_round_trip_through_the_parse() {
        let report = Report::parse(&report_block(Report::stored_fields(
            "origin-77",
            42,
            "/report@moderation_bot",
        )));
        assert_eq!(report.target_origin.as_deref(), Some("origin-77"));
        assert_eq!(report.reported_principal_id, Some(42));
        assert_eq!(report.line.as_deref(), Some("/report@moderation_bot"));
    }

    #[test]
    fn a_report_is_inert_transparent_and_invisible() {
        let report = Report::parse(&report_block(Report::stored_fields("o", 1, "line")));
        assert_eq!(report.awaiting(), None, "a report summons nothing");
        assert!(
            report.frontier_transparent(),
            "the owed-turn frontier reads through it"
        );
        assert!(report.durable(), "a report is a durable ledger row");
        assert_eq!(report.group_role(), None, "invisible to projection");
        assert_eq!(report.llm_text(), None);
        assert_eq!(report.llm_parts(), None);
    }

    #[test]
    fn the_report_line_is_the_lead_plus_the_handle() {
        assert_eq!(REPORT_LINE_LEAD, "/report@");
        assert_eq!(report_line("moderation_bot"), "/report@moderation_bot");
    }

    /// The exact copy of every fixed result, pinned verbatim: the filed
    /// result claims filing with this turn — never arrival — and every
    /// refusal closes with the admission wrapper's no-retry teaching,
    /// while the transient failure names the moment and teaches nothing.
    #[test]
    fn the_result_wording_is_pinned_verbatim() {
        assert_eq!(
            FILED_RESULT,
            "The report is filed and goes out with this turn, as a reply to the reported \
             message. Do not call this tool again this turn; answer from what you already \
             have."
        );
        assert_eq!(
            DECLINED_RESULT,
            "declined: this conversation already filed a report inside the report window. \
             Do not call this tool again this turn; answer from what you already have."
        );
        assert_eq!(
            NEEDS_REPLY_ERROR,
            "a report needs a reply: the ask does not reply to the message being reported. \
             Tell the member to reply to the offending message and ask again. Do not call \
             this tool again this turn; answer from what you already have."
        );
        assert_eq!(
            SELF_REPORT_ERROR,
            "declined: the reply points at one of the assistant's own messages, and the \
             assistant does not report itself. Do not call this tool again this turn; \
             answer from what you already have."
        );
        assert_eq!(
            GROUP_ONLY_ERROR,
            "declined: reports are filed in group conversations only. Do not call this \
             tool again this turn; answer from what you already have."
        );
        assert_eq!(
            UNRECORDED_TARGET_ERROR,
            "declined: the replied-to message is not in the assistant's records, so no \
             report can name it. Do not call this tool again this turn; answer from what \
             you already have."
        );
        for closes_with_no_retry in [
            FILED_RESULT,
            DECLINED_RESULT,
            NEEDS_REPLY_ERROR,
            SELF_REPORT_ERROR,
            GROUP_ONLY_ERROR,
            UNRECORDED_TARGET_ERROR,
        ] {
            assert!(
                closes_with_no_retry.ends_with(NO_RETRY),
                "every whole-turn result closes with the no-retry teaching: \
                 {closes_with_no_retry}"
            );
        }
        let transient = transient_error();
        assert!(
            transient.contains("right now") && !transient.contains(NO_RETRY),
            "a transient fact names the moment and teaches no never-again: {transient}"
        );
    }
}
