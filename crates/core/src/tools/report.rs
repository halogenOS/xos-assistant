//! The report tool and the report block kind: the assistant's own
//! assessment of a rule-violating message, filed as a block, delivered
//! with the turn (member-initiated 2026-08-23; autonomous with a
//! validated target since 2026-08-24, unit 15).
//!
//! The flow: the prompt teaches the model to judge each group message
//! against the pinned rules, and on a clear violation the model calls this
//! tool NAMING the violating message by its projected id. The named origin
//! is validated against the turn's own co-summoner set — the messages the
//! model is actually assessing this turn — so the model gains the
//! precision to pick one violator among several absorbed messages and
//! cannot aim a report at anything else: not an old message, not an
//! arbitrary id, not another channel's. Executing the tool appends a
//! [`Report`] block carrying the target origin, the reported message's
//! principal id and the fixed report line; the consumer's outbound edge
//! delivers the line threaded onto the reported message, on the turn's
//! completion and on its failure alike, independent of whether the turn's
//! answer is spoken or empty. The block projects nothing: the filed
//! report is machinery, and the model's knowledge of it is the tool
//! result. The report is an assessment only — the moderation bot's human
//! administrators decide (decision 0070); the assistant takes no action.
//!
//! This crosses unit 5's "no tool writes anywhere" rule under its dated
//! amendment: a tool may append blocks of kinds that exist for tool-driven
//! delivery; lookups still write nothing.
//!
//! Filings are bounded per ORIGIN, not per channel (2026-08-24): a message
//! that dies unanswered re-co-summons the next turn, so without a bound it
//! could be re-assessed and re-reported — the filing scans the loaded
//! ledger for an existing report of the named origin and declines a
//! duplicate. Distinct violations in a busy hour are never throttled; the
//! same message is reported at most once, however many turns re-assess it.
//! The runner executes same-round calls in parallel tasks, so the
//! scan-then-append pair runs under the tool's own filing lock: of two
//! calls naming one origin, the second scans only after the first's block
//! landed, and the dedup declines it. The append holds the erasure
//! fence, so a report cannot re-materialize an origin an erasure just
//! nulled; the reported message's principal id is stored precisely so
//! erasure can reach the block, through the crate-private
//! `erase_reported_origin` pass the erasure operation composes.

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, CoreEvent, FromBlock, LeafKind,
    Projection, Role, StoreError, ToolContext, ToolHandler, ToolOutcome,
};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{Authority, ChannelKind};
use crate::tools::provenance::co_summoners;

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

/// The authority this tool requires — member: the turn behind an
/// autonomous assessment is summoned by ordinary members' messages. The
/// admission check supplies no extra protection at this bar; the tool sits
/// under it because every tool does (stated, not implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The one parameter: the violating message, named by the id the
/// projection shows in brackets ahead of it.
pub const PARAMETER_MESSAGE_ID: &str = "message_id";

/// What the fixed report line opens with; the configured moderation handle
/// follows directly. The moderation bot acts on replies carrying exactly
/// this command shape.
pub const REPORT_LINE_LEAD: &str = "/report@";

/// The filed result the model reads. It claims filing, not arrival: the
/// report goes out with this turn, and a failed platform send is logged
/// and not retried. No never-again teaching — a turn assessing several
/// messages may file one report per violator, each its own call.
pub const FILED_RESULT: &str = "The report is filed and goes out with this turn, as a reply \
     to the reported message. Do not report this message again.";

/// The anti-aiming decline: the named origin is not in the turn's
/// co-summoner set — an old message, an arbitrary id, another channel's —
/// so it is not one the model is assessing this turn.
pub const NOT_ASSESSED_ERROR: &str = "declined: that message is not one you are assessing \
     this turn. Do not call this tool again this turn; answer from what you already have.";

/// The duplicate decline: a report of the named origin already stands in
/// this conversation, and a message is reported at most once.
pub const ALREADY_REPORTED_ERROR: &str = "declined: that message is already reported, and \
     a message is reported at most once. Do not call this tool again this turn; answer \
     from what you already have.";

/// The missing-target decline: the call named no message id, and a report
/// names its target.
pub const NEEDS_TARGET_ERROR: &str = "declined: a report names its target — the id shown \
     in brackets ahead of the violating message. Do not call this tool again this turn; \
     answer from what you already have.";

/// The self-report refusal: the named message resolves to the assistant's
/// own voice. Reachable in principle — the model could name an id it was
/// never shown — and refused structurally: the assistant does not report
/// itself.
pub const SELF_REPORT_ERROR: &str = "declined: the named message is the assistant's own, \
     and the assistant does not report itself. Do not call this tool again this turn; \
     answer from what you already have.";

