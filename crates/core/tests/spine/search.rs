//! The web search at the core's edges (unit 27): the envelope over a
//! scripted vendor, the guard and the person bound refusing before anything
//! is sent, the taught failures, and the one predicate that decides whether
//! the tool exists at all.
//!
//! The vendor cannot be called from a test, and no recorded payload of it
//! exists anywhere in the tree, so the fixtures here are AUTHORED to the
//! shapes the unit's grounding recorded from the live probe of 2026-08-27 —
//! a short page for a ten-row request, a row without a snippet, rows
//! carrying fields the tool ignores, and the 403 whose JSON `message` is
//! the only thing telling a key refusal from any other refusal. They are
//! served through the suite's existing loopback pattern
//! (`crate::lookup_wire`), and are refreshed against a live probe when the
//! operator's key exists.

use assistant_core::tools::search;
use assistant_core::{Authority, ChannelKind};
use serde_json::{Value, json};

use crate::lookup_wire::{LookupAnswer, LookupServer};
use crate::support::{
    self, CLOSING_ANSWER, ToolScript, channel, field, inbound, inbound_as, recv_reply,
    settle_shape, tool_scripted_provider,
};

/// The turn shape a search call writes: the palette, the message, the call
/// and its outcome, then the model's closing answer.
const SEARCH_TURN: [&str; 6] = [
    "system_prompt",
    "tool_palette",
    "chat_message",
    "tool_call",
    "tool_result",
    "text",
];

/// The same turn announced first (unit 40): the heads-up line finalizes as
/// its own text block ahead of the call, so one turn writes two texts with
/// the call and its result between them.
const ANNOUNCED_SEARCH_TURN: [&str; 7] = [
    "system_prompt",
    "tool_palette",
    "chat_message",
    "text",
    "tool_call",
    "tool_result",
    "text",
];

/// The same turn where the call was refused or failed.
const DECLINED_TURN: [&str; 6] = [
    "system_prompt",
    "tool_palette",
    "chat_message",
    "tool_call",
    "tool_error",
    "text",
];

