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

/// What the administrator list answers for one chat.
enum AdminScript {
    List(Vec<(i64, String)>),
    Failing,
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

    /// Script one chat's administrator list to fail until re-scripted.
    pub fn fail_admins(&self, chat_id: i64) {
        self.state
            .admins
            .lock()
            .expect("the admin scripts lock")
            .insert(chat_id, AdminScript::Failing);
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
        "sendMessage" => {
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
        "getChatAdministrators" => {
            let chat_id = body["chat_id"].as_i64().unwrap_or_default();
            let admins = state.admins.lock().expect("the admin scripts lock");
            match admins.get(&chat_id) {
                Some(AdminScript::Failing) => (
                    500,
                    json!({ "ok": false, "description": "scripted failure" }),
                ),
                Some(AdminScript::List(list)) => {
                    let members: Vec<Value> = list
                        .iter()
                        .map(|(id, status)| json!({ "user": { "id": id }, "status": status }))
                        .collect();
                    (200, json!({ "ok": true, "result": members }))
                }
                None => (200, json!({ "ok": true, "result": [] })),
            }
        }
        _ => (200, json!({ "ok": true, "result": true })),
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
