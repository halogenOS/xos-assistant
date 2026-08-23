//! The tools unit at the core's edges: the three lookups against scripted
//! wires — the wiki lookup under its own section (AC2) — the palette
//! supersession that folds a pre-unit conversation into the registered
//! set, the anchor gate over the turn's provenance (AC5, lifted with the
//! dispatch anchor), the tail-only stamp under mid-turn absorption (AC6),
//! the budget composition (AC8), and the redispatch canary over the
//! closed duplicate-turn window. The no-palette refusal is pinned
//! behaviorally in the admission wrapper's own tests, beside its strings.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{Block, CoreEvent, EventBus, Store, ToolContext, ToolHandler, ToolOutcome};
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::commit::{self, CommitLookup};
use assistant_core::tools::release::{self, ReleaseLookup};
use assistant_core::tools::wiki::{self, WikiLookup};
use assistant_core::{Assistant, Authority, ChannelKind, ProtectionConfig};
use serde_json::{Value, json};

use crate::lookup_wire::{LookupAnswer, LookupServer};
use crate::support::{
    self, CLOSING_ANSWER, Round, ToolScript, carries, channel, field, inbound, inbound_as,
    inbound_unaddressed, recv_reply, round_scripted_provider, settle_shape, tool_scripted_provider,
};

/// A commit-lookup input the scripts send — non-empty by the script's
/// contract.
const COMMIT_INPUT: &str = r#"{"repository":"android_manifest","reference":"deadbeef"}"#;

/// The scripted forge's commit answer, in the Forgejo v1 shape.
fn forge_commit_body() -> Value {
    json!({
        "sha": "deadbeef00112233445566778899aabbccddeeff",
        "html_url": "https://example.invalid/commit/deadbeef",
        "commit": {
            "message": "Track the manifest\n\nBody prose the compact form drops.\n",
            "author": { "name": "A. Committer", "date": "2026-08-17T01:23:26+02:00" }
        }
    })
}

/// The compact result the forge answer above decodes to.
fn forge_compact_result() -> String {
    "Commit deadbeef0011 in halogenOS/android_manifest\n\
     Subject: Track the manifest\n\
     Author: A. Committer\n\
     Date: 2026-08-17T01:23:26+02:00\n\
     Link: https://example.invalid/commit/deadbeef"
        .to_owned()
}

/// A probe tool: records whether its body ran, so a decline can prove the
/// body never did.
struct ProbeTool {
    name: &'static str,
    executed: Arc<AtomicBool>,
}

impl ProbeTool {
    fn new(name: &'static str) -> (Self, Arc<AtomicBool>) {
        let executed = Arc::new(AtomicBool::new(false));
        (
            Self {
                name,
                executed: Arc::clone(&executed),
            },
            executed,
        )
    }
}

impl ToolHandler<CoreEvent> for ProbeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.into(),
            description: "a probe that records having run".into(),
            parameters: json!({ "type": "object" }),
        }
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async {
            self.executed.store(true, Ordering::SeqCst);
            ToolOutcome::Done("the probe ran".into())
        })
    }
}

/// The outbound edge a fixture's replies arrive on.
type Replies = tokio::sync::mpsc::UnboundedReceiver<assistant_core::OutboundReply>;

/// One assembled tool fixture: the assistant over the tool-scripted
/// provider and the given set, under the default budgets, plus the
/// outbound edge.
async fn tool_fixture(
    script: ToolScript,
    hold: Option<Arc<support::TurnHold>>,
    tools: ToolSet,
) -> (support::Fixture, Replies) {
    tool_fixture_configured(script, hold, tools, ProtectionConfig::default()).await
}

/// The tool fixture with the budgets spelled out, for the tests that pin
/// how protection and tools compose.
async fn tool_fixture_configured(
    script: ToolScript,
    hold: Option<Arc<support::TurnHold>>,
    tools: ToolSet,
    protection: ProtectionConfig,
) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (provider, handle) = tool_scripted_provider(script, hold);
    assemble(store, provider, handle, tools, protection).await
}

/// One assembled fixture over the round-scripted provider, under the
/// default budgets.
async fn round_fixture(
    rounds: Vec<Round>,
    hold: Arc<support::TurnHold>,
    tools: ToolSet,
) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (provider, handle) = round_scripted_provider(rounds, hold);
    assemble(store, provider, handle, tools, ProtectionConfig::default()).await
}

/// The one home of the assembly preamble every fixture shares: start the
/// assistant over the given store and provider, and open the outbound
/// edge.
async fn assemble(
    store: Store,
    provider: Box<dyn agent_ledger::ProviderModule>,
    handle: support::ScriptHandle,
    tools: ToolSet,
    protection: ProtectionConfig,
) -> (support::Fixture, Replies) {
    let fixture = support::start_assistant_full(store, provider, handle, tools, protection).await;
    let replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    (fixture, replies)
}

// ─── AC3: each tool pinned — happy path, error status, timeout ───────────

/// The commit lookup's happy path: the scripted forge's answer is decoded
/// to the compact result, the result block carries it, and the chat
/// receives only the model's closing answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_commit_lookup_decodes_the_forge_answer() {
    let forge = LookupServer::start(LookupAnswer::Json(200, forge_commit_body())).await;
    let mut tools = ToolSet::new();
    tools.admit(
        commit::REQUIRED_AUTHORITY,
        CommitLookup::new(forge.base(), commit::DEFAULT_TIMEOUT),
    );
    let script = ToolScript {
        tool: commit::NAME.into(),
        input: COMMIT_INPUT.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-commit"),
            ChannelKind::Direct,
            "42",
            "what changed?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the tool turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;

    assert_eq!(field(&blocks[3], "name"), commit::NAME);
    assert_eq!(field(&blocks[3], "input"), COMMIT_INPUT);
    assert_eq!(field(&blocks[4], "content"), forge_compact_result());
    // Read by tail here: the settle can land on either side of the edge's
    // stored disclosure prepend; the delivered reply below pins the exact
    // text once the prepend has provably happened.
    assert!(
        field(&blocks[5], "content").ends_with(CLOSING_ANSWER),
        "the stored closing text is the model's answer"
    );

    // The wire: one GET, at the dialect's path.
    let requests = forge.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/api/v1/repos/halogenOS/android_manifest/git/commits/deadbeef"
    );

    // The result reached the model's second request, verbatim.
    {
        use agent_ledger::providers::{ContentPart as WirePart, MessageContent};
        let requests = fixture.script.seen.lock().unwrap();
        assert_eq!(requests.len(), 2, "one turn: the call, then the close");
        assert!(
            requests[1].iter().any(|message| matches!(
                &message.content,
                MessageContent::Parts(parts) if parts.iter().any(|part| matches!(
                    part,
                    WirePart::ToolResult { content, .. } if *content == forge_compact_result()
                ))
            )),
            "the second request carries the tool result: {:?}",
            requests[1]
        );
    }

    // The chat receives the model's answer alone — never the tool result.
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, assistant_core::disclosed(CLOSING_ANSWER));
    let extra = replies.try_recv();
    assert!(extra.is_err(), "one turn, one chat answer; got {extra:?}");
}