/// The direct-conversation refusal: reports belong to groups, where the
/// moderation bot and its administrators are.
pub const GROUP_ONLY_ERROR: &str = "declined: reports are filed in group conversations \
     only. Do not call this tool again this turn; answer from what you already have.";

/// The unrecorded-target refusal: the named message resolves to no
/// recorded principal — a row the store did not produce whole — so no
/// report can name a person erasure could later reach, and filing one
/// would ship an identifier out of erasure's reach (the exact gap decision
/// 0003 exists to prevent).
pub const UNRECORDED_TARGET_ERROR: &str = "declined: the named message is not in the \
     assistant's records, so no report can name it. Do not call this tool again this \
     turn; answer from what you already have.";

/// The transient failure: a read or the append did not stand, so nothing
/// was filed. No no-retry line — the fact may not hold beyond this
/// failure, and the per-origin dedup finds nothing filed, so a later
/// turn's assessment files cleanly.
fn transient_error() -> String {
    "the report could not be filed right now; nothing was filed.".to_owned()
}

/// The fixed report line for one configured moderation handle.
#[must_use]
pub fn report_line(handle: &str) -> String {
    format!("{REPORT_LINE_LEAD}{handle}")
}

/// The named origin of one call's input: the trimmed, non-empty
/// `message_id` string, `None` for a missing field, a non-string, an
/// empty id, or input that is not a JSON object — every unusable shape is
/// one refusal, taught by [`NEEDS_TARGET_ERROR`].
fn named_origin(input: &str) -> Option<String> {
    let value: Value = serde_json::from_str(input).ok()?;
    let origin = value.get(PARAMETER_MESSAGE_ID)?.as_str()?.trim();
    (!origin.is_empty()).then(|| origin.to_owned())
}

/// The pure target resolution over one loaded ledger, in the order of the
/// claims: the named origin must belong to the turn's co-summoner set —
/// the messages the model is assessing this turn, the anti-aiming bound;
/// the resolved row must not be the assistant's own voice (the assistant
/// holds no principal row, so "the assistant's own" is the stored role
/// fact — no ingestion writes an assistant-voiced chat row today, and the
/// refusal covers the in-principle shapes structurally); the row must
/// carry a recorded principal, because a report erasure could never reach
/// must not exist (the schema stores the column NOT NULL, so this absence
/// is a row the store did not produce whole — refused all the same); and
/// no report of the origin may already stand, the per-origin dedup that
/// bounds the die-after-filing re-summon path. `Ok` is the reported
/// principal; `Err` is the decline the model reads.
fn resolve_reportable(
    ledger: &[Block],
    call_block_id: i64,
    origin: &str,
) -> Result<i64, &'static str> {
    let Some(target) = co_summoners(ledger, call_block_id)
        .into_iter()
        .find(|message| message.origin.as_deref() == Some(origin))
    else {
        return Err(NOT_ASSESSED_ERROR);
    };
    if target.role == Some(Role::Assistant) {
        return Err(SELF_REPORT_ERROR);
    }
    let Some(reported_principal_id) = target.principal_id else {
        return Err(UNRECORDED_TARGET_ERROR);
    };
    let already = ledger
        .iter()
        .any(|block| match AssistantKind::from_block(block) {
            AssistantKind::Report(report) => report.target_origin.as_deref() == Some(origin),
            _ => false,
        });
    if already {
        return Err(ALREADY_REPORTED_ERROR);
    }
    Ok(reported_principal_id)
}

/// The report tool: member authority, group conversations only, one
/// validated parameter. Constructed by the assembly when a moderation
/// handle is configured under helpful answering — the erasure fence is
/// injected here, at registration, so the tool never reaches into the
/// assembly.
pub(crate) struct ReportTool {
    /// The configured moderation handle, already trimmed with its leading
    /// `@` stripped by the configuration layer.
    handle: String,
    /// The erasure fence, held shared across the resolution and the append
    /// so a report cannot re-materialize an origin an erasure just nulled.
    /// Taken as the bare shared lock, not as the assembly's own alias for
    /// it — a leaf tool names nothing in the module that registers it.
    fence: Arc<RwLock<()>>,
    /// The filing lock: one scan-then-append at a time. The erasure fence
    /// is SHARED between filings — it excludes erasure, not a sibling
    /// call — and the runner executes same-round calls in parallel tasks,
    /// so without this lock two calls naming one origin both scan before
    /// either appends and the per-origin dedup double-files.
    filing: tokio::sync::Mutex<()>,
}

impl ReportTool {
    pub(crate) fn new(handle: impl Into<String>, fence: Arc<RwLock<()>>) -> Self {
        Self {
            handle: handle.into(),
            fence,
            filing: tokio::sync::Mutex::new(()),
        }
    }