/// Await the streaming tail an open narration promises — the window in
/// which a second person's message is absorbed, provably before the call
/// block exists.
async fn await_streaming_tail(store: &agent_ledger::Store, conversation_id: i64) {
    support::await_ledger(store, conversation_id, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
}

/// One call's input.
fn ask(query: &str) -> String {
    json!({ "query": query }).to_string()
}

/// The vendor's answer to a ten-row request, authored to the grounding's
/// recorded shape: EIGHT rows for a ten-row ask — the short page the
/// vendor's own samples return — one of them without a snippet, several
/// carrying fields this tool ignores, and no total-results field anywhere,
/// because the vendor sends none.
fn vendor_page() -> Value {
    json!({
        "searchParameters": { "q": "linux kernel", "page": 1, "type": "search" },
        "organic": [
            {
                "title": "Kernel (operating system)",
                "link": "https://en.wikipedia.org/wiki/Kernel_(operating_system)",
                "snippet": "A kernel is the core of an operating system.",
                "position": 1
            },
            {
                "title": "The Linux Kernel Archives",
                "link": "https://www.kernel.org/",
                "snippet": "The mainline tree and its releases.",
                "position": 2,
                "sitelinks": [{ "title": "Releases", "link": "https://www.kernel.org/releases" }]
            },
            {
                "title": "Kernel research",
                "link": "https://cs.stanford.edu/research/kernel",
                "snippet": "A university page about kernels.",
                "position": 3
            },
            {
                "title": "Government guidance on operating systems",
                "link": "https://www.gov.uk/guidance/operating-systems",
                "snippet": "Official guidance.",
                "position": 4
            },
            {
                "title": "A kernel post",
                "link": "https://someone.medium.com/a-kernel-post",
                "snippet": "One writer's notes.",
                "position": 5,
                "date": "2026-02-02"
            },
            {
                "title": "No snippet here",
                "link": "https://example.invalid/no-snippet",
                "position": 6
            },
            {
                "title": "A relative row",
                "link": "/relative/path",
                "snippet": "A row whose link carries no host.",
                "position": 7,
                "attributes": { "rating": "4" }
            },
            {
                "title": "An ordinary site",
                "link": "https://example.invalid/ordinary",
                "snippet": "Ordinary prose.",
                "position": 8
            }
        ]
    })
}

/// What the answer above renders as — every line of the envelope, stated.
fn rendered_page() -> String {
    "Web search results for: linux kernel\n\
     Page: 1\n\
     Results: 8\n\
     \n\
     1. Kernel (operating system)\n\
     Link: https://en.wikipedia.org/wiki/Kernel_(operating_system)\n\
     Source: encyclopedia\n\
     Snippet: A kernel is the core of an operating system.\n\
     \n\
     2. The Linux Kernel Archives\n\
     Link: https://www.kernel.org/\n\
     Source: website\n\
     Snippet: The mainline tree and its releases.\n\
     \n\
     3. Kernel research\n\
     Link: https://cs.stanford.edu/research/kernel\n\
     Source: official\n\
     Snippet: A university page about kernels.\n\
     \n\
     4. Government guidance on operating systems\n\
     Link: https://www.gov.uk/guidance/operating-systems\n\
     Source: official\n\
     Snippet: Official guidance.\n\
     \n\
     5. A kernel post\n\
     Link: https://someone.medium.com/a-kernel-post\n\
     Source: blog\n\
     Snippet: One writer's notes.\n\
     \n\
     6. No snippet here\n\
     Link: https://example.invalid/no-snippet\n\
     Source: website\n\
     \n\
     7. A relative row\n\
     Link: /relative/path\n\
     Source: unknown\n\
     Snippet: A row whose link carries no host.\n\
     \n\
     8. An ordinary site\n\
     Link: https://example.invalid/ordinary\n\
     Source: website\n\
     Snippet: Ordinary prose."
        .to_owned()
}

/// AC2 and AC3: the envelope renders exactly what arrived — a page short of
/// the requested ten, a row without a snippet, unknown row fields ignored —
/// and every row's source hint follows the inlined table. The request is
/// the posted one the vendor takes: the query as written, the page, the
/// requested count, autocorrect off and the configured locale, with the key
/// in its header.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_envelope_renders_the_short_page_the_vendor_answered() {
    let vendor = LookupServer::start(LookupAnswer::Json(200, vendor_page())).await;
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("linux kernel"),
            narration: None,
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-search"),
            ChannelKind::Direct,
            "42",
            "what is a kernel?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the search turn",
        &SEARCH_TURN,
    )
    .await;
    assert_eq!(field(&blocks[4], "content"), rendered_page());

    // The wire: one POST, at the vendor's path, with the key in its header
    // and the body the unit fixed.
    let requests = vendor.requests();
    assert_eq!(requests.len(), 1, "one search, one request");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/search");
    assert_eq!(
        requests[0].header("x-api-key"),
        Some(support::SEARCH_KEY),
        "the configured key travels in the vendor's header"
    );
    let body = requests[0].json_body();
    assert_eq!(body["q"], json!("linux kernel"), "the query as written");
    assert_eq!(body["page"], json!(1));
    assert_eq!(body["num"], json!(10));
    assert_eq!(
        body["autocorrect"],
        json!(false),
        "a silently corrected query would answer a question nobody asked"
    );
    assert_eq!(body["hl"], json!("en"), "the configured language");
    assert!(
        body.get("gl").is_none(),
        "an unconfigured country sends no gl"
    );

    // The chat receives the model's answer alone — never the results.
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, support::disclosed(CLOSING_ANSWER));
    assert!(
        !reply.text.contains("kernel.org"),
        "the results never reach the chat"
    );
}

/// One searching fixture over a scripted vendor answer and one scripted
/// call input, run to a settled turn of the given shape; answers the
/// outcome block's field and the vendor's recorded requests.
async fn declined_turn(answer: LookupAnswer, input: String, channel_id: &str) -> (String, usize) {
    let vendor = LookupServer::start(answer).await;
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input,
            narration: None,
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel(channel_id),
            ChannelKind::Direct,
            "42",
            "a question",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the declined search turn",
        &DECLINED_TURN,
    )
    .await;
    // The chat receives the model's own answer, never the taught decline.
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, support::disclosed(CLOSING_ANSWER));
    (field(&blocks[4], "error"), vendor.requests().len())
}