/// The release lookup's happy path with a configured token: the tag path is
/// asked, the authorization header carries the token, and the compact
/// result summarizes the assets.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_release_lookup_decodes_the_mirror_answer_and_sends_the_token() {
    let mirror = LookupServer::start(LookupAnswer::Json(
        200,
        json!({
            "tag_name": "20260707.2230.36-rb",
            "name": "[release build] XOS-16.2",
            "published_at": "2026-07-07T20:43:15Z",
            "html_url": "https://example.invalid/releases/tag/20260707.2230.36-rb",
            "assets": [
                { "name": "boot.img", "size": 4096 },
                { "name": "halogenOS_Device-16.2.zip", "size": 8192 }
            ]
        }),
    ))
    .await;
    let mut tools = ToolSet::new();
    tools.admit(
        release::REQUIRED_AUTHORITY,
        ReleaseLookup::new(
            mirror.base(),
            Some("FAKE-MIRROR-TOKEN".into()),
            release::DEFAULT_TIMEOUT,
        ),
    );
    let script = ToolScript {
        tool: release::NAME.into(),
        input: r#"{"tag":"20260707.2230.36-rb"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-release"),
            ChannelKind::Direct,
            "42",
            "what is the latest build?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the tool turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;

    let compact = field(&blocks[4], "content");
    assert_eq!(
        compact,
        "Release 20260707.2230.36-rb — [release build] XOS-16.2\n\
         Published: 2026-07-07T20:43:15Z\n\
         Link: https://example.invalid/releases/tag/20260707.2230.36-rb\n\
         Assets: 2\n\
         - boot.img (4096 bytes)\n\
         - halogenOS_Device-16.2.zip (8192 bytes)"
    );

    let requests = mirror.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/repos/halogenOS/builds/releases/tags/20260707.2230.36-rb"
    );
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer FAKE-MIRROR-TOKEN"),
        "the configured token flows to the mirror's authorization header"
    );
    assert!(
        requests[0].header("user-agent").is_some(),
        "the mirror requires a user agent"
    );

    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// AC7's absent-token half, and the default tag: no `tag` in the input asks
/// the latest-release path, and no configured token sends no authorization
/// header at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absent_token_sends_no_header_and_the_default_is_the_latest_release() {
    let mirror = LookupServer::start(LookupAnswer::Json(
        200,
        json!({
            "tag_name": "t",
            "published_at": "d",
            "html_url": "l",
            "assets": []
        }),
    ))
    .await;
    let mut tools = ToolSet::new();
    tools.admit(
        release::REQUIRED_AUTHORITY,
        ReleaseLookup::new(mirror.base(), None, release::DEFAULT_TIMEOUT),
    );
    let script = ToolScript {
        tool: release::NAME.into(),
        input: "{\"tag\":null}".into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&channel("dm-latest"), ChannelKind::Direct, "42", "latest?"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );

    let requests = mirror.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/repos/halogenOS/builds/releases/latest");
    assert_eq!(
        requests[0].header("authorization"),
        None,
        "an absent token sends no header"
    );
}

/// One lookup constructed over a scripted server and a timeout — what lets
/// the failure loop below run the same two cases against each tool.
fn commit_tools(base: String, timeout: Duration) -> ToolSet {
    let mut tools = ToolSet::new();
    tools.admit(commit::REQUIRED_AUTHORITY, CommitLookup::new(base, timeout));
    tools
}

fn release_tools(base: String, timeout: Duration) -> ToolSet {
    let mut tools = ToolSet::new();
    tools.admit(
        release::REQUIRED_AUTHORITY,
        ReleaseLookup::new(base, None, timeout),
    );
    tools
}

/// AC3's failure paths, for each tool: an error status and a timeout under
/// a short constructed bound each resolve the call with a tool error the
/// model sees, while the chat receives only the model's closing answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_error_status_and_a_timeout_become_tool_errors_the_model_sees() {
    type ToolsAt = fn(String, Duration) -> ToolSet;
    for (tool_name, input, who, tools_at) in [
        (
            commit::NAME,
            COMMIT_INPUT,
            "the forge",
            commit_tools as ToolsAt,
        ),
        (
            release::NAME,
            r#"{"tag":null}"#,
            "the mirror",
            release_tools as ToolsAt,
        ),
    ] {
        for (case, answer, timeout, expected) in [
            (
                "status",
                LookupAnswer::Json(502, json!({})),
                Duration::from_secs(10),
                format!("{who} answered HTTP 502"),
            ),
            (
                "timeout",
                LookupAnswer::Stall(Duration::from_secs(5)),
                Duration::from_millis(100),
                format!("{who} did not answer within the time bound"),
            ),
        ] {
            let server = LookupServer::start(answer).await;
            let tools = tools_at(server.base(), timeout);
            let script = ToolScript {
                tool: tool_name.into(),
                input: input.into(),
                narration: None,
            };
            let (fixture, mut replies) = tool_fixture(script, None, tools).await;

            let receipt = support::ingest_recorded(
                &fixture.assistant,
                inbound(
                    &channel("dm-fail"),
                    ChannelKind::Direct,
                    "42",
                    "what changed?",
                ),
            )
            .await;
            let blocks = settle_shape(
                &fixture.store,
                receipt.conversation_id,
                "the failed-call turn",
                &[
                    "system_prompt",
                    "tool_palette",
                    "chat_message",
                    "tool_call",
                    "tool_error",
                    "text",
                ],
            )
            .await;
            assert_eq!(
                field(&blocks[4], "error"),
                expected,
                "the {tool_name} {case} path records its named tool error"
            );
            // The model saw the error: the closing request carried the
            // answered call, and the chat received only the model's answer.
            assert_eq!(
                recv_reply(&mut replies).await.text,
                assistant_core::disclosed(CLOSING_ANSWER)
            );
            let extra = replies.try_recv();
            assert!(
                extra.is_err(),
                "the raw error never reaches the chat; got {extra:?}"
            );
        }
    }
}

