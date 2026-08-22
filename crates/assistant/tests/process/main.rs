//! The process-level suite (AC8): the compiled binary against scripted
//! loopback endpoints, driven only through its public surface — the
//! configuration file, the environment, the signals and the exit status.
//!
//! The suite extends the token scan to the whole process: the fake bot token
//! and the fake provider key are asserted absent from the store file, the log
//! file and the captured stderr of a run that provably used both.

mod support;

use support::{
    ANSWER, BinaryRun, CompletionsServer, KEY, STOP_BOUND, Scratch, TOKEN, TelegramServer,
    assert_absent, assert_absent_if_present, private_update,
};

/// The fixture prompt the loader reads; the run story's real prompt files
/// live in the repository and are not this suite's concern.
const PROMPT: &str = "You are the process suite's scripted assistant fixture.";

/// One configuration file pointing every endpoint at the scripted servers,
/// with the token behind an environment variable and the key behind a file —
/// one of each indirection, so both are exercised. The log destination is
/// taken as its raw TOML value, so a caller can spell the bare console word
/// or the file table.
// Debug formatting is deliberate: it quotes and escapes each value exactly
// as a TOML string literal needs.
#[allow(clippy::unnecessary_debug_formatting)]
fn configuration(
    scratch: &Scratch,
    telegram_root: &str,
    completions_base: &str,
    log_destination: &str,
) -> std::path::PathBuf {
    scratch.write("prompts/assistant.md", PROMPT);
    let key_file = scratch.write("provider-key", &format!("{KEY}\n"));
    scratch.write(
        "assistant.toml",
        &format!(
            "store_path = {:?}\n\
             telegram_state_path = {:?}\n\
             prompt_dir = {:?}\n\
             log = {log_destination}\n\
             model = \"test-vendor/test-model\"\n\n\
             [endpoints]\n\
             telegram = {telegram_root:?}\n\
             openrouter = {completions_base:?}\n\n\
             [secrets.bot_token]\n\
             env = \"PROCESS_TEST_BOT_TOKEN\"\n\n\
             [secrets.openrouter_key]\n\
             file = {:?}\n",
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
        &format!(
            "{{ file = {:?} }}",
            log_file.to_str().expect("the scratch path is unicode")
        ),
    );

    telegram.push_update(private_update(1, 42, "hello assistant"));
    let mut run = BinaryRun::spawn(
        &[&config],
        &[("PROCESS_TEST_BOT_TOKEN", TOKEN)],
        &scratch.path("stderr.txt"),
    );

    let sends = telegram.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"].as_i64(), Some(42));
    assert_eq!(sends[0].body["text"].as_str(), Some(ANSWER));
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
             openrouter_key = \"{inline_secret}\"\n"
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
        "\"stderr\"",
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
        "\"stderr\"",
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