/// AC4's refused key: the vendor's 403 is read for its JSON message, and
/// the taught result names the refusal without a status number anywhere.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_key_teaches_its_own_result_and_never_a_status() {
    let (taught, requests) = declined_turn(
        LookupAnswer::Json(
            403,
            json!({ "message": "Unauthorized.", "statusCode": 403 }),
        ),
        ask("linux kernel"),
        "dm-search-403",
    )
    .await;
    assert_eq!(taught, search::REFUSED_KEY_RESULT);
    assert_eq!(requests, 1, "the request was made and refused");
    for status in ["403", "HTTP"] {
        assert!(
            !taught.contains(status),
            "no bare status surfaces: {taught}"
        );
    }
    assert!(
        !taught.contains(support::SEARCH_KEY),
        "the failure path never carries the key: {taught}"
    );
}

/// AC4's other refusal at the same status: a 403 that is not about the key
/// reads as its own taught result, told apart by the message alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_that_is_not_about_the_key_teaches_a_different_result() {
    let (taught, _) = declined_turn(
        LookupAnswer::Json(403, json!({ "message": "Not enough credits" })),
        ask("linux kernel"),
        "dm-search-credits",
    )
    .await;
    assert_eq!(taught, search::REFUSED_REQUEST_RESULT);
    assert_ne!(taught, search::REFUSED_KEY_RESULT);
}

/// AC4's rate limit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_limit_teaches_its_own_result() {
    let (taught, _) = declined_turn(
        LookupAnswer::Json(429, json!({ "message": "Too many requests" })),
        ask("linux kernel"),
        "dm-search-429",
    )
    .await;
    assert_eq!(taught, search::RATE_LIMITED_RESULT);
}

/// AC4's empty page one: an honest nothing, recorded as a RESULT and not as
/// a failure — the two readings are what tell an empty search from a broken
/// one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_empty_page_is_answered_and_not_failed() {
    let vendor = LookupServer::start(LookupAnswer::Json(200, json!({ "organic": [] }))).await;
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("a query nothing answers"),
            narration: None,
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-search-empty"),
            ChannelKind::Direct,
            "42",
            "a question",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the empty search turn",
        &SEARCH_TURN,
    )
    .await;
    assert_eq!(
        field(&blocks[4], "content"),
        search::no_results_result("a query nothing answers")
    );
    recv_reply(&mut replies).await;
}

/// AC4's unreachable host: nothing listens at the configured address, and
/// the taught result says so without a status or a transport error's own
/// prose.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unreachable_vendor_teaches_its_own_result() {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("linux kernel"),
            narration: None,
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        support::UNROUTABLE.to_owned(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-search-down"),
            ChannelKind::Direct,
            "42",
            "a question",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the unreachable search turn",
        &DECLINED_TURN,
    )
    .await;
    assert_eq!(field(&blocks[4], "error"), search::UNREACHABLE_RESULT);
    recv_reply(&mut replies).await;
}

/// `AC7b` end to end: a query carrying a person reference is refused before
/// the wire, the stub proves no request arrived, and the recorded refusal
/// carries no fragment of the matched token — a tool record is exactly what
/// erasure cannot reach.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_person_reference_is_refused_before_the_wire() {
    let (taught, requests) = declined_turn(
        LookupAnswer::Json(200, vendor_page()),
        ask("what did @quietsparrow say about the kernel"),
        "dm-search-guard",
    )
    .await;
    assert_eq!(taught, search::PERSON_REFERENCE_RESULT);
    assert_eq!(requests, 0, "a refused query reaches no vendor");
    for fragment in ["quietsparrow", "sparrow", "@quiet"] {
        assert!(
            !taught.contains(fragment),
            "the refusal echoes the matched token: {taught}"
        );
    }
}

/// AC7's bounds end to end: an over-long query is refused whole with the
/// limit named and nothing is sent — never a truncated question.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_long_query_is_refused_whole_with_nothing_sent() {
    let (taught, requests) = declined_turn(
        LookupAnswer::Json(200, vendor_page()),
        ask(&"x".repeat(search::QUERY_LIMIT + 1)),
        "dm-search-long",
    )
    .await;
    assert_eq!(taught, search::over_long_result());
    assert!(
        taught.contains(&search::QUERY_LIMIT.to_string()),
        "the refusal names the limit: {taught}"
    );
    assert_eq!(requests, 0, "nothing was sent");
}