/// A redirect answer is a tool error, never a second request: the client is
/// built without redirect following per the one-bounded-GET contract, so
/// the scripted 302 — pointing straight back at the same server — resolves
/// the call as the named tool error and the server sees exactly one GET.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redirect_answer_is_a_tool_error_and_is_not_followed() {
    let forge = LookupServer::start(LookupAnswer::Redirect("/elsewhere".into())).await;
    let tools = commit_tools(forge.base(), commit::DEFAULT_TIMEOUT);
    let script = ToolScript {
        tool: commit::NAME.into(),
        input: COMMIT_INPUT.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-redirect"),
            ChannelKind::Direct,
            "42",
            "what changed?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the redirected-call turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[4], "error"),
        "the forge answered with a redirect, which this lookup does not follow"
    );
    assert_eq!(
        forge.requests().len(),
        1,
        "one bounded GET: the redirect was not followed"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

// ─── The palette: supersession on first activity, the creation set ───────

/// A store-direct conversation modeling a pre-unit store: mapped and
/// prompted but carrying no palette block. Under the on-delta supersession
/// (2026-08-23), its first activity appends the registered set as a fresh
/// palette block ahead of the message — so the very turn that activity
/// summons admits the tool, and the scripted forge sees the call. The
/// supersession makes the no-palette shape unreachable end to end, so the
/// refusal it once drew here is pinned behaviorally on the admission
/// wrapper, in that module's tests, where a conversation no
/// reconciliation touched can still exist.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pre_unit_conversation_gains_the_registered_tools_on_first_activity() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let forge = LookupServer::start(LookupAnswer::Json(200, forge_commit_body())).await;

    // The pre-unit shape, written store-directly: conversation, prompt and
    // channel mapping exist; no palette block does.
    let conversation = store
        .create_conversation(
            "scripted-1".into(),
            "script-model".into(),
            "Script Model".into(),
            support::VENDOR.into(),
        )
        .await
        .expect("a conversation row");
    store
        .insert_system_prompt(conversation, support::SYSTEM_PROMPT.into())
        .await
        .expect("the prompt records");
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute(
            "INSERT INTO channels (adapter, channel, kind, conversation_id) \
             VALUES (?1, ?2, 'direct', ?3)",
            (support::ADAPTER, "dm-pre-unit", conversation),
        )?;
        Ok(())
    })
    .await
    .expect("the pre-unit mapping writes");

    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: commit::NAME.into(),
            input: COMMIT_INPUT.into(),
            narration: None,
        },
        None,
    );
    let mut tools = ToolSet::new();
    tools.admit(
        commit::REQUIRED_AUTHORITY,
        CommitLookup::new(forge.base(), commit::DEFAULT_TIMEOUT),
    );
    let (fixture, mut replies) =
        assemble(store, provider, handle, tools, ProtectionConfig::default()).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-pre-unit"),
            ChannelKind::Direct,
            "42",
            "what changed?",
        ),
    )
    .await;
    assert_eq!(receipt.conversation_id, conversation);

    let blocks = settle_shape(
        &fixture.store,
        conversation,
        "the admitted first-activity turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The delta append: the superseding palette lands ahead of the message
    // and names exactly the registered set, so this very turn admits.
    let names: Vec<String> =
        serde_json::from_str(&field(&blocks[1], "tools")).expect("the stored list parses");
    assert_eq!(
        names,
        vec![
            commit::NAME.to_owned(),
            assistant_core::tools::rights::NAME.to_owned()
        ],
        "the appended palette names the registered set, the privacy tool included"
    );
    assert_eq!(field(&blocks[4], "content"), forge_compact_result());
    assert_eq!(
        forge.requests().len(),
        1,
        "the gained tool ran against the forge"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// A created conversation's palette names exactly the registered set — the
/// three lookups plus the always-registered privacy tool — and a direct and
/// a group conversation get the identical palette.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_created_conversation_names_exactly_the_registered_set_direct_and_group_alike() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let direct = support::ingest_recorded(
        &fixture.assistant,
        inbound(&channel("dm-palette"), ChannelKind::Direct, "42", "hello"),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-palette").await;
    let group = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "42", "hello group"),
    )
    .await;
    recv_reply(&mut replies).await;
    recv_reply(&mut replies).await;

    let mut palettes = Vec::new();
    for conversation in [direct.conversation_id, group.conversation_id] {
        let blocks = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let stored: Vec<&Block> = blocks
            .iter()
            .filter(|block| block.block_type == "tool_palette")
            .collect();
        assert_eq!(stored.len(), 1, "one palette block per conversation");
        assert_eq!(
            blocks[1].block_type, "tool_palette",
            "the palette sits beside the prompt, before any message"
        );
        let names: Vec<String> =
            serde_json::from_str(&field(stored[0], "tools")).expect("the stored list parses");
        assert_eq!(
            names,
            vec![
                commit::NAME.to_owned(),
                release::NAME.to_owned(),
                assistant_core::tools::wiki::NAME.to_owned(),
                assistant_core::tools::rights::NAME.to_owned()
            ],
            "the palette names the three lookups and the always-registered privacy tool"
        );
        palettes.push(names);
    }
    assert_eq!(palettes[0], palettes[1], "direct and group get one palette");
}

// ─── The anchor gate: admission by the turn's provenance ─────────────────
//
// Authority enforcement is the admission wrapper's provenance gate
// (decision 0043): the call block's dispatch anchor names the turn's
// summoning frontier, and the reading is the minimum over the anchor's
// debt ORIGIN SET — the own-debt-takers in the contiguous answer-due
// chain ending at the anchor, a pure propagator carrying no vote — and
// the span's CO-SUMMONERS, the addressed, unlimited messages absorbed
// between the anchor and the call, the same opened-debt predicate the
// budgets count. Unaddressed chatter and a line the budgets refused
// contribute nothing. Registration accepts any authority, so the gate
// itself carries the whole rule, pinned here end to end under the
// production event order.

