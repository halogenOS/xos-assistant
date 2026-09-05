//! The process-level suite (AC8): the compiled binary against scripted
//! loopback endpoints, driven only through its public surface — the
//! configuration file, the environment, the signals and the exit status.
//!
//! The suite extends the token scan to the whole process: the fake bot token
//! and the fake provider key are asserted absent from the store file, the log
//! file and the captured stderr of a run that provably used both.

mod support;

use serde_json::json;
use support::{
    ANSWER, BinaryRun, CompletionsServer, DEADLINE, KEY, LookupServer, MIRROR_TOKEN, SEARCH_KEY,
    STOP_BOUND, Scratch, TOKEN, TelegramServer, UNROUTABLE, assert_absent,
    assert_absent_if_present, private_update,
};

/// The fixture prompt the loader reads; the run story's real prompt files
/// live in the repository and are not this suite's concern.
const PROMPT: &str = "You are the process suite's scripted assistant fixture.";

/// What one test varies about its configuration file; the defaults are what
/// most runs want, so a caller names only the knob its story turns.
struct ConfigOptions {
    /// The log destination as its raw TOML value, so a caller can spell the
    /// bare console word or the file table.
    log_destination: String,
    /// The forge endpoint, for the tool run against the scripted forge;
    /// absent points it at the unroutable loopback address, so no run ever
    /// registers a lookup against a real host.
    forge_root: Option<String>,
    /// The mirror endpoint, under the same rule.
    mirror_root: Option<String>,
    /// Whether the file names the optional mirror-token secret, behind the
    /// suite's environment variable.
    mirror_token: bool,
    /// The direct-chat switch's spelled value; absent omits the key, which
    /// means on.
    direct_chats: Option<&'static str>,
    /// The web search's endpoint. Present configures the whole search
    /// wiring — the address here and the key secret behind a scratch file;
    /// absent omits both, which is a deployment without a search.
    search_root: Option<String>,
    /// Raw TOML appended after the named tables — the budget test's
    /// protection table.
    extra_tables: String,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            log_destination: "\"stderr\"".into(),
            forge_root: None,
            mirror_root: None,
            mirror_token: false,
            direct_chats: None,
            search_root: None,
            extra_tables: String::new(),
        }
    }
}

/// One configuration file pointing every endpoint at the scripted servers,
/// with the token behind an environment variable and the key behind a file —
/// one of each indirection, so both are exercised.
// Debug formatting is deliberate: it quotes and escapes each value exactly
// as a TOML string literal needs.
#[allow(clippy::unnecessary_debug_formatting)]
fn configuration(
    scratch: &Scratch,
    telegram_root: &str,
    completions_base: &str,
    options: &ConfigOptions,
) -> std::path::PathBuf {
    let ConfigOptions {
        log_destination,
        forge_root,
        mirror_root,
        mirror_token,
        direct_chats,
        search_root,
        extra_tables,
    } = options;
    scratch.write("prompts/assistant.md", PROMPT);
    let key_file = scratch.write("provider-key", &format!("{KEY}\n"));
    let forge_endpoint = format!(
        "forge = {:?}\n",
        forge_root.as_deref().unwrap_or(UNROUTABLE)
    );
    let mirror_endpoint = format!(
        "mirror = {:?}\n",
        mirror_root.as_deref().unwrap_or(UNROUTABLE)
    );
    let mirror_secret = if *mirror_token {
        "[secrets.mirror_token]\nenv = \"PROCESS_TEST_MIRROR_TOKEN\"\n\n"
    } else {
        ""
    };
    // The search's address and its key travel together: the key alone is
    // what makes the tool exist, so a configuration naming one names both.
    let (search_endpoint, search_secret) = match search_root {
        Some(address) => {
            let key_file = scratch.write("search-key", &format!("{SEARCH_KEY}\n"));
            (
                format!("search = {address:?}\n"),
                format!("[secrets.search_api_key]\nfile = {key_file:?}\n\n"),
            )
        }
        None => (String::new(), String::new()),
    };
    // A top-level key, so it must sit ahead of the file's first table.
    let direct_chats_key = direct_chats
        .map(|value| format!("direct_chats = {value:?}\n"))
        .unwrap_or_default();
    scratch.write(
        "assistant.toml",
        &format!(
            "store_path = {:?}\n\
             telegram_state_path = {:?}\n\
             prompt_dir = {:?}\n\
             log = {log_destination}\n\
             model = \"test-vendor/test-model\"\n\
             {direct_chats_key}\n\
             [endpoints]\n\
             telegram = {telegram_root:?}\n\
             chat_completions = {completions_base:?}\n\
             {forge_endpoint}\
             {mirror_endpoint}\
             {search_endpoint}\n\
             [secrets.bot_token]\n\
             env = \"PROCESS_TEST_BOT_TOKEN\"\n\n\
             [secrets.chat_completions_api_key]\n\
             file = {:?}\n\n\
             {mirror_secret}\
             {search_secret}\
             {extra_tables}",
            scratch.path("store.db"),
            scratch.path("telegram.offset"),
            scratch.path("prompts"),
            key_file,
        ),
    )
}