/// AC7's page bound end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_page_past_the_last_is_refused_with_nothing_sent() {
    let (taught, requests) = declined_turn(
        LookupAnswer::Json(200, vendor_page()),
        json!({ "query": "linux kernel", "page": 6 }).to_string(),
        "dm-search-page",
    )
    .await;
    assert_eq!(taught, search::page_out_of_range_result());
    assert_eq!(requests, 0, "nothing was sent");
}

/// AC7's person key, end to end and only reachable here: a turn whose
/// debt-origin set holds TWO distinct people declines the spend, because
/// the budget is per person and folding two people into one bucket would
/// break exactly that guarantee. The second person's message is absorbed
/// mid-narration, provably before the call block exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_holding_two_people_declines_the_spend() {
    let vendor = LookupServer::start(LookupAnswer::Json(200, vendor_page())).await;
    let hold = support::TurnHold::new();
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("linux kernel"),
            narration: Some("One moment.".into()),
        },
        Some(hold.clone()),
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-search-two-people").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "asker", "what is a kernel?"),
    )
    .await;
    hold.started().await;
    await_streaming_tail(&fixture.store, receipt.conversation_id).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "second-asker",
            Authority::Member,
            "and how does it schedule?",
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the two-person search turn",
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
    assert_eq!(
        blocks[3].fields["addressed"],
        json!(true),
        "the premise: the absorbed message co-summons the turn"
    );
    assert_eq!(field(&blocks[6], "error"), search::NO_SINGLE_PERSON_RESULT);
    assert_eq!(
        vendor.requests().len(),
        0,
        "a spend with no single person to book it to is never made"
    );
    recv_reply(&mut replies).await;
}

/// AC5 and AC9: a configured key puts the tool in the palette AND its
/// teaching in the recorded prompt; no key leaves both absent, and nothing
/// fails at startup either way. One predicate, pinned from both sides.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_configured_key_admits_and_teaches_and_no_key_does_neither() {
    let vendor = LookupServer::start(LookupAnswer::Json(200, vendor_page())).await;
    let (provider, script) = support::scripted_provider(None);
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let searching = support::start_assistant_searching(
        store,
        provider,
        script,
        support::production_toolset(),
        vendor.base(),
    )
    .await;
    let mut replies = searching
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &searching.assistant,
        inbound(
            &channel("dm-search-palette"),
            ChannelKind::Direct,
            "42",
            "hello",
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    let blocks = searching
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let names: Vec<String> =
        serde_json::from_str(&field(&blocks[1], "tools")).expect("the stored list parses");
    assert!(
        names.contains(&search::NAME.to_owned()),
        "a configured key puts the search in the palette: {names:?}"
    );
    assert_eq!(
        field(&blocks[0], "content"),
        support::composed_searching_prompt(),
        "the recorded prompt is the composition with the search teaching"
    );
    assert!(
        field(&blocks[0], "content").contains(assistant_core::SEARCH_TEACHING),
        "the teaching rides the prompt the conversation records"
    );

    // The unconfigured half: the suite's default fixture configures no key.
    let plain = support::start_assistant(None).await;
    let mut plain_replies = plain
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let plain_receipt = support::ingest_recorded(
        &plain.assistant,
        inbound(&channel("dm-no-search"), ChannelKind::Direct, "42", "hello"),
    )
    .await;
    recv_reply(&mut plain_replies).await;
    let plain_blocks = plain
        .store
        .list_blocks(plain_receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let plain_names: Vec<String> =
        serde_json::from_str(&field(&plain_blocks[1], "tools")).expect("the stored list parses");
    assert!(
        !plain_names.contains(&search::NAME.to_owned()),
        "no key, no palette entry: {plain_names:?}"
    );
    let plain_prompt = field(&plain_blocks[0], "content");
    assert!(
        !plain_prompt.contains(search::NAME),
        "no key, no teaching either"
    );
    assert!(
        !plain_prompt.contains(assistant_core::SEARCH_TEACHING),
        "the composed prompt teaches no tool the palette does not carry"
    );
}

/// AC6 end to end: nothing the run records carries a fragment of the key —
/// not the recorded prompt, not the palette, not the call, not the result,
/// and not the delivered reply.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_recorded_block_carries_a_fragment_of_the_key() {
    let vendor = LookupServer::start(LookupAnswer::Json(200, vendor_page())).await;
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("linux kernel"),
            narration: None,
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-search-secret"),
            ChannelKind::Direct,
            "42",
            "what is a kernel?",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the search turn",
        &SEARCH_TURN,
    )
    .await;
    let recorded = format!("{blocks:?}");
    for fragment in [support::SEARCH_KEY, "FAKE-SEARCH", "0123456789"] {
        assert!(
            !recorded.contains(fragment),
            "a recorded block carries a fragment of the key"
        );
    }
    let reply = recv_reply(&mut replies).await;
    assert!(!reply.text.contains("FAKE-SEARCH"));
}