/// A member call is admitted for a member-level tool: the body runs and
/// its result is recorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_call_is_admitted_for_a_member_level_tool() {
    let (probe, executed) = ProbeTool::new("member_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Member, probe);
    let script = ToolScript {
        tool: "member_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&channel("dm-member"), ChannelKind::Direct, "42", "probe it"),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the admitted turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert!(executed.load(Ordering::SeqCst), "the admitted body ran");
    assert_eq!(field(&blocks[4], "content"), "the probe ran");
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// Admitted above the floor: an admin-summoned turn's call to an
/// admin-level tool passes the anchor gate — the call's dispatch anchor
/// names the admin summons, the interval holds nothing lower, and the body
/// runs. The anchor premise is asserted on the stored call block itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admin_summoned_turn_admits_an_admin_tool() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &channel("dm-admin-summons"),
            ChannelKind::Direct,
            "boss",
            Authority::Admin,
            "probe it",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the admitted admin turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert!(executed.load(Ordering::SeqCst), "the admitted body ran");
    assert_eq!(field(&blocks[4], "content"), "the probe ran");
    assert_eq!(
        blocks[3].dispatch_anchor,
        Some(blocks[2].id),
        "the call block anchors on the summoning message"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// Declined below the requirement: a member-summoned turn's call to an
/// admin-level tool is declined by the anchor gate with the reading
/// recorded in the error text, the body provably never runs, and the turn
/// still closes with the model's answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_summoned_turn_declines_an_admin_tool() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-member-summons"),
            ChannelKind::Direct,
            "42",
            "probe it",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the declined turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    let decline = field(&blocks[4], "error");
    assert!(
        decline.contains("needs admin authority"),
        "the decline names the requirement: {decline}"
    );
    assert!(
        decline.contains("reads member"),
        "the decline records the reading, per decision 0043: {decline}"
    );
    assert!(
        decline.contains("Do not call this tool again"),
        "the decline teaches the model not to retry: {decline}"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "declined means the body never ran"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// The narrated-turn absorbed shape the refuted walk escalated on: a
/// member summons an admin-tool turn, and an addressed admin message is
/// absorbed while the narration is still streaming — before the call
/// block exists. The absorbed admin co-summons the turn, but the fold is
/// a minimum: the member summons keeps the reading at member and the
/// admin tool stays declined. Absorption cannot escalate a turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admin_absorbed_mid_narration_cannot_escalate_a_member_summons() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let hold = support::TurnHold::new();
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: Some("One moment.".into()),
    };
    let (fixture, mut replies) = tool_fixture(script, Some(hold.clone()), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-narrated-absorption").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "m", "the summons"),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration is mid-stream — the ledger's tail is the streaming
    // block — when the admin message is absorbed, provably before the
    // call block exists.
    hold.started().await;
    await_streaming_tail(&fixture.store, conv).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "an admin mid-turn",
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the declined narrated turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    // The premise: the absorbed admin is addressed — a co-summoner, not a
    // bystander — and still cannot raise the minimum.
    assert_eq!(blocks[3].fields["addressed"], json!(true));
    let decline = field(&blocks[6], "error");
    assert!(
        decline.contains("reads member"),
        "the absorbed admin cannot raise the fold above the member \
         summons: {decline}"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "declined means the body never ran"
    );
    assert_eq!(
        blocks[5].dispatch_anchor,
        Some(blocks[2].id),
        "the call anchors on the summons, never on the absorbed message"
    );
    recv_closing(&mut replies).await;
}

/// The between-rounds absorbed shape the refuted walk had no bound for: an
/// admin summons a turn, its narration finalizes, and an ADDRESSED member
/// message is absorbed in the window between the finalize and the call's
/// insert. The absorbed member opened a debt of its own — the turn under
/// way answers it — so it co-summons the turn and the fold lowers the
/// reading to member: the admin tool is declined, stated against the
/// escalation the shape used to leak.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_addressed_member_absorbed_between_rounds_lowers_an_admin_summons() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let hold = support::TurnHold::new();
    let rounds = vec![
        Round {
            narration: Some("One moment."),
            hold_after_finalize: true,
            hold_before_done: false,
            call: Some("admin_probe"),
        },
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: false,
            call: None,
        },
    ];
    let (fixture, mut replies) = round_fixture(rounds, Arc::clone(&hold), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-window-absorption").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "the summons",
        ),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration finalized, the call not yet inserted: the ledger's
    // tail is the finalized text when the member message is absorbed.
    hold.started().await;
    await_tail(&fixture.store, conv, "the finalized narration", "text").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "m", "a member in the window"),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the declined windowed turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "text",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    // The premise: the absorbed member is a co-summoner — addressed, and
    // no budget refused it.
    assert_eq!(blocks[4].fields["addressed"], json!(true));
    assert_eq!(blocks[4].fields.get("limited"), None);
    let decline = field(&blocks[6], "error");
    assert!(
        decline.contains("reads member"),
        "the absorbed member co-summons and lowers the admin summons: {decline}"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "declined means the body never ran"
    );
    assert_eq!(
        blocks[5].dispatch_anchor,
        Some(blocks[2].id),
        "the call anchors on the summons across the absorption window"
    );
    recv_closing(&mut replies).await;
}