/// The whole run story: the binary starts against the scripted endpoints,
/// answers a direct message with the completions server's prose, stops
/// cleanly on SIGTERM — and neither secret appears in the store file, the
/// log file or the captured stderr, though the run provably used both.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_process_answers_and_no_secret_reaches_the_store_or_a_log() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("answers");
    let log_file = scratch.path("assistant.log");
    // The table spelling, driven through the whole process: the file arm of
    // the log destination, spelled the way a file named after the console
    // word would need.
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            log_destination: format!(
                "{{ file = {:?} }}",
                log_file.to_str().expect("the scratch path is unicode")
            ),
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, "hello assistant"));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"].as_i64(), Some(42));
    assert_eq!(
        sends[0].body["text"].as_str(),
        Some(support::disclosed(ANSWER).as_str()),
        "the person's first answer opens with the disclosure line"
    );
    assert!(
        completions.hit_count() >= 1,
        "the answer must have come over the scripted completions wire"
    );
    // The prompt-file-to-wire join: the text written into the prompt
    // directory above — and nothing hardcoded — is the system message the
    // completions server received.
    assert!(
        completions.requests().iter().any(|body| {
            body["messages"].as_array().is_some_and(|messages| {
                messages.iter().any(|message| {
                    message["role"] == "system"
                        && serde_json::to_string(&message["content"])
                            .expect("the content serializes")
                            .contains(PROMPT)
                })
            })
        }),
        "the loaded prompt file's text reaches the wire as the system message"
    );
    assert!(
        !telegram.recorded("getMe").is_empty(),
        "the bot identity is fetched from the scripted server"
    );

    run.terminate();
    let status = run.wait_exit(STOP_BOUND).await;
    assert!(status.success(), "SIGTERM ends the process cleanly");

    let log = std::fs::read_to_string(&log_file).expect("the log file reads");
    assert!(
        log.contains("the assistant is up"),
        "the startup facts reach the configured log destination"
    );
    for (secret, what) in [(TOKEN, "the bot token"), (KEY, "the provider key")] {
        for name in ["assistant.log", "stderr.txt", "store.db"] {
            assert_absent(&scratch.path(name), secret, what);
        }
        // The sidecars are merged away when the store closes cleanly, so
        // only they may be absent; the store file itself must exist for
        // the scan to bind.
        for suffix in ["-wal", "-shm"] {
            assert_absent_if_present(&scratch.path(&format!("store.db{suffix}")), secret, what);
        }
    }
}

/// The mirror token, driven through the binary (AC7): the scripted
/// completions server makes the process's model call the release lookup,
/// the scripted mirror receives the request WITH the token as its
/// authorization header — so the run provably used the secret — and the
/// token still appears nowhere: not in the store file, not in the log file,
/// not on stderr.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_mirror_token_reaches_the_wire_and_no_artifact() {
    let telegram = TelegramServer::start().await;
    let completions =
        CompletionsServer::start_scripted(Some(("lookup_release".into(), "{\"tag\":null}".into())))
            .await;
    let mirror = LookupServer::start(json!({
        "tag_name": "20260707.2230.36-rb",
        "name": "[release build] for the process suite",
        "published_at": "2026-07-07T20:43:15Z",
        "html_url": "https://example.invalid/releases/latest",
        "assets": [{ "name": "boot.img", "size": 4096 }]
    }))
    .await;
    let scratch = Scratch::new("mirror-token");
    let log_file = scratch.path("assistant.log");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            log_destination: format!(
                "{{ file = {:?} }}",
                log_file.to_str().expect("the scratch path is unicode")
            ),
            mirror_root: Some(mirror.base()),
            mirror_token: true,
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, "what is the latest build?"));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[
            ("PROCESS_TEST_BOT_TOKEN", TOKEN),
            ("PROCESS_TEST_MIRROR_TOKEN", MIRROR_TOKEN),
        ],
        &scratch.path("stderr.txt"),
    );

    // The whole tool turn: the model called the lookup, the mirror answered
    // under the token, and the closing prose reached the chat.
    let requests = mirror.await_requests(1).await;
    assert_eq!(requests[0].path, "/repos/halogenOS/builds/releases/latest");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some(&*format!("Bearer {MIRROR_TOKEN}")),
        "the configured token flows to the mirror's authorization header"
    );
    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"].as_str(),
        Some(support::disclosed(ANSWER).as_str()),
        "the person's first answer opens with the disclosure line"
    );

    run.terminate();
    let status = run.wait_exit(STOP_BOUND).await;
    assert!(status.success(), "SIGTERM ends the process cleanly");

    // The scan: the token the run provably used reaches no artifact.
    for name in ["assistant.log", "stderr.txt", "store.db"] {
        assert_absent(&scratch.path(name), MIRROR_TOKEN, "the mirror token");
    }
    for suffix in ["-wal", "-shm"] {
        assert_absent_if_present(
            &scratch.path(&format!("store.db{suffix}")),
            MIRROR_TOKEN,
            "the mirror token",
        );
    }
}

