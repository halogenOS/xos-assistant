//! The privacy tool: plain language reaching the same rights mechanisms the
//! command family serves, through one enforced surface (decided
//! 2026-08-23). The model reads a member's ask — "stop collecting my
//! messages", "delete my data" — and calls this tool; the system, not the
//! model, performs the change. The module is named for the rights it
//! reaches (renamed 2026-08-23): the crate's `privacy` module is the
//! command family's, and two modules answering to one name read as one
//! at every import.
//!
//! The subject is never a parameter and never a guess. The tool acts on the
//! turn's origin set resolved to PRINCIPALS: the own-debt-takers of the
//! debt origin walk — the same walk the report tool's target resolution
//! rides — mapped to their stored principal ids. Exactly one distinct
//! principal in the set: the tool acts on it. Several (the absorbed
//! co-summoner shape), none, or an erased row whose principal no longer
//! resolves: the tool declines with the fixed ambiguity result naming the
//! commands, because acting on a guessed person is the one failure this
//! design must never have — the commands are always unambiguous.
//!
//! Two actions. `opt_out` raises the suppression flag through the identity
//! module's own write, under the erasure fence held for reading — this
//! crosses unit 5's no-write rule under its dated second clause: a tool may
//! write the consumer's own identity-table fact when the write IS the
//! honored right. `request_deletion` files the same principal-keyed pending
//! state the `/privacydelete` command files and returns the fixed result
//! carrying the literal confirm token for the model to relay verbatim; the
//! prompt orders the relay, and a model garbling it costs one retry via the
//! command path — the stated residual. Any other or absent action answers
//! the fixed invalid-action result.
//!
//! The tool's deterministic replies share the per-person reply bound with
//! the command family: an exhausted window withholds the state change with
//! the reply — never a silent change — and answers the transient result,
//! which is also what a failed write answers: nothing was recorded, and the
//! commands remain the direct path.

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::identity;
use crate::message::Authority;
use crate::privacy::{CONFIRM_INSTRUCTION, OPT_OUT_DONE, PendingDeletions};
use crate::tools::provenance::sole_principal;
use crate::window::ReplyWindow;

/// The registered name the model calls the tool by.
pub const NAME: &str = "privacy_request";

/// The authority this tool requires — member: the rights it reaches are
/// every member's own. The admission wrapper supplies no extra protection
/// at this bar; the tool sits under it because every tool does.
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The action that raises the suppression flag.
pub const ACTION_OPT_OUT: &str = "opt_out";

/// The action that files the pending deletion confirmation.
pub const ACTION_REQUEST_DELETION: &str = "request_deletion";

/// The ambiguity decline: the origin set resolves to no single person, so
/// nothing is acted on and the commands — always unambiguous — are named.
pub const AMBIGUOUS_RESULT: &str = "Several people spoke in this turn, so the request is \
     not acted on. The person concerned should send /privacyout or /privacydelete \
     themselves.";

/// The invalid-action decline: the action vocabulary is closed, and the
/// wording itself stops a rewording loop.
pub const INVALID_ACTION_RESULT: &str = "The privacy tool accepts opt_out or \
     request_deletion. Nothing was changed. Do not retry with other words.";

/// The transient decline: a read or the write did not stand — or the
/// person's own reply window is exhausted — so nothing changed and nothing
/// was recorded; the commands remain the direct path.
pub const TRANSIENT_RESULT: &str = "The change did not take effect. Nothing was recorded. \
     The person can use /privacyout or /privacydelete directly.";

/// What every relayed result opens with before the fixed line the person
/// must read: the model is ordered to pass the copy through untouched.
const RELAY_LEAD: &str = "Relay this to the person verbatim: ";

/// The opt-out's filed result: the flag stands, and the fixed opt-out line
/// is handed to the model for the verbatim relay.
#[must_use]
pub fn opt_out_result() -> String {
    format!("The opt-out is recorded. {RELAY_LEAD}{OPT_OUT_DONE}")
}

/// The deletion request's filed result: the pending stands, and the confirm
/// instruction — carrying the literal confirm token — is handed to the
/// model for the verbatim relay.
#[must_use]
pub fn request_deletion_result() -> String {
    format!("The deletion request is filed. {RELAY_LEAD}{CONFIRM_INSTRUCTION}")
}

/// The privacy tool: member authority, no target parameter, one closed
/// action vocabulary. Constructed by the assembly, which injects the shared
/// pending-deletion memory, the shared per-person reply bound and the
/// erasure fence at registration, so the tool never reaches into the
/// assembly and the command path and this tool act on one state.
pub(crate) struct PrivacyTool {
    /// The pending confirmations `/confirmdelete` consumes — the very
    /// memory the command path files into.
    pending: Arc<PendingDeletions>,
    /// The per-person reply bound shared with the command family.
    window: Arc<ReplyWindow>,
    /// The erasure fence, held shared across the resolution and the flag
    /// write, so the tool cannot re-raise facts an erasure is removing.
    /// Taken as the bare shared lock, not as the assembly's own alias for
    /// it — a leaf tool names nothing in the module that registers it.
    fence: Arc<RwLock<()>>,
}

