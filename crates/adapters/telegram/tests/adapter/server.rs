//! The scripted Bot API server: a plain HTTP listener on the loopback
//! interface that answers the three methods the adapter speaks, from a
//! script, and records every request for the assertions. Nothing here
//! leaves the machine — the listener binds loopback and the adapter under
//! test is pointed at it.
//!
//! Update semantics mirror the platform's acknowledgement contract: the
//! server keeps every pushed update until a poll's offset confirms it, and a
//! poll returns everything at or past its offset. That is what lets the
//! restart and redelivery tests drive the adapter through the public wire
//! alone.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

use crate::support::DEADLINE;

/// How long an empty poll waits for a pushed update before answering empty —
/// the scripted stand-in for the platform's long poll, short so suites stay
/// fast.
const SCRIPTED_LONG_POLL: Duration = Duration::from_millis(200);

/// One recorded request: which method, with which decoded body.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub body: Value,
}

/// One scripted rate-limit refusal, answered instead of the real handler.
struct RateLimited {
    retry_after: u64,
}

/// One scripted send outcome, consumed in order, one per `sendMessage`.
enum SendScript {
    /// The rate-limit refusal with its stated wait.
    RateLimited(RateLimited),
    /// A scripted server failure.
    Failing,
    /// Delivered normally — a placeholder that lets a later entry name a
    /// specific chunk of a multi-chunk reply.
    Delivered,
}

/// How often a hanging administrator request re-reads its script, so a
/// test can release the wedge by scripting the chat again.
const HANG_RECHECK: Duration = Duration::from_millis(10);

/// What the administrator list answers for one chat.
#[derive(Clone)]
enum AdminScript {
    List(Vec<(i64, String)>),
    Failing,
    /// The request does not answer while this script stands — the fixture
    /// for a wedged consumer, which is what the webhook door's queue and
    /// deadline pins need. Re-scripting the chat releases the request,
    /// which then answers the new script: a step that was slow past the
    /// door's deadline and then succeeded.
    Hanging,
}

/// What the chat lookup answers for one chat: its title and, optionally,
/// its exposed pinned message as `(date, text)` — a zero date scripts the
/// inaccessible form.
pub struct ChatScript {
    pub title: Option<String>,
    pub pinned: Option<(i64, String)>,
}

#[derive(Default)]
struct ServerState {
    /// The bot identity `getMe` answers, `(id, username)`. `None` scripts
    /// the identity fetch to fail until one is set.
    me: Mutex<Option<(i64, String)>>,
    /// Every unconfirmed update, in push order.
    updates: Mutex<Vec<Value>>,
    /// Wakes a long-polling request when an update is pushed.
    wake: Notify,
    requests: Mutex<Vec<Recorded>>,
    send_scripts: Mutex<VecDeque<SendScript>>,
    poll_scripts: Mutex<VecDeque<RateLimited>>,
    admins: Mutex<HashMap<i64, AdminScript>>,
    /// The chat lookup's script per chat; an unscripted chat answers a
    /// scripted failure, so a test that says nothing about the lookup
    /// exercises the retried-on-next-contact path and keeps its ledger
    /// free of notes.
    chats: Mutex<HashMap<i64, ChatScript>>,
    /// Whether every `sendChatAction` answers a scripted server failure —
    /// the fixture for the failed-typing-action pin. Unset, actions
    /// succeed plainly.
    failing_chat_actions: Mutex<bool>,
    /// The answer every `sendMessage` carrying reply parameters is served,
    /// as a status and a description. Unset, a threaded send is served
    /// like any other.
    refused_threading: Mutex<Option<(u16, String)>>,
    /// The address currently registered as the webhook, as the platform
    /// keeps it: the empty string means none is registered. The registration
    /// sets it, the deletion clears it, and the pins read it.
    webhook_url: Mutex<String>,
    /// Whether every `setWebhook` answers a scripted refusal.
    failing_registration: Mutex<bool>,
    /// Whether every `deleteWebhook` answers a scripted server failure —
    /// the fixture for a polling start whose deletion fails.
    failing_webhook_delete: Mutex<bool>,
}

/// The running scripted server. Dropping it stops accepting; the per-test
/// runtime tears the connection tasks down with itself.
pub struct BotApiServer {
    root: String,
    state: Arc<ServerState>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for BotApiServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

impl BotApiServer {
    /// Bind a fresh loopback port and start serving, with the suite's
    /// default bot identity scripted for `getMe`.
    pub async fn start() -> Self {
        let server = Self::start_without_identity().await;
        server.set_me(crate::support::BOT_ID, crate::support::BOT_USERNAME);
        server
    }