/// AC6's logging half, driven through the binary: with the web search
/// configured, the startup line names the ADDRESS the search will post to,
/// and the key that travels in the request header appears in no artifact —
/// not in the log file, not on stderr, not in the store. The address is
/// asserted present so the scan cannot pass by the search simply not being
/// wired.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_startup_line_names_the_search_address_and_never_its_key() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("search-key");
    let log_file = scratch.path("assistant.log");
    // The search vendor is pointed at the unroutable loopback address: this
    // run configures the search and never calls it, so nothing leaves the
    // machine while the wiring is proven.
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            log_destination: format!(
                "{{ file = {:?} }}",
                log_file.to_str().expect("the scratch path is unicode")
            ),
            search_root: Some(UNROUTABLE.to_owned()),
            ..ConfigOptions::default()
        },
    );

    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    // The startup line is written once the assembly stands, so the run is
    // awaited by the artifact this test reads.
    let deadline = std::time::Instant::now() + DEADLINE;
    let log = loop {
        let log = std::fs::read_to_string(&log_file).unwrap_or_default();
        if log.contains("the assistant is up") {
            break log;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the startup line; the log held {log:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert!(
        log.contains(&format!("search_endpoint={UNROUTABLE}")),
        "the startup line names the address the search posts to; the log held {log:?}"
    );

    run.terminate();
    assert!(run.wait_exit(STOP_BOUND).await.success());

    // The key, and every fragment of it a partial rendering could leak.
    for fragment in [SEARCH_KEY, "sk-search", "FAKE-PROCESS-TEST-SEARCH-KEY"] {
        for name in ["assistant.log", "stderr.txt", "store.db"] {
            assert_absent(&scratch.path(name), fragment, "the web search key");
        }
        for suffix in ["-wal", "-shm"] {
            assert_absent_if_present(
                &scratch.path(&format!("store.db{suffix}")),
                fragment,
                "the web search key",
            );
        }
    }
}

/// The forge endpoint, driven through the binary (AC7): the `forge` key in
/// the configuration's endpoints table parses and reaches the commit lookup
/// — the scripted completions server makes the process's model call it, and
/// the scripted forge receives the request at the Forgejo path, with no
/// authorization header, before the closing prose reaches the chat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_forge_endpoint_reaches_the_commit_lookup() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start_scripted(Some((
        "lookup_commit".into(),
        "{\"repository\":\"android_manifest\",\"reference\":\"deadbeef\"}".into(),
    )))
    .await;
    let forge = LookupServer::start(json!({
        "sha": "deadbeef00112233445566778899aabbccddeeff",
        "html_url": "https://example.invalid/commit/deadbeef",
        "commit": {
            "message": "Track the manifest",
            "author": { "name": "A. Committer", "date": "2026-08-17T01:23:26+02:00" }
        }
    }))
    .await;
    let scratch = Scratch::new("forge");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            forge_root: Some(forge.base()),
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, "what changed?"));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    let requests = forge.await_requests(1).await;
    assert_eq!(
        requests[0].path, "/api/v1/repos/halogenOS/android_manifest/git/commits/deadbeef",
        "the configured forge endpoint reaches the commit lookup's dialect path"
    );
    assert_eq!(
        requests[0].authorization, None,
        "the forge is asked unauthenticated"
    );
    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"].as_str(),
        Some(support::disclosed(ANSWER).as_str()),
        "the person's first answer opens with the disclosure line"
    );

    run.terminate();
    assert!(run.wait_exit(STOP_BOUND).await.success());
}