/// The narrowing decision 0043 documents, pinned from the admitted side: a
/// resting member's unaddressed message recorded before the admin summons
/// lies outside the provenance interval — the span starts strictly at the
/// summons, and the summons carries no debt from a resting author — so the
/// admin tool is still admitted even though the dispatched request carries
/// the member's text. Without the span read's lower bound the pre-summons
/// member would fold in and this turn would read member instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_resting_member_before_the_summons_lies_outside_the_interval() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-pre-summons-member").await;

    // The resting member: recorded before the summons, unaddressed,
    // summoning nothing.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &key,
            ChannelKind::Group,
            "m",
            "a bystander before the summons",
        ),
    )
    .await;
    let conv = receipt.conversation_id;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "probe it",
        ),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the admitted turn behind a resting member",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise of the narrowing: the member rests without a debt, so
    // the summons opens its own at admin and carries nothing lower.
    assert_eq!(blocks[2].fields["answer_due"], json!(false));
    assert_eq!(blocks[3].fields["debt_authority"], json!("admin"));
    assert!(executed.load(Ordering::SeqCst), "the admitted body ran");
    assert_eq!(field(&blocks[5], "content"), "the probe ran");
    assert_eq!(
        blocks[4].dispatch_anchor,
        Some(blocks[3].id),
        "the call anchors on the summons, past the resting member"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// The refinement's modal case (decision 0043, refined 2026-08-22): live
/// group chatter landing during the turn's span is not a veto. An admin
/// summons an admin-tool turn, two unaddressed member lines are absorbed
/// while the narration streams, and the tool is still admitted — an
/// unaddressed message summons nothing and co-summons nothing, one rule
/// for context in the span as before the summons.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unaddressed_bystanders_absorbed_mid_turn_contribute_nothing() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let hold = support::TurnHold::new();
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: Some("One moment.".into()),
    };
    let (fixture, mut replies) = tool_fixture(script, Some(hold.clone()), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-bystanders").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "probe it",
        ),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration is mid-stream when the two bystander lines land,
    // provably before the call block exists — inside the interval the
    // gate folds.
    hold.started().await;
    await_streaming_tail(&fixture.store, conv).await;
    for text in ["a bystander line", "another bystander line"] {
        support::ingest_recorded(
            &fixture.assistant,
            inbound_unaddressed(&key, ChannelKind::Group, "m", text),
        )
        .await;
    }
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the admitted turn behind the bystanders",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise: both absorbed lines rest unaddressed inside the span.
    for absorbed in [&blocks[3], &blocks[4]] {
        assert_eq!(absorbed.fields["addressed"], json!(false));
    }
    assert!(executed.load(Ordering::SeqCst), "the admitted body ran");
    assert_eq!(field(&blocks[7], "content"), "the probe ran");
    assert_eq!(
        blocks[6].dispatch_anchor,
        Some(blocks[2].id),
        "the call anchors on the admin summons, past the bystanders"
    );
    recv_closing(&mut replies).await;
}

/// A refused line is not a veto (decision 0043, refined 2026-08-22): under
/// a one-answer channel budget the admin summons consumes the slot, so the
/// addressed member line absorbed mid-turn is recorded limited — the
/// protection unit refused it service — and joins no fold: the admin tool
/// is admitted. The budgets and the gate agree on one opened-debt
/// predicate; a message outside it neither spends budget nor lowers
/// provenance.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_limited_line_absorbed_mid_turn_does_not_veto() {
    let (probe, executed) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Admin, probe);
    let hold = support::TurnHold::new();
    let script = ToolScript {
        tool: "admin_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: Some("One moment.".into()),
    };
    let (fixture, mut replies) = tool_fixture_configured(
        script,
        Some(hold.clone()),
        tools,
        support::budgets(None, Some((1, 600))),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-limited-line").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "probe it",
        ),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration is mid-stream when the flooder's addressed line lands:
    // the channel budget's one slot is already the summons's, so the line
    // is recorded limited.
    hold.started().await;
    await_streaming_tail(&fixture.store, conv).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key,
            ChannelKind::Group,
            "m",
            "an addressed line past the budget",
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the admitted turn behind the refused line",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise: the absorbed line is addressed AND limited — the
    // budget refused its debt, so it opened none and co-summons nothing.
    assert_eq!(blocks[3].fields["addressed"], json!(true));
    assert_eq!(blocks[3].fields["limited"], json!("channel"));
    assert_eq!(blocks[3].fields["answer_due"], json!(false));
    assert!(executed.load(Ordering::SeqCst), "the admitted body ran");
    assert_eq!(field(&blocks[6], "content"), "the probe ran");
    assert_eq!(
        blocks[5].dispatch_anchor,
        Some(blocks[2].id),
        "the call anchors on the admin summons, past the refused line"
    );
    recv_closing(&mut replies).await;
}

/// The escalation ledger of 0043's second refinement (2026-08-22), under
/// the production event order: a member summons a tool turn, round one's
/// call records and its result inserts while the round's stream is still
/// open — a real wire's shape, the trailing done held — and the admin's
/// addressed line is absorbed AFTER that result, in the window whose
/// tail-derived continuation anchor used to hand the whole turn the
/// admin's identity. The continuation's admin call still anchors the
/// ORIGINAL member summons, reads member, and is declined with its body
/// never run: a line absorbed behind a result joins the fold as a
/// co-summoner but can never re-anchor the turn it joins.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admin_absorbed_after_a_rounds_result_cannot_reanchor_the_turn() {
    let (member_probe, member_ran) = ProbeTool::new("member_probe");
    let (admin_probe, admin_ran) = ProbeTool::new("admin_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Member, member_probe);
    tools.admit(Authority::Admin, admin_probe);
    let hold = support::TurnHold::new();
    let rounds = vec![
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: true,
            call: Some("member_probe"),
        },
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: false,
            call: Some("admin_probe"),
        },
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: false,
            call: None,
        },
    ];
    let (fixture, mut replies) = round_fixture(rounds, Arc::clone(&hold), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-post-result-absorption").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "m", "the summons"),
    )
    .await;
    let conv = receipt.conversation_id;

    // Round one is held before its trailing done: the call recorded, the
    // result inserted, the stream provably still open when the admin's
    // line lands — so the line sits between the result and the
    // continuation the close re-drives.
    hold.started().await;
    await_tail(&fixture.store, conv, "round one's result", "tool_result").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "an admin after the result",
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the declined continuation",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    // The premise: the absorbed admin sits between round one's result and
    // the continuation's call, addressed, its own debt taken.
    assert_eq!(blocks[5].fields["addressed"], json!(true));
    assert_eq!(blocks[5].fields.get("limited"), None);
    // The turn's identity: both calls anchor the original member summons —
    // the continuation never re-anchors onto the absorbed admin.
    assert_eq!(blocks[3].dispatch_anchor, Some(blocks[2].id));
    assert_eq!(
        blocks[6].dispatch_anchor,
        Some(blocks[2].id),
        "the continuation's call anchors the original summons, not the \
         absorbed admin"
    );
    let decline = field(&blocks[7], "error");
    assert!(
        decline.contains("needs admin authority") && decline.contains("reads member"),
        "the member summons keeps the reading at member: {decline}"
    );
    assert!(
        member_ran.load(Ordering::SeqCst),
        "round one's member tool ran"
    );
    assert!(
        !admin_ran.load(Ordering::SeqCst),
        "declined means the admin body never ran"
    );
    recv_closing(&mut replies).await;
    // The continuation answered the absorbed line: its request carries it.
    let requests = fixture.script.seen.lock().unwrap();
    assert!(
        requests[1]
            .iter()
            .any(|m| carries(m, "an admin after the result")),
        "the continuation's request carries the absorbed admin line"
    );
}