impl PrivacyTool {
    pub(crate) fn new(
        pending: Arc<PendingDeletions>,
        window: Arc<ReplyWindow>,
        fence: Arc<RwLock<()>>,
    ) -> Self {
        Self {
            pending,
            window,
            fence,
        }
    }

    /// The whole request, under the erasure fence. `Err` carries the fixed
    /// decline the runner records and the model reads.
    async fn act(
        &self,
        action: Action,
        ctx: &ToolContext<'_, CoreEvent>,
    ) -> Result<String, String> {
        let _no_erasure_mid_request = self.fence.read().await;
        let conversation_id = ctx.agency.conversation_id;
        let ledger = match ctx.agency.store.list_blocks(conversation_id).await {
            Ok(ledger) => ledger,
            Err(error) => {
                tracing::warn!(conversation_id, %error, "the privacy tool's ledger read failed");
                return Err(TRANSIENT_RESULT.to_owned());
            }
        };
        // The subject: the one person behind the turn, resolved once in the
        // provenance reading. An unresolvable set — none, several, or a
        // taker whose stored principal is unreadable — is this tool's
        // ambiguity, never a smaller set to act on.
        let Some(principal_id) = sole_principal(&ledger, ctx.block_id) else {
            return Err(AMBIGUOUS_RESULT.to_owned());
        };
        // The person must still resolve: an erased row's principal id names
        // nobody the identity tables know, and acting on it would raise a
        // flag no lookup ever finds.
        let tx = ctx.agency.store.tx();
        match identity::exists(&tx, principal_id).await {
            Ok(true) => {}
            Ok(false) => return Err(AMBIGUOUS_RESULT.to_owned()),
            Err(error) => {
                tracing::warn!(conversation_id, %error, "the privacy tool's identity read failed");
                return Err(TRANSIENT_RESULT.to_owned());
            }
        }
        // The per-person bound, shared with the command family, through its
        // one grant-with-the-change operation: the state change applies
        // exactly when its reply is granted, never silently, and a failed
        // write hands the grant back before the transient decline.
        let change = async {
            match action {
                // The flag stands either way — freshly raised or already so
                // — and the relayed line is the same honored right.
                Action::OptOut => identity::set_opt_out(&tx, principal_id)
                    .await
                    .map(|_| opt_out_result()),
                Action::RequestDeletion => {
                    self.pending.file(principal_id).await;
                    Ok(request_deletion_result())
                }
            }
        };
        match self.window.grant_with(principal_id, change).await {
            Some(Ok(filed)) => Ok(filed),
            Some(Err(error)) => {
                tracing::warn!(
                    conversation_id,
                    principal_id,
                    %error,
                    "the privacy tool's flag write failed; nothing recorded"
                );
                Err(TRANSIENT_RESULT.to_owned())
            }
            // The exhausted window: the change never ran.
            None => Err(TRANSIENT_RESULT.to_owned()),
        }
    }
}

/// The closed action vocabulary, parsed from the call's input.
enum Action {
    OptOut,
    RequestDeletion,
}

/// The action a call's input names, `None` for anything outside the closed
/// vocabulary — an unparsable input, an absent field and a stranger string
/// alike.
fn parse_action(input: &str) -> Option<Action> {
    let parsed: Value = serde_json::from_str(input).ok()?;
    match parsed.get("action")?.as_str()? {
        ACTION_OPT_OUT => Some(Action::OptOut),
        ACTION_REQUEST_DELETION => Some(Action::RequestDeletion),
        _ => None,
    }
}