/// The protection budget, driven through the binary (AC8): with the
/// configuration file's protection table granting one answer, the first
/// direct ask is answered and the second — provably processed, its update
/// confirmed by a later poll's offset — draws no send and no completion
/// request. Over-limit is silent in the chat by design; the limited fact in
/// the store is the audit trail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_configured_budget_limits_answers_through_the_binary() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("budget");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            extra_tables: "[protection]\nprincipal_answers = 1\n".into(),
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, "the first ask"));
    let run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );
    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"].as_str(),
        Some(support::disclosed(ANSWER).as_str()),
        "the person's first answer opens with the disclosure line"
    );

    // The second ask from the same person crosses the one-answer budget.
    // Its processing is proven by the poll cycle: the poller confirms an
    // update only after handling it, so a getUpdates asking from offset 3
    // means the second update went through the whole path.
    telegram.push_update(private_update(2, 42, "the second ask"));
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let confirmed = telegram
            .recorded("getUpdates")
            .iter()
            .any(|poll| poll.body["offset"].as_i64() == Some(3));
        if confirmed {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the poll that confirms the second update"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // A grace period so a wrongly spawned answer would surface as a send.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(
        telegram.recorded("sendMessage").len(),
        1,
        "the over-limit ask draws no answer and no notice"
    );

    // The spend the limit exists to save: the refused ask never reaches the
    // model at all. Read as what no request carries, not as a round count —
    // an answered turn takes two rounds since unit 55 (the round that calls
    // the sending tool and the round that reads its result), and a message
    // absorbed between them stands the second one down, so counting rounds
    // would be timing and not the budget. The words of the refused ask are
    // the exact thing that must never be sent to the model.
    let asked_the_model = completions.requests().iter().any(|body| {
        serde_json::to_string(body)
            .expect("the recorded body serializes")
            .contains("the second ask")
    });
    assert!(
        !asked_the_model,
        "the refused debt reached the model: the limit spent a model call it exists to save"
    );
    // Non-vacuity: the ADMITTED ask did reach it, so the reading above is a
    // statement about the refusal and not about a silent wire. The wire also
    // carries the framework's title derivation, told apart by its appended
    // instruction — the same discriminator the core suite uses.
    assert!(
        completions.requests().iter().any(|body| {
            let recorded = serde_json::to_string(body).expect("the recorded body serializes");
            recorded.contains("the first ask") && !recorded.contains("Generate a concise title")
        }),
        "the admitted ask reached the model"
    );
    drop(run);
}

/// The direct-chat switch, driven through the binary (decision 0069): with
/// `direct_chats = "off"` in the configuration file, a direct message is
/// disregarded before anything is written — the update is confirmed (a
/// later poll asks past it, so the offset advances), no send goes out, no
/// completion request is spent, and the message's distinctive text appears
/// nowhere in the store file, which the run still created.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_direct_chat_switch_off_disregards_a_direct_message_through_the_binary() {
    const PROBE: &str = "THE-DISTINCTIVE-OFF-SWITCH-PROBE";
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("direct-off");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            direct_chats: Some("off"),
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, PROBE));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    // The confirmation proves processing: the poller confirms an update
    // only after handling it, so a getUpdates asking from offset 2 means
    // the direct message went through the whole path and was acknowledged.
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let confirmed = telegram
            .recorded("getUpdates")
            .iter()
            .any(|poll| poll.body["offset"].as_i64() == Some(2));
        if confirmed {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the poll that confirms the disregarded update"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // A grace period so a wrongly spawned answer would surface as a send.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        telegram.recorded("sendMessage").is_empty(),
        "the disregarded message draws no send of any kind"
    );
    assert_eq!(
        completions.hit_count(),
        0,
        "no model turn is spent on a disregarded message"
    );

    run.terminate();
    assert!(run.wait_exit(STOP_BOUND).await.success());
    // The store exists — the scan binds — and holds nothing of the message.
    assert_absent(
        &scratch.path("store.db"),
        PROBE,
        "the disregarded message's text",
    );
}

/// The switch's other word, driven through the binary: `direct_chats =
/// "on"` spelled out decodes and behaves exactly like the absent key — the
/// direct message is answered over the scripted wire.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_direct_chat_switch_on_spelled_out_serves_direct_chats() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("direct-on");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions {
            direct_chats: Some("on"),
            ..ConfigOptions::default()
        },
    );

    telegram.push_update(private_update(1, 42, "hello again"));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"].as_i64(), Some(42));
    assert_eq!(
        sends[0].body["text"].as_str(),
        Some(support::disclosed(ANSWER).as_str()),
        "the person's first answer opens with the disclosure line"
    );

    run.terminate();
    assert!(run.wait_exit(STOP_BOUND).await.success());
}