    /// Bind a fresh loopback port with `getMe` scripted to fail until
    /// [`Self::set_me`] is called — the fixture for the identity-first
    /// contract.
    pub async fn start_without_identity() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let state = Arc::new(ServerState::default());
        let accept_state = Arc::clone(&state);
        let accept_task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let connection_state = Arc::clone(&accept_state);
                tokio::spawn(serve_connection(connection_state, stream));
            }
        });
        Self {
            root: format!("http://{addr}"),
            state,
            accept_task,
        }
    }

    /// The API root the adapter's configuration points at.
    pub fn root(&self) -> String {
        self.root.clone()
    }

    /// Script the bot identity `getMe` answers from here on.
    pub fn set_me(&self, id: i64, username: &str) {
        *self.state.me.lock().expect("the identity script locks") = Some((id, username.to_owned()));
    }

    /// Push one update; a hanging poll wakes and serves it.
    pub fn push_update(&self, update: Value) {
        self.state
            .updates
            .lock()
            .expect("the update list locks")
            .push(update);
        self.state.wake.notify_waiters();
    }

    /// Script one chat's administrator list: `(user id, status)` pairs.
    pub fn set_admins(&self, chat_id: i64, admins: &[(i64, &str)]) {
        let list = admins
            .iter()
            .map(|(id, status)| (*id, (*status).to_owned()))
            .collect();
        self.state
            .admins
            .lock()
            .expect("the admin scripts lock")
            .insert(chat_id, AdminScript::List(list));
    }

    /// Script one chat's lookup answer: its title and, optionally, its
    /// pinned message as `(date, text)`.
    pub fn set_chat_info(&self, chat_id: i64, title: &str, pinned: Option<(i64, &str)>) {
        self.state
            .chats
            .lock()
            .expect("the chat scripts lock")
            .insert(
                chat_id,
                ChatScript {
                    title: Some(title.to_owned()),
                    pinned: pinned.map(|(date, text)| (date, text.to_owned())),
                },
            );
    }

    /// Script one chat's administrator list to fail until re-scripted.
    pub fn fail_admins(&self, chat_id: i64) {
        self.state
            .admins
            .lock()
            .expect("the admin scripts lock")
            .insert(chat_id, AdminScript::Failing);
    }

    /// Script one chat's administrator list not to answer — the wedge
    /// behind the queue-full and deadline pins: the shared step parks inside
    /// this call, so whatever queues behind it provably waits. Scripting the
    /// chat again with [`Self::set_admins`] releases the parked request,
    /// which then answers the new script.
    pub fn hang_admins(&self, chat_id: i64) {
        self.state
            .admins
            .lock()
            .expect("the admin scripts lock")
            .insert(chat_id, AdminScript::Hanging);
    }

    /// Script the platform as already having a webhook registered at this
    /// address — the state a polling start's deletion clears.
    pub fn set_registered_webhook(&self, url: &str) {
        url.clone_into(
            &mut self
                .state
                .webhook_url
                .lock()
                .expect("the webhook state locks"),
        );
    }

    /// The address the platform currently holds registered, empty when none
    /// is — read after a delete to pin that the registration is gone.
    pub fn registered_webhook(&self) -> String {
        self.state
            .webhook_url
            .lock()
            .expect("the webhook state locks")
            .clone()
    }

    /// Script every `setWebhook` from here on to answer a refusal.
    pub fn fail_registration(&self) {
        *self
            .state
            .failing_registration
            .lock()
            .expect("the registration script locks") = true;
    }

    /// Script every `deleteWebhook` from here on to answer a server
    /// failure — the fixture for a polling start whose deletion fails.
    pub fn fail_webhook_delete(&self) {
        *self
            .state
            .failing_webhook_delete
            .lock()
            .expect("the webhook delete script locks") = true;
    }

    /// Script the next `times` sends to answer the rate-limit reply with the
    /// stated wait; sends past the script succeed.
    pub fn script_rate_limited_sends(&self, retry_after: u64, times: usize) {
        let mut scripts = self
            .state
            .send_scripts
            .lock()
            .expect("the send scripts lock");
        for _ in 0..times {
            scripts.push_back(SendScript::RateLimited(RateLimited { retry_after }));
        }
    }

    /// Script one send to fail outright after `delivered` sends that
    /// succeed — so a multi-chunk reply loses exactly the named later
    /// chunk; sends past the script succeed again.
    pub fn script_send_failure_after(&self, delivered: usize) {
        let mut scripts = self
            .state
            .send_scripts
            .lock()
            .expect("the send scripts lock");
        for _ in 0..delivered {
            scripts.push_back(SendScript::Delivered);
        }
        scripts.push_back(SendScript::Failing);
    }

    /// Script every send carrying reply parameters to answer the given
    /// status and description; a send without them is served normally. Both
    /// are the test's to choose: the description so a pin can name a cause
    /// other than a deleted target — the one cause the request's own
    /// tolerance already covers — and the status so a pin can tell the
    /// platform declining the request (a client error) from the platform
    /// failing to answer for it (a server error), which says nothing about
    /// whether the send was performed.
    pub fn refuse_threaded_sends(&self, status: u16, description: &str) {
        *self
            .state
            .refused_threading
            .lock()
            .expect("the threading refusal locks") = Some((status, description.to_owned()));
    }

    /// Script every `sendChatAction` from here on to answer a server
    /// failure.
    pub fn fail_chat_actions(&self) {
        *self
            .state
            .failing_chat_actions
            .lock()
            .expect("the action script locks") = true;
    }

    /// Script the next `times` update polls the same way; polls past the
    /// script serve normally.
    pub fn script_rate_limited_polls(&self, retry_after: u64, times: usize) {
        let mut scripts = self
            .state
            .poll_scripts
            .lock()
            .expect("the poll scripts lock");
        for _ in 0..times {
            scripts.push_back(RateLimited { retry_after });
        }
    }

    /// Every recorded request of one method, in arrival order.
    pub fn recorded(&self, method: &str) -> Vec<Recorded> {
        self.state
            .requests
            .lock()
            .expect("the request log locks")
            .iter()
            .filter(|recorded| recorded.method == method)
            .cloned()
            .collect()
    }

    /// Await at least `count` recorded requests of one method, or name the
    /// stall.
    pub async fn await_recorded(&self, method: &str, count: usize) -> Vec<Recorded> {
        let deadline = std::time::Instant::now() + DEADLINE;
        loop {
            let recorded = self.recorded(method);
            if recorded.len() >= count {
                return recorded;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out awaiting {count} recorded {method} requests; have {}",
                recorded.len()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// Serve one connection: HTTP/1.1 requests in a keep-alive loop, one
/// scripted answer each. The parser handles exactly what the adapter's
/// client sends — a POST with a JSON body and a content length.
async fn serve_connection(state: Arc<ServerState>, mut stream: TcpStream) {
    let mut buffered: Vec<u8> = Vec::new();
    loop {
        let Some((method, body, consumed)) = read_request(&mut stream, &mut buffered).await else {
            return;
        };
        buffered.drain(..consumed);
        let (status, answer) = dispatch(&state, method, body).await;
        let payload = answer.to_string();
        let reason = if status == 200 { "OK" } else { "Error" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{payload}",
            payload.len()
        );
        if stream.write_all(response.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// Read one whole request from the stream into the buffer; answer the method
/// name, the decoded body, and how many buffered bytes the request consumed.
/// `None` when the peer closed or sent something unreadable.
async fn read_request(
    stream: &mut TcpStream,
    buffered: &mut Vec<u8>,
) -> Option<(String, Value, usize)> {
    let header_end = loop {
        if let Some(position) = find_header_end(buffered) {
            break position;
        }
        read_more(stream, buffered).await?;
    };
    let head = String::from_utf8_lossy(&buffered[..header_end]).into_owned();
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let body_end = header_end + 4 + content_length;
    while buffered.len() < body_end {
        read_more(stream, buffered).await?;
    }
    let method = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or_default()
        .to_owned();
    let body =
        serde_json::from_slice(&buffered[header_end + 4..body_end]).unwrap_or_else(|_| json!({}));
    Some((method, body, body_end))
}

/// The position of the header terminator, when the buffer holds it.
fn find_header_end(buffered: &[u8]) -> Option<usize> {
    buffered.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Read one more chunk; `None` on close or error.
async fn read_more(stream: &mut TcpStream, buffered: &mut Vec<u8>) -> Option<()> {
    let mut chunk = [0_u8; 4096];
    match stream.read(&mut chunk).await {
        Ok(0) | Err(_) => None,
        Ok(n) => {
            buffered.extend_from_slice(&chunk[..n]);
            Some(())
        }
    }
}

/// Answer one `sendMessage` from the scripts: the threading refusal
/// first, then the send script. The threading refusal is read ahead and
/// consumes none of the send script — a threaded send never reached the
/// script's outcome — and it applies only to a send that carries reply
/// parameters, so a plain send, a retry included, is served normally.
fn send_answer(state: &Arc<ServerState>, body: &Value) -> (u16, Value) {
    let refusal = state
        .refused_threading
        .lock()
        .expect("the threading refusal locks")
        .clone();
    if let (Some((status, description)), Some(_)) = (refusal, body.get("reply_parameters")) {
        return (status, json!({ "ok": false, "description": description }));
    }
    let script = state
        .send_scripts
        .lock()
        .expect("the send scripts lock")
        .pop_front();
    match script {
        Some(SendScript::RateLimited(refusal)) => rate_limit_answer(&refusal),
        Some(SendScript::Failing) => (
            500,
            json!({ "ok": false, "description": "scripted send failure" }),
        ),
        Some(SendScript::Delivered) | None => {
            (200, json!({ "ok": true, "result": { "message_id": 1 } }))
        }
    }
}

/// Answer one request from the script, recording it first.
async fn dispatch(state: &Arc<ServerState>, method: String, body: Value) -> (u16, Value) {
    state
        .requests
        .lock()
        .expect("the request log locks")
        .push(Recorded {
            method: method.clone(),
            body: body.clone(),
        });
    match method.as_str() {
        "getMe" => {
            let me = state.me.lock().expect("the identity script locks").clone();
            match me {
                Some((id, username)) => (
                    200,
                    json!({ "ok": true, "result": {
                        "id": id,
                        "is_bot": true,
                        "first_name": "Fixture",
                        "username": username,
                    } }),
                ),
                None => (
                    500,
                    json!({ "ok": false, "description": "scripted identity failure" }),
                ),
            }
        }
        "getUpdates" => {
            let script = state
                .poll_scripts
                .lock()
                .expect("the poll scripts lock")
                .pop_front();
            match script {
                Some(refusal) => rate_limit_answer(&refusal),
                None => get_updates(state, &body).await,
            }
        }
        "sendMessage" => send_answer(state, &body),
        "getChat" => chat_info_answer(state, &body),
        "sendChatAction" => {
            if *state
                .failing_chat_actions
                .lock()
                .expect("the action script locks")
            {
                (
                    500,
                    json!({ "ok": false, "description": "scripted action failure" }),
                )
            } else {
                (200, json!({ "ok": true, "result": true }))
            }
        }
        "setWebhook" => registration_answer(state, &body),
        "deleteWebhook" => deletion_answer(state),
        "getChatAdministrators" => admin_answer(state, &body).await,
        // Every other method — the leave call above all — succeeds plainly;
        // the recorded request is what the assertions read.
        _ => (200, json!({ "ok": true, "result": true })),
    }
}

/// One chat's administrator list from the script. A hanging script parks
/// the request by re-reading the script on an interval instead of by a
/// permanent park, so a test can release the wedge — the shape a step that
/// outran the webhook door's deadline and then succeeded needs.
async fn admin_answer(state: &Arc<ServerState>, body: &Value) -> (u16, Value) {
    let chat_id = body["chat_id"].as_i64().unwrap_or_default();
    loop {
        // Read and released before any await: a held lock would stall every
        // other connection with this one.
        let script = state
            .admins
            .lock()
            .expect("the admin scripts lock")
            .get(&chat_id)
            .cloned();
        match script.as_ref() {
            Some(AdminScript::Hanging) => tokio::time::sleep(HANG_RECHECK).await,
            Some(AdminScript::Failing) => {
                return (
                    500,
                    json!({ "ok": false, "description": "scripted failure" }),
                );
            }
            Some(AdminScript::List(list)) => {
                let members: Vec<Value> = list
                    .iter()
                    .map(|(id, status)| json!({ "user": { "id": id }, "status": status }))
                    .collect();
                return (200, json!({ "ok": true, "result": members }));
            }
            None => return (200, json!({ "ok": true, "result": [] })),
        }
    }
}

/// The registration: it sets the one piece of webhook state the platform
/// keeps, or answers the scripted refusal.
///
/// That refusal is the platform's described shape — a success status
/// carrying `ok: false` — and its description echoes the secret it was
/// handed, the way a real description quotes the parameter it refused. The
/// combination is what makes the adapter's scrubbing of a refusal text a
/// real path instead of one nothing ever exercises: a refusal whose
/// description the client never reads would scrub nothing.
fn registration_answer(state: &Arc<ServerState>, body: &Value) -> (u16, Value) {
    if *state
        .failing_registration
        .lock()
        .expect("the registration script locks")
    {
        let offered = body["secret_token"].as_str().unwrap_or_default();
        return (
            200,
            json!({
                "ok": false,
                "description": format!("scripted registration failure for secret_token {offered}"),
            }),
        );
    }
    let url = body["url"].as_str().unwrap_or_default().to_owned();
    *state.webhook_url.lock().expect("the webhook state locks") = url;
    (200, json!({ "ok": true, "result": true }))
}

/// The deletion: it clears the registration whether or not anything was
/// there — the platform's own idempotence, which is why the polling start
/// asks nothing first — or answers the scripted failure.
fn deletion_answer(state: &Arc<ServerState>) -> (u16, Value) {
    if *state
        .failing_webhook_delete
        .lock()
        .expect("the webhook delete script locks")
    {
        return (
            500,
            json!({ "ok": false, "description": "scripted webhook delete failure" }),
        );
    }
    state
        .webhook_url
        .lock()
        .expect("the webhook state locks")
        .clear();
    (200, json!({ "ok": true, "result": true }))
}

/// The chat lookup's answer from the script: the chat's facts, or the
/// scripted failure an unscripted chat draws.
fn chat_info_answer(state: &Arc<ServerState>, body: &Value) -> (u16, Value) {
    let chat_id = body["chat_id"].as_i64().unwrap_or_default();
    let chats = state.chats.lock().expect("the chat scripts lock");
    match chats.get(&chat_id) {
        Some(script) => {
            let mut result = json!({ "id": chat_id, "type": "group" });
            if let Some(title) = &script.title {
                result["title"] = json!(title);
            }
            if let Some((date, text)) = &script.pinned {
                result["pinned_message"] = json!({
                    "message_id": 1,
                    "date": date,
                    "chat": { "id": chat_id, "type": "group" },
                    "text": text,
                });
            }
            (200, json!({ "ok": true, "result": result }))
        }
        None => (
            500,
            json!({ "ok": false, "description": "scripted lookup failure" }),
        ),
    }
}

/// The platform's rate-limit reply, the stated wait included.
fn rate_limit_answer(refusal: &RateLimited) -> (u16, Value) {
    (
        429,
        json!({
            "ok": false,
            "error_code": 429,
            "description": "Too Many Requests: retry later",
            "parameters": { "retry_after": refusal.retry_after },
        }),
    )
}

/// The update poll: confirm everything before the offset — destructively,
/// but only at request arrival, exactly when the platform acknowledges — and
/// answer what remains, after one scripted long-poll wait when nothing is
/// pending yet. A stale poll parked in the wait must not confirm an update
/// pushed after it arrived, so the post-wait read only filters. A stale
/// request already on the wire when its adapter is aborted still confirms
/// at arrival, though — a test that re-pushes an update below a finished
/// process run's offset must give the restarted adapter a fresh server,
/// out of the stale request's reach.
async fn get_updates(state: &Arc<ServerState>, body: &Value) -> (u16, Value) {
    let offset = body.get("offset").and_then(Value::as_i64);
    {
        let mut updates = state.updates.lock().expect("the update list locks");
        if let Some(offset) = offset {
            updates.retain(|update| update["update_id"].as_i64().unwrap_or(0) >= offset);
        }
    }
    for waited in [false, true] {
        let pending: Vec<Value> = {
            let updates = state.updates.lock().expect("the update list locks");
            updates
                .iter()
                .filter(|update| {
                    offset.is_none_or(|offset| update["update_id"].as_i64().unwrap_or(0) >= offset)
                })
                .cloned()
                .collect()
        };
        if !pending.is_empty() || waited {
            return (200, json!({ "ok": true, "result": pending }));
        }
        let _ = tokio::time::timeout(SCRIPTED_LONG_POLL, state.wake.notified()).await;
    }
    (200, json!({ "ok": true, "result": [] }))
}