/// The veto ledger of 0043's second refinement (2026-08-22), under the
/// production event order: an admin's command is followed on its heels by
/// a member's unaddressed line, which propagates the admin's debt
/// (decision 0021) and becomes the turn's summoning frontier — the anchor
/// whose min-folded stamp reads member, the fold that used to veto the
/// admin whose debt it merely carried. The gate reads the debt's ORIGIN
/// SET through the propagator: the admin who took the debt votes, the
/// carrier does not, and the admin tool admits and runs.
///
/// The ledger is written by the production ingest path in a first process
/// whose provider never answers, so the propagating line is durably the
/// tail; the second process boots its actor latched over the stored
/// ledger, and the production unlatch intent — the very event the admin's
/// own ingest emits — is replayed once, now that the propagating line is
/// the tail, so the turn fires against the exact frontier the live
/// incident dispatched on.
#[test]
fn a_propagating_frontier_reads_the_admins_debt_and_admits() {
    let db = support::TempDb::new("veto-ledger");
    let key = channel("room-propagating-frontier");
    let admin_tools = || {
        let (probe, executed) = ProbeTool::new("admin_probe");
        let mut tools = ToolSet::new();
        tools.admit(Authority::Admin, probe);
        (tools, executed)
    };

    // Process one: the admin command, then the member's unaddressed line —
    // recorded and stamped by the production path, answered by nothing.
    let conv = support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the first store opens");
        let (tools, _never_consulted) = admin_tools();
        let assistant = Assistant::start(
            store.clone(),
            Arc::new(EventBus::new()),
            support::registry_of(support::silent_provider()),
            tools,
            assistant_core::AssemblyConfig {
                binding: support::binding(),
                system_prompt: support::SYSTEM_PROMPT.into(),
                protection: ProtectionConfig::default(),
                operators: support::operator_config(),
                direct_chats: assistant_core::DirectChats::default(),
                privacy_policy_address: None,
                moderation_handle: None,
            },
        )
        .await
        .expect("the first assembly starts");
        support::authorize(&assistant, &key).await;
        let receipt = support::ingest_recorded(
            &assistant,
            inbound_as(
                &key,
                ChannelKind::Group,
                "boss",
                Authority::Admin,
                "probe it",
            ),
        )
        .await;
        support::ingest_recorded(
            &assistant,
            inbound_unaddressed(&key, ChannelKind::Group, "m", "lol"),
        )
        .await;
        // The premise: the line propagates the admin's debt, and its own
        // min-folded stamp — the ANSWERING fact — reads member.
        let messages = chat_messages(&store, receipt.conversation_id).await;
        assert_eq!(messages.len(), 2, "the command and the line");
        assert_eq!(messages[1].fields["addressed"], json!(false));
        assert_eq!(messages[1].fields["answer_due"], json!(true));
        assert_eq!(
            messages[1].fields["debt_authority"],
            json!("member"),
            "the carrier's min-folded stamp is the fold that vetoed the admin"
        );
        receipt.conversation_id
    });

    // Process two: the actor boots latched over the stored ledger, whose
    // tail is the propagating line.
    support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let (tools, executed) = admin_tools();
        let (provider, handle) = tool_scripted_provider(
            ToolScript {
                tool: "admin_probe".into(),
                input: r#"{"ask":"run"}"#.into(),
                narration: None,
            },
            None,
        );
        let (fixture, mut replies) =
            assemble(store, provider, handle, tools, ProtectionConfig::default()).await;
        fixture.bus.emit(CoreEvent::UnlatchRequested {
            conversation_id: conv,
        });

        let blocks = settle_shape(
            &fixture.store,
            conv,
            "the admitted propagated turn",
            &[
                "system_prompt",
                "tool_palette",
                "chat_message",
                "chat_message",
                "tool_call",
                "tool_result",
                "text",
            ],
        )
        .await;
        assert_eq!(
            blocks[4].dispatch_anchor,
            Some(blocks[3].id),
            "the turn anchors on the propagating frontier, not on the \
             admin command"
        );
        assert!(executed.load(Ordering::SeqCst), "the admin body ran");
        assert_eq!(field(&blocks[5], "content"), "the probe ran");
        let closing = recv_reply(&mut replies).await.text;
        assert_eq!(closing, assistant_core::disclosed(CLOSING_ANSWER));
    });
}

// ─── AC6: the tail-only stamp under mid-turn absorption ──────────────────

/// The conversation's chat messages, oldest first.
async fn chat_messages(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == "chat_message")
        .collect()
}

/// Await the ledger's newest block reaching the given type — the stored
/// shape a hold's window promises before the test absorbs a message.
async fn await_tail(store: &Store, conversation_id: i64, what: &str, tail: &str) {
    support::await_ledger(store, conversation_id, what, |blocks| {
        blocks.last().is_some_and(|block| block.block_type == tail)
    })
    .await;
}

/// Await the streaming tail an open narration promises.
async fn await_streaming_tail(store: &Store, conversation_id: i64) {
    support::await_ledger(store, conversation_id, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
}

/// Await the closing answer, consuming any narration texts delivered ahead
/// of it — a finalized narration is an answer block like any other on the
/// outbound edge. Matched by tail: whichever answer block delivered first
/// carries the person's disclosure line, so the closing text may arrive
/// bare or introduced.
async fn recv_closing(
    replies: &mut tokio::sync::mpsc::UnboundedReceiver<assistant_core::OutboundReply>,
) {
    while !recv_reply(replies).await.text.ends_with(CLOSING_ANSWER) {}
}

/// The tail-only stamp under mid-turn absorption, per decision 0021's
/// closure (2026-08-22): an addressed message absorbed while the turn's
/// narration is still streaming opens a FRESH debt at its own authority —
/// the ledger's tail is the turn's streaming machinery, which carries no
/// unanswered chat debt to read. Correct for answering, since the turn
/// under way answers the absorbed message together with the summons; and
/// tool admission never reads this stamp — the anchor gate reads the
/// turn's provenance through the call's dispatch anchor (decision 0043).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absorbed_message_opens_a_fresh_debt_at_its_own_authority() {
    let (probe, _ran) = ProbeTool::new("member_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Member, probe);
    let hold = support::TurnHold::new();
    let script = ToolScript {
        tool: "member_probe".into(),
        input: r#"{"ask":"run"}"#.into(),
        narration: Some("One moment.".into()),
    };
    let (fixture, mut replies) = tool_fixture(script, Some(hold.clone()), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-fresh-debt").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "m", "the summons"),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration is mid-stream — the ledger's tail is the streaming
    // block — when the admin message is absorbed.
    hold.started().await;
    await_streaming_tail(&fixture.store, conv).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &key,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "an admin mid-turn",
        ),
    )
    .await;
    let absorbed = chat_messages(&fixture.store, conv).await;
    assert_eq!(absorbed.len(), 2, "the summons and the absorbed message");
    assert_eq!(absorbed[1].fields["answer_due"], json!(true));
    assert_eq!(
        absorbed[1].fields["debt_authority"],
        json!("admin"),
        "an absorbed message opens a fresh debt at its own authority — the \
         tail-only read chains nothing through mid-turn machinery"
    );
    hold.release();

    // The turn completes and pays every debt: the closing answer is the
    // newest block, and nothing summons a second turn.
    recv_closing(&mut replies).await;
}