    /// The whole filing, under the erasure fence. `Err` carries the tool
    /// error the runner records and the model reads. The order of the
    /// checks is the order of the claims: the conversation can carry a
    /// report at all; the named origin is one the model is assessing this
    /// turn; the named message resolves to a recorded person who is not
    /// the assistant; and no report of it stands yet.
    async fn file(
        &self,
        ctx: &ToolContext<'_, CoreEvent>,
        origin: &str,
    ) -> Result<&'static str, String> {
        let _one_filing_at_a_time = self.filing.lock().await;
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
        // The validation of decision 2026-08-24, whole: the co-summoner
        // bound, the voice and record guards, and the per-origin dedup —
        // one pure resolution over the loaded vector, its order and its
        // reasoning on [`resolve_reportable`].
        let reported_principal_id = match resolve_reportable(&ledger, ctx.block_id, origin) {
            Ok(principal) => principal,
            Err(decline) => return Err(decline.to_owned()),
        };
        let appended = ctx
            .agency
            .store
            .append_consumer_block(
                conversation_id,
                None,
                REPORT_KIND,
                Report::stored_fields(origin, reported_principal_id, &report_line(&self.handle)),
                None,
            )
            .await;
        if let Err(error) = appended {
            tracing::warn!(conversation_id, %error, "the report's append failed; nothing filed");
            return Err(transient_error());
        }
        Ok(FILED_RESULT)
    }
}