/// AC3 (unit 40): the operator's example shape, by its two deterministic
/// facts. The LEDGER order — the announce text stands before the call
/// block, which stands before the result — is read from the settled shape,
/// and it holds because the message end that finalizes the narration
/// precedes the drained tool lifecycle on every wire. The CHAT order — the
/// announce reply arrives before the answer reply — is read from two
/// receives on the one outbound edge, and the disclosure line tells them
/// apart: the announce is this person's first delivery and carries the
/// introduction, the closing answer behind it arrives bare.
///
/// What is NOT pinned, deliberately: that the announce was delivered
/// before the search's result existed. Asserting that would race the
/// outbound edge against the vendor's stub over two bus subscribers, and a
/// flaky pin proves less than no pin. The two facts here bracket the same
/// claim — announce, then search, then answer — without a stopwatch.
///
/// The announce text is a fixture string, not product copy: the live model
/// words its own line from the teaching.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_announce_ahead_of_the_search_arrives_before_the_answer() {
    const ANNOUNCE: &str = "Let me look up what a kernel is.";
    let vendor = LookupServer::start(LookupAnswer::Json(200, vendor_page())).await;
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: search::NAME.into(),
            input: ask("linux kernel"),
            narration: Some(ANNOUNCE.into()),
        },
        None,
    );
    let store = agent_ledger::Store::in_memory_with(assistant_core::schema::store_config())
        .expect("an in-memory store opens");
    let fixture = support::start_assistant_searching(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        vendor.base(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-search-announced"),
            ChannelKind::Direct,
            "42",
            "what is a kernel?",
        ),
    )
    .await;

    // The chat's arrival order: the introduced announce, then the bare
    // answer, on the one edge that delivers both.
    let introduced = support::disclosed(ANNOUNCE);
    let announced = recv_reply(&mut replies).await;
    assert_eq!(
        announced.text, introduced,
        "the announce is the first thing the chat receives"
    );
    let answered = recv_reply(&mut replies).await;
    assert_eq!(
        answered.text, CLOSING_ANSWER,
        "the answer follows it, bare — the introduction rode the announce"
    );

    // The ledger's order: the announce, the call, its result, the answer.
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the announced search turn",
        &ANNOUNCED_SEARCH_TURN,
    )
    .await;
    assert_eq!(field(&blocks[3], "content"), introduced);
    assert_eq!(field(&blocks[4], "name"), search::NAME);
    assert_eq!(field(&blocks[5], "content"), rendered_page());
    assert_eq!(field(&blocks[6], "content"), CLOSING_ANSWER);
    assert_eq!(
        vendor.requests().len(),
        1,
        "the announced turn ran the one search it announced"
    );
}

// ─── The palette pins across the units that share these files ────────────

/// The palette pins in the neighbouring suites name the runtime-facts tool
/// beside the search tool.
///
/// It is a cross-file pin because the palette is one recorded list and its
/// pins live in two suites: a unit that adds a tool has to reach both, and
/// a search unit rewriting `tools.rs` from a base that predates another
/// unit's tool would silently drop it from the recorded set. This is the
/// cheap check that the drop did not happen.
#[test]
fn the_palette_pins_name_the_runtime_tool() {
    for (file, pins) in [
        ("crates/core/tests/spine/tools.rs", include_str!("tools.rs")),
        (
            "crates/core/tests/spine/report.rs",
            include_str!("report.rs"),
        ),
    ] {
        assert!(
            pins.contains("runtime::NAME"),
            "{file} pins a palette that no longer names the runtime-facts tool: the \
             recorded set is the three lookups, privacy, runtime facts, and the search \
             tool where a key is configured"
        );
    }
}