// ─── The duplicate-turn redispatch canary ────────────────────────────────

/// The canary for the framework's duplicate-turn window, now closed
/// (2026-08-22): a message absorbed between a narration's finalize and the
/// tool call's insert used to make the runtime dispatch a second model
/// turn over the mid-turn ledger. The dispatch state now settles only on
/// the stream's closed signal, so the window cannot fire — the provider
/// counts exactly the turn's two requests, and a regression would surface
/// as a third counted request replaying an already-played round.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_redispatch_window_dispatches_no_echo_turn() {
    let (probe, _ran) = ProbeTool::new("member_probe");
    let mut tools = ToolSet::new();
    tools.admit(Authority::Member, probe);
    let hold = support::TurnHold::new();
    let rounds = vec![
        Round {
            narration: Some("One moment."),
            hold_after_finalize: true,
            hold_before_done: false,
            call: Some("member_probe"),
        },
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: false,
            call: None,
        },
    ];
    let (fixture, mut replies) = round_fixture(rounds, Arc::clone(&hold), tools).await;
    let key = support::authorized_group(&fixture.assistant, "room-redispatch-canary").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "m", "the summons"),
    )
    .await;
    let conv = receipt.conversation_id;

    // The narration finalized, the call not yet inserted: the ledger's
    // tail is the finalized text when the second message is absorbed —
    // the redispatch window.
    hold.started().await;
    await_tail(&fixture.store, conv, "the finalized narration", "text").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "n", "a question in the window"),
    )
    .await;
    // Keep the window open long enough for a redispatched turn to reach
    // the provider before the release closes it — without the pause the
    // race is decided by scheduler timing and the echo shows up in only
    // some runs.
    tokio::time::sleep(Duration::from_millis(300)).await;
    hold.release();

    recv_closing(&mut replies).await;
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        2,
        "exactly the turn's two requests: the call round and the close — \
         the window dispatched no third"
    );
}

// ─── AC8: budgets and tools compose ──────────────────────────────────────

/// A turn that calls tools consumes exactly one answer slot, and a limited
/// message summons no tools: under a one-answer budget the first ask runs
/// its whole tool turn, the second is recorded limited, draws no reply, no
/// further tool call and no further forge request.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tool_turn_takes_one_slot_and_a_limited_message_summons_no_tools() {
    let forge = LookupServer::start(LookupAnswer::Json(200, forge_commit_body())).await;
    let (fixture, mut replies) = tool_fixture_configured(
        ToolScript {
            tool: commit::NAME.into(),
            input: COMMIT_INPUT.into(),
            narration: None,
        },
        None,
        commit_tools(forge.base(), commit::DEFAULT_TIMEOUT),
        support::budgets(Some((1, 600)), None),
    )
    .await;
    let key = channel("dm-budget");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the first ask"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
    let settled = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the whole tool turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(forge.requests().len(), 1);

    // The second ask crosses the one-answer budget: recorded, limited,
    // never summoning a turn — and therefore never a tool.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the second ask"),
    )
    .await;
    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the recorded limited message",
        |blocks| blocks.len() == settled.len() + 1,
    )
    .await;
    let limited = blocks.last().expect("the limited message is newest");
    assert_eq!(limited.block_type, "chat_message");
    assert_eq!(limited.fields["limited"], json!("principal"));
    assert_eq!(limited.fields["answer_due"], json!(false));

    // A grace period so a wrongly summoned turn would surface.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    assert_eq!(
        after.len(),
        settled.len() + 1,
        "the limited message summoned nothing: {:?}",
        after
            .iter()
            .map(|b| b.block_type.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        after
            .iter()
            .filter(|block| block.block_type == "tool_call")
            .count(),
        1,
        "one tool call in total — the limited message summoned no tools"
    );
    assert_eq!(forge.requests().len(), 1, "no further forge request");
    // A wrongly dispatched turn for the limited message would be counted —
    // every request the provider serves increments the turn count — so the
    // exact two keeps the no-turn claim falsifiable.
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        2,
        "one answer slot: the opening and closing requests of one turn"
    );
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "the limited ask draws no reply; got {extra:?}"
    );
}

// ─── AC2: the wiki lookup over the scripted raw host ─────────────────────

/// The scripted raw host's page body for the happy-path pins.
const WIKI_PAGE: &str = "# Home\n\nWelcome to the halogenOS wiki.\n";

/// One wiki tool set over a scripted server and a timeout.
fn wiki_tools(base: String, timeout: Duration) -> ToolSet {
    let mut tools = ToolSet::new();
    tools.admit(wiki::REQUIRED_AUTHORITY, WikiLookup::new(base, timeout));
    tools
}

/// The wiki lookup end to end: the page fetch reaches the model — the raw
/// text is the result block, asked at the raw host's page path — and the
/// answer reaches the chat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_wiki_lookup_reads_a_page_end_to_end() {
    let host = LookupServer::start(LookupAnswer::Text(200, WIKI_PAGE.into())).await;
    let tools = wiki_tools(host.base(), wiki::DEFAULT_TIMEOUT);
    let script = ToolScript {
        tool: wiki::NAME.into(),
        input: r#"{"page":"Home"}"#.into(),
        narration: None,
    };
    let (fixture, mut replies) = tool_fixture(script, None, tools).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-wiki"),
            ChannelKind::Direct,
            "42",
            "what does the wiki say?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the wiki turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[4], "content"), WIKI_PAGE);
    let requests = host.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/wiki/halogenOS/android_manifest/Home.md");
    assert_eq!(
        recv_reply(&mut replies).await.text,
        assistant_core::disclosed(CLOSING_ANSWER)
    );
}