impl ToolHandler<CoreEvent> for ReportTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "File a report with the group's moderation bot for a message that \
                 violates the group's pinned rules. Name the violating message by its id, \
                 shown in brackets ahead of the message; it must be a message you are \
                 assessing in this turn. The report is an assessment only — the group's \
                 administrators decide. This tool is the only way to report — never write \
                 the report command into an answer yourself."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    PARAMETER_MESSAGE_ID: {
                        "type": "string",
                        "description": "The violating message's id, exactly as shown in \
                             brackets ahead of the message"
                    }
                },
                "required": [PARAMETER_MESSAGE_ID]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            let Some(origin) = named_origin(input) else {
                return ToolOutcome::Error(NEEDS_TARGET_ERROR.to_owned());
            };
            match self.file(&ctx, &origin).await {
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

    /// The parameter reading, every unusable shape refused as one absence:
    /// the well-formed call answers its trimmed id, and a missing field, a
    /// non-string, an empty id, extra whitespace only, or non-JSON input
    /// each resolve to `None` — the needs-a-target refusal's whole domain.
    #[test]
    fn the_named_origin_reads_the_message_id_and_refuses_every_unusable_shape() {
        assert_eq!(
            named_origin(r#"{"message_id":"origin-9"}"#).as_deref(),
            Some("origin-9")
        );
        assert_eq!(
            named_origin(r#"{"message_id":"  origin-9  "}"#).as_deref(),
            Some("origin-9"),
            "the id is trimmed"
        );
        for unusable in [
            "{}",
            r#"{"message_id":""}"#,
            r#"{"message_id":"   "}"#,
            r#"{"message_id":7}"#,
            r#"{"target":"origin-9"}"#,
            "not json",
            "",
        ] {
            assert_eq!(named_origin(unusable), None, "refused: {unusable:?}");
        }
    }

    /// The exact copy of every fixed result, pinned verbatim: the filed
    /// result claims filing with this turn — never arrival — and every
    /// decline closes with the admission wrapper's no-retry teaching,
    /// while the transient failure names the moment and teaches nothing.
    #[test]
    fn the_result_wording_is_pinned_verbatim() {
        assert_eq!(
            FILED_RESULT,
            "The report is filed and goes out with this turn, as a reply to the reported \
             message. Do not report this message again."
        );
        assert_eq!(
            NOT_ASSESSED_ERROR,
            "declined: that message is not one you are assessing this turn. Do not call \
             this tool again this turn; answer from what you already have."
        );
        assert_eq!(
            ALREADY_REPORTED_ERROR,
            "declined: that message is already reported, and a message is reported at most \
             once. Do not call this tool again this turn; answer from what you already \
             have."
        );
        assert_eq!(
            NEEDS_TARGET_ERROR,
            "declined: a report names its target — the id shown in brackets ahead of the \
             violating message. Do not call this tool again this turn; answer from what \
             you already have."
        );
        assert_eq!(
            SELF_REPORT_ERROR,
            "declined: the named message is the assistant's own, and the assistant does \
             not report itself. Do not call this tool again this turn; answer from what \
             you already have."
        );
        assert_eq!(
            GROUP_ONLY_ERROR,
            "declined: reports are filed in group conversations only. Do not call this \
             tool again this turn; answer from what you already have."
        );
        assert_eq!(
            UNRECORDED_TARGET_ERROR,
            "declined: the named message is not in the assistant's records, so no report \
             can name it. Do not call this tool again this turn; answer from what you \
             already have."
        );
        for closes_with_no_retry in [
            NOT_ASSESSED_ERROR,
            ALREADY_REPORTED_ERROR,
            NEEDS_TARGET_ERROR,
            SELF_REPORT_ERROR,
            GROUP_ONLY_ERROR,
            UNRECORDED_TARGET_ERROR,
        ] {
            assert!(
                closes_with_no_retry.ends_with(NO_RETRY),
                "every decline closes with the no-retry teaching: {closes_with_no_retry}"
            );
        }
        assert!(
            !FILED_RESULT.contains(NO_RETRY),
            "the filed result forbids repeating THIS report, not the tool: a turn \
             assessing several violators files each through its own call"
        );
        let transient = transient_error();
        assert!(
            transient.contains("right now") && !transient.contains(NO_RETRY),
            "a transient fact names the moment and teaches no never-again: {transient}"
        );
    }

    /// One synthetic chat row for the resolution pins: origin, voice,
    /// principal and the summoned stamp, everything else absent — the
    /// leanest shape the parse accepts.
    fn chat_row(
        id: i64,
        role: Role,
        origin: &str,
        principal: Option<i64>,
        addressed: bool,
    ) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("text".into(), json!("a recorded line"));
        fields.insert("origin".into(), json!(origin));
        fields.insert("authority".into(), json!("member"));
        fields.insert("addressed".into(), json!(addressed));
        fields.insert("answer_due".into(), json!(addressed));
        if let Some(principal) = principal {
            fields.insert("principal_id".into(), json!(principal));
        }
        Block {
            id,
            role: Some(role),
            block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// One loaded call block anchored on the given id.
    fn call_row(id: i64, anchor: i64) -> Block {
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_call".into(),
            created_at: String::new(),
            dispatch_anchor: Some(anchor),
            fields: serde_json::Map::new(),
        }
    }

    /// The pure resolution, every claim in its order: a named co-summoner
    /// resolves to its principal; an origin outside the set — a bystander's
    /// included — declines as not-assessed; the assistant's own voice
    /// declines as the self-report; a row without a recorded principal
    /// (a shape the schema's NOT NULL keeps out of every stored ledger, so
    /// it is pinned here at the pure seam where it is reachable) declines
    /// as unrecorded; and a standing report of the origin declines the
    /// repeat.
    #[test]
    fn the_resolution_validates_the_set_the_voice_the_record_and_the_dedup() {
        let mut ledger = vec![
            chat_row(1, Role::User, "origin-bystander", Some(3), false),
            chat_row(2, Role::User, "origin-anchor", Some(5), true),
            chat_row(3, Role::User, "origin-violator", Some(7), true),
            chat_row(4, Role::Assistant, "origin-self", Some(8), true),
            chat_row(5, Role::User, "origin-broken", None, true),
            call_row(9, 2),
        ];
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-violator"),
            Ok(7),
            "the named co-summoner resolves to its recorded principal"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-anchor"),
            Ok(5),
            "the anchor's own message is in the assessment set too"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-nobody"),
            Err(NOT_ASSESSED_ERROR),
            "an arbitrary id is not one the turn is assessing"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-bystander"),
            Err(NOT_ASSESSED_ERROR),
            "a bystander's line co-summons nothing and cannot be aimed at"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-self"),
            Err(SELF_REPORT_ERROR),
            "the assistant's own voice is refused before its principal is read"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-broken"),
            Err(UNRECORDED_TARGET_ERROR),
            "a row without a recorded principal names nobody erasure can reach"
        );

        ledger.insert(
            5,
            Block {
                id: 6,
                role: None,
                block_type: REPORT_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: Some(2),
                fields: Report::stored_fields("origin-violator", 7, "/report@m"),
            },
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-violator"),
            Err(ALREADY_REPORTED_ERROR),
            "a standing report of the origin declines the repeat"
        );
        assert_eq!(
            resolve_reportable(&ledger, 9, "origin-anchor"),
            Ok(5),
            "the dedup is per origin: a distinct violation still files"
        );
    }

    /// The description teaches the validated shape: the id parameter, the
    /// assessment bound, the administrators' decision, and the only-way
    /// rule.
    #[test]
    fn the_definition_teaches_the_id_the_bound_and_the_decision() {
        let definition = ReportTool::new("moderation_bot", Arc::new(RwLock::new(()))).definition();
        assert_eq!(definition.name, NAME);
        for fact in [
            "by its id",
            "a message you are assessing in this turn",
            "administrators decide",
            "never write the report command into an answer yourself",
        ] {
            assert!(
                definition.description.contains(fact),
                "the description carries: {fact}"
            );
        }
        let required = definition.parameters["required"]
            .as_array()
            .expect("the schema names its required list");
        assert_eq!(required, &[json!(PARAMETER_MESSAGE_ID)]);
    }
}