impl ToolHandler<CoreEvent> for PrivacyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "Honor a member's own privacy ask, stated in their own words: \
                 action opt_out stops collecting and answering their messages on this \
                 platform, action request_deletion starts the deletion of their stored \
                 data. Use it only when the person asks for themselves; the tool acts on \
                 whoever asked, takes no target, and declines when several people spoke. \
                 Relay the result's quoted text to the person verbatim."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [ACTION_OPT_OUT, ACTION_REQUEST_DELETION],
                        "description": "What the person asked for."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            // The action is judged before anything is read: an invalid ask
            // deserves its own teaching, not an ambiguity verdict.
            let Some(action) = parse_action(input) else {
                return ToolOutcome::Error(INVALID_ACTION_RESULT.to_owned());
            };
            match self.act(action, &ctx).await {
                Ok(filed) => ToolOutcome::Done(filed),
                Err(decline) => ToolOutcome::Error(decline),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;
    use crate::schema::store_config;
    use crate::window::{PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW};

    /// One tool over fresh shared state — the shape the assembly builds.
    fn fixture_tool() -> PrivacyTool {
        PrivacyTool::new(
            Arc::new(PendingDeletions::new()),
            Arc::new(ReplyWindow::new(PRIVACY_REPLY_WINDOW, PRIVACY_REPLY_CAP)),
            Arc::new(RwLock::new(())),
        )
    }

    /// One conversation on a fresh in-memory store, as an agency context.
    async fn fixture_agency() -> AgencyCtx<CoreEvent> {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let conversation_id = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        AgencyCtx {
            conversation_id,
            store,
            bus: Arc::new(EventBus::new()),
        }
    }

    /// An invalid and an absent action each answer the fixed
    /// invalid-action result, judged before anything is read.
    #[tokio::test]
    async fn an_invalid_or_absent_action_answers_the_fixed_result() {
        let agency = fixture_agency().await;
        let tool = fixture_tool();
        for input in [r#"{"action":"delete_everything"}"#, "{}", ""] {
            let outcome = tool
                .execute(
                    input,
                    ToolContext {
                        agency: &agency,
                        tool_call_id: "call-0",
                        block_id: 0,
                    },
                )
                .await;
            match outcome {
                ToolOutcome::Error(decline) => assert_eq!(
                    decline, INVALID_ACTION_RESULT,
                    "the input {input:?} answers the invalid-action result"
                ),
                ToolOutcome::Done(_) | ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                    panic!("an invalid action never acts")
                }
            }
        }
    }

    /// The none shape: a call the loaded vector does not hold resolves an
    /// empty origin set, and an empty set is the same ambiguity decline as
    /// a crowded one — never a person to act on.
    #[tokio::test]
    async fn an_unresolvable_origin_set_declines_with_the_ambiguity_result() {
        let agency = fixture_agency().await;
        let tool = fixture_tool();
        let outcome = tool
            .execute(
                r#"{"action":"opt_out"}"#,
                ToolContext {
                    agency: &agency,
                    tool_call_id: "call-0",
                    block_id: 12345,
                },
            )
            .await;
        match outcome {
            ToolOutcome::Error(decline) => assert_eq!(decline, AMBIGUOUS_RESULT),
            ToolOutcome::Done(_) | ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                panic!("an empty origin set never acts")
            }
        }
    }

    /// The palette governs this tool like every other: wrapped in the
    /// admission check, a conversation whose recorded palette does not
    /// name it declines before the body runs.
    #[tokio::test]
    async fn the_palette_governs_the_tool() {
        let agency = fixture_agency().await;
        agency
            .store
            .append_consumer_block(
                agency.conversation_id,
                None,
                crate::tools::palette::TOOL_PALETTE_KIND,
                crate::tools::palette::ToolPalette::stored_fields(&["lookup_commit".into()]),
                None,
            )
            .await
            .expect("the palette block appends");
        let admitted =
            crate::tools::admission::AdmittedTool::new(REQUIRED_AUTHORITY, fixture_tool());
        let outcome = admitted
            .execute(
                r#"{"action":"opt_out"}"#,
                ToolContext {
                    agency: &agency,
                    tool_call_id: "call-0",
                    block_id: 0,
                },
            )
            .await;
        match outcome {
            ToolOutcome::Error(decline) => assert!(
                decline.contains(NAME)
                    && decline.contains("is not in this conversation's tool palette"),
                "the unnamed tool draws the palette decline: {decline}"
            ),
            ToolOutcome::Done(_) | ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                panic!("a palette that does not name the tool admits nothing")
            }
        }
    }

    /// The exact copy of every fixed result, pinned verbatim against the
    /// unit spec: the two declines that hold, the transient one, and the
    /// relayed results carrying the fixed lines — the deletion result with
    /// the literal confirm token inside.
    #[test]
    fn the_result_wording_is_pinned_verbatim() {
        assert_eq!(
            AMBIGUOUS_RESULT,
            "Several people spoke in this turn, so the request is not acted on. The person \
             concerned should send /privacyout or /privacydelete themselves."
        );
        assert_eq!(
            INVALID_ACTION_RESULT,
            "The privacy tool accepts opt_out or request_deletion. Nothing was changed. Do \
             not retry with other words."
        );
        assert_eq!(
            TRANSIENT_RESULT,
            "The change did not take effect. Nothing was recorded. The person can use \
             /privacyout or /privacydelete directly."
        );
        assert!(
            opt_out_result().ends_with(OPT_OUT_DONE),
            "the opt-out result relays the fixed opt-out line"
        );
        assert!(
            request_deletion_result().ends_with(CONFIRM_INSTRUCTION),
            "the deletion result relays the confirm instruction"
        );
        assert!(
            request_deletion_result().contains(crate::privacy::CONFIRM_COMMAND),
            "the pinned fact: the literal confirm token rides in the tool result"
        );
    }

    /// The closed action vocabulary: the two named actions parse, and every
    /// other shape — a stranger action, an absent field, an unparsable
    /// input — is the invalid-action answer's `None`.
    #[test]
    fn the_action_vocabulary_is_closed() {
        assert!(matches!(
            parse_action(r#"{"action":"opt_out"}"#),
            Some(Action::OptOut)
        ));
        assert!(matches!(
            parse_action(r#"{"action":"request_deletion"}"#),
            Some(Action::RequestDeletion)
        ));
        assert!(parse_action(r#"{"action":"delete_everything"}"#).is_none());
        assert!(parse_action("{}").is_none());
        assert!(parse_action("not json").is_none());
        assert!(parse_action(r#"{"action":7}"#).is_none());
    }
}