/// The wiki failure paths: a missing page becomes the tool error naming
/// the page-name shape, and a timeout under a short constructed bound
/// becomes the time-bound error — the chat receives only the model's
/// answer either way.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_missing_wiki_page_and_a_timeout_become_tool_errors() {
    for (case, answer, timeout, expected) in [
        (
            "missing page",
            LookupAnswer::Text(404, "404: Not Found".into()),
            Duration::from_secs(10),
            "the wiki has no page named `Guessed-Page` — a page is named by its title \
             with spaces as dashes, parentheses literal; fetch the Home page or the \
             _Sidebar page to see the page names"
                .to_owned(),
        ),
        (
            "timeout",
            LookupAnswer::Stall(Duration::from_secs(5)),
            Duration::from_millis(100),
            "the wiki did not answer within the time bound".to_owned(),
        ),
    ] {
        let host = LookupServer::start(answer).await;
        let tools = wiki_tools(host.base(), timeout);
        let script = ToolScript {
            tool: wiki::NAME.into(),
            input: r#"{"page":"Guessed-Page"}"#.into(),
            narration: None,
        };
        let (fixture, mut replies) = tool_fixture(script, None, tools).await;
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            inbound(
                &channel("dm-wiki-fail"),
                ChannelKind::Direct,
                "42",
                "what does the wiki say?",
            ),
        )
        .await;
        let blocks = settle_shape(
            &fixture.store,
            receipt.conversation_id,
            "the failed wiki turn",
            &[
                "system_prompt",
                "tool_palette",
                "chat_message",
                "tool_call",
                "tool_error",
                "text",
            ],
        )
        .await;
        assert_eq!(
            field(&blocks[4], "error"),
            expected,
            "the {case} path records its named tool error"
        );
        assert_eq!(
            recv_reply(&mut replies).await.text,
            assistant_core::disclosed(CLOSING_ANSWER)
        );
        let extra = replies.try_recv();
        assert!(
            extra.is_err(),
            "the raw error never reaches the chat; got {extra:?}"
        );
    }
}

/// One direct call of a tool handler, outside any turn: the context is the
/// bare agency wiring the framework would hand it, which the wiki lookup
/// never reads.
async fn call_wiki(tool: &WikiLookup, store: &Store, input: &str) -> ToolOutcome {
    let agency = agent_ledger::AgencyCtx {
        conversation_id: 1,
        store: store.clone(),
        bus: Arc::new(EventBus::new()),
    };
    let ctx = ToolContext {
        agency: &agency,
        tool_call_id: "call-direct",
        block_id: 0,
    };
    tool.execute(input, ctx).await
}

/// The cache over the real wire, where time runs: a repeat inside the TTL
/// serves without a second request, a missing page's answer is cached the
/// same way, and the entry cap clears the cache whole — the refetch past
/// the TTL is pinned under paused time on the cache itself, beside its
/// clock in the wiki module.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_wiki_cache_serves_repeats_caches_the_missing_page_and_clears_at_the_cap() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");

    // A repeat inside the TTL: one request on the wire.
    let host = LookupServer::start(LookupAnswer::Text(200, WIKI_PAGE.into())).await;
    let tool = WikiLookup::new(host.base(), wiki::DEFAULT_TIMEOUT);
    for _ in 0..2 {
        let outcome = call_wiki(&tool, &store, r#"{"page":"Home"}"#).await;
        assert!(
            matches!(&outcome, ToolOutcome::Done(text) if text == WIKI_PAGE),
            "the page answers from the wire, then from the cache"
        );
    }
    assert_eq!(
        host.requests().len(),
        1,
        "the repeat inside the TTL asked the wire nothing"
    );

    // A missing page is cached alike: two calls, one request, the same
    // named error both times.
    let missing = LookupServer::start(LookupAnswer::Text(404, "404: Not Found".into())).await;
    let guessing = WikiLookup::new(missing.base(), wiki::DEFAULT_TIMEOUT);
    for _ in 0..2 {
        let outcome = call_wiki(&guessing, &store, r#"{"page":"Guessed-Page"}"#).await;
        assert!(
            matches!(&outcome, ToolOutcome::Error(error) if error.contains("no page named")),
            "the missing page answers the named error, cached"
        );
    }
    assert_eq!(
        missing.requests().len(),
        1,
        "negative caching bounds a model guessing page names"
    );

    // The cap: one page past it clears the cache whole, so the first
    // page's next ask meets the wire again.
    let capped_host = LookupServer::start(LookupAnswer::Text(200, WIKI_PAGE.into())).await;
    let capped = WikiLookup::new(capped_host.base(), wiki::DEFAULT_TIMEOUT);
    for n in 0..=wiki::CACHE_CAP {
        let input = format!("{{\"page\":\"Page-{n}\"}}");
        let outcome = call_wiki(&capped, &store, &input).await;
        assert!(matches!(outcome, ToolOutcome::Done(_)));
    }
    let before_repeat = capped_host.requests().len();
    assert_eq!(
        before_repeat,
        wiki::CACHE_CAP + 1,
        "every distinct page fetched once"
    );
    let outcome = call_wiki(&capped, &store, r#"{"page":"Page-0"}"#).await;
    assert!(matches!(outcome, ToolOutcome::Done(_)));
    assert_eq!(
        capped_host.requests().len(),
        wiki::CACHE_CAP + 2,
        "the cap cleared the cache whole, so the first page refetched"
    );
}

/// The result bound over the wire: an over-bound page is cut at the named
/// limit with the truncation marker riding after it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_bound_wiki_page_is_truncated_with_the_marker() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let long_page = "x".repeat(wiki::RESULT_LIMIT + 500);
    let host = LookupServer::start(LookupAnswer::Text(200, long_page)).await;
    let tool = WikiLookup::new(host.base(), wiki::DEFAULT_TIMEOUT);
    let outcome = call_wiki(&tool, &store, r#"{"page":"Home"}"#).await;
    let ToolOutcome::Done(text) = outcome else {
        panic!("the over-bound page still answers");
    };
    assert!(
        text.ends_with(wiki::TRUNCATION_MARKER),
        "the marker names the cut"
    );
    assert_eq!(
        text.chars().count(),
        wiki::RESULT_LIMIT + wiki::TRUNCATION_MARKER.chars().count(),
        "the bound counts characters of the page, the marker rides after it"
    );
}