/// A configuration the process cannot read exits nonzero before anything
/// starts: a missing argument, a missing file, and a file that does not
/// decode each name their refusal on stderr — and a decode refusal never
/// echoes the file's own text, only the failing place.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreadable_configuration_exits_nonzero() {
    let scratch = Scratch::new("refusals");

    let mut no_argument = BinaryRun::spawn(&[], &[], &scratch.path("stderr-usage.txt"));
    assert!(!no_argument.wait_exit(STOP_BOUND).await.success());
    let stderr = std::fs::read_to_string(scratch.path("stderr-usage.txt")).expect("stderr reads");
    assert!(
        stderr.contains("usage"),
        "the usage line names the argument"
    );

    let missing = scratch.path("absent.toml");
    let mut unreadable = BinaryRun::spawn(&[&missing], &[], &scratch.path("stderr-missing.txt"));
    assert!(!unreadable.wait_exit(STOP_BOUND).await.success());
    let stderr = std::fs::read_to_string(scratch.path("stderr-missing.txt")).expect("stderr reads");
    assert!(
        stderr.contains("could not be read"),
        "the refusal names the unreadable file; stderr was {stderr:?}"
    );

    let malformed = scratch.write("malformed.toml", "this is not [ toml");
    let mut undecodable =
        BinaryRun::spawn(&[&malformed], &[], &scratch.path("stderr-malformed.txt"));
    assert!(!undecodable.wait_exit(STOP_BOUND).await.success());
    let stderr =
        std::fs::read_to_string(scratch.path("stderr-malformed.txt")).expect("stderr reads");
    assert!(
        stderr.contains("does not decode"),
        "the refusal names the decoding failure; stderr was {stderr:?}"
    );

    // A secret pasted inline where its indirection belongs fails the decode
    // — and the refusal must not echo the pasted value, because it goes to
    // stderr before any logging is set up.
    let inline_secret = "INLINE-PASTED-FAKE-SECRET";
    let inline = scratch.write(
        "inline.toml",
        &format!(
            "store_path = \"s\"\n\
             telegram_state_path = \"t\"\n\
             prompt_dir = \"p\"\n\
             log = \"stderr\"\n\
             model = \"m\"\n\n\
             [secrets]\n\
             bot_token = {{ env = \"X\" }}\n\
             chat_completions_api_key = \"{inline_secret}\"\n"
        ),
    );
    let mut pasted = BinaryRun::spawn(&[&inline], &[], &scratch.path("stderr-inline.txt"));
    assert!(!pasted.wait_exit(STOP_BOUND).await.success());
    let stderr = std::fs::read_to_string(scratch.path("stderr-inline.txt")).expect("stderr reads");
    assert!(
        stderr.contains("does not decode"),
        "the refusal names the decoding failure; stderr was {stderr:?}"
    );
    assert!(
        !stderr.contains(inline_secret),
        "the pasted value must never be echoed; stderr was {stderr:?}"
    );
}

/// A secret whose named source cannot be read refuses the start, nonzero,
/// naming the source and never a value.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_secret_source_refuses_the_start() {
    let scratch = Scratch::new("secret-missing");
    let config = configuration(
        &scratch,
        "http://127.0.0.1:9",
        "http://127.0.0.1:9",
        &ConfigOptions::default(),
    );

    // The token's environment variable is deliberately not passed.
    let mut run = BinaryRun::spawn(&[&config], &[], &scratch.path("stderr.txt"));
    assert!(!run.wait_exit(STOP_BOUND).await.success());
    let stderr = std::fs::read_to_string(scratch.path("stderr.txt")).expect("stderr reads");
    assert!(
        stderr.contains("bot_token") && stderr.contains("PROCESS_TEST_BOT_TOKEN"),
        "the refusal names the secret's key and source; stderr was {stderr:?}"
    );
}

/// SIGTERM ends an idle, running process cleanly within the stated bound —
/// timed from the signal, on a process proven up by its recorded identity
/// fetch.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sigterm_ends_the_process_cleanly_within_the_stated_bound() {
    let telegram = TelegramServer::start().await;
    let completions = CompletionsServer::start().await;
    let scratch = Scratch::new("sigterm");
    let config = configuration(
        &scratch,
        &telegram.root(),
        &completions.base(),
        &ConfigOptions::default(),
    );

    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );
    telegram.await_recorded("getMe", 1).await;

    let signalled_at = std::time::Instant::now();
    run.terminate();
    let status = run.wait_exit(STOP_BOUND).await;
    assert!(status.success(), "SIGTERM ends the process cleanly");
    assert!(
        signalled_at.elapsed() < STOP_BOUND,
        "the stop stays within the stated bound"
    );
}
