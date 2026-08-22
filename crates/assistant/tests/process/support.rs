//! Fixtures for the process-level suite: a scratch directory per test, the
//! two scripted loopback servers, and the compiled binary run as a child
//! process.
//!
//! Every server binds the loopback interface on a fresh port and the
//! configuration under test points at it, so no traffic leaves the machine.
//! The secrets are fakes with distinctive spellings; the scans assert those
//! exact strings never reach the store file or a log line.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

/// How long an awaited condition may take before the test names a stall.
pub const DEADLINE: Duration = Duration::from_secs(30);

/// The stated bound the stop criterion holds the process to: SIGTERM to a
/// clean exit.
pub const STOP_BOUND: Duration = Duration::from_secs(10);

/// The fake bot token. Nothing real; the scans look for this exact string.
pub const TOKEN: &str = "0000000000:FAKE-PROCESS-TEST-TOKEN";

/// The fake provider key. Nothing real; the scans look for this exact
/// string.
pub const KEY: &str = "sk-or-FAKE-PROCESS-TEST-KEY";

/// The fake mirror token. Nothing real; the scans look for this exact
/// string.
pub const MIRROR_TOKEN: &str = "ghp-FAKE-PROCESS-TEST-MIRROR-TOKEN";

/// The one answer the scripted completions server streams.
pub const ANSWER: &str = "The scripted process answer.";

/// A loopback address nothing listens on: what a configuration names for a
/// lookup host its run never calls, so an accidental call fails fast on the
/// loopback instead of reaching a real host.
pub const UNROUTABLE: &str = "http://127.0.0.1:1";

/// A unique directory in the temp location, removed with its content on
/// drop, so parallel tests never share files and no run leaves litter.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Self {
        let unique = format!(
            "assistant-process-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the clock is past the epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("the scratch directory creates");
        Self(dir)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Write one file into the scratch directory and answer its path.
    pub fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the parent directory creates");
        }
        std::fs::write(&path, content).expect("the scratch file writes");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One recorded request: the method asked and its decoded body.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub body: Value,
}

#[derive(Default)]
struct TelegramState {
    /// Every unconfirmed update, in push order; a poll's offset confirms.
    updates: Mutex<Vec<Value>>,
    wake: Notify,
    requests: Mutex<Vec<Recorded>>,
}

/// The scripted Bot API server: `getMe`, the acknowledgement-honouring
/// `getUpdates`, and a recording `sendMessage`.
pub struct TelegramServer {
    root: String,
    state: Arc<TelegramState>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for TelegramServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

impl TelegramServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let state = Arc::new(TelegramState::default());
        let accept_state = Arc::clone(&state);
        let accept_task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(serve_telegram(Arc::clone(&accept_state), stream));
            }
        });
        Self {
            root: format!("http://{addr}"),
            state,
            accept_task,
        }
    }

    pub fn root(&self) -> String {
        self.root.clone()
    }

    pub fn push_update(&self, update: Value) {
        self.state
            .updates
            .lock()
            .expect("the update list locks")
            .push(update);
        self.state.wake.notify_waiters();
    }

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

    pub async fn await_recorded(&self, method: &str, count: usize) -> Vec<Recorded> {
        let deadline = Instant::now() + DEADLINE;
        loop {
            let recorded = self.recorded(method);
            if recorded.len() >= count {
                return recorded;
            }
            assert!(
                Instant::now() < deadline,
                "timed out awaiting {count} recorded {method} requests; have {}",
                recorded.len()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// One update carrying a private-chat message; the chat id is the person's
/// own id, as on the platform.
#[must_use]
pub fn private_update(update_id: i64, user_id: i64, text: &str) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": update_id + 1000,
            "date": 1_700_000_000 + update_id,
            "chat": { "id": user_id, "type": "private" },
            "from": { "id": user_id, "first_name": format!("Person {user_id}") },
            "text": text,
        },
    })
}

/// Serve one Bot API connection: HTTP/1.1 keep-alive, one answer per
/// request.
async fn serve_telegram(state: Arc<TelegramState>, mut stream: TcpStream) {
    let mut buffered: Vec<u8> = Vec::new();
    loop {
        let Some((method, body, consumed)) = read_request(&mut stream, &mut buffered).await else {
            return;
        };
        buffered.drain(..consumed);
        state
            .requests
            .lock()
            .expect("the request log locks")
            .push(Recorded {
                method: method.clone(),
                body: body.clone(),
            });
        let answer = match method.as_str() {
            "getMe" => json!({ "ok": true, "result": {
                "id": 999_000,
                "is_bot": true,
                "first_name": "Fixture",
                "username": "assistant_process_bot",
            } }),
            "getUpdates" => get_updates(&state, &body).await,
            "sendMessage" => json!({ "ok": true, "result": { "message_id": 1 } }),
            _ => json!({ "ok": true, "result": true }),
        };
        let payload = answer.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: keep-alive\r\n\r\n{payload}",
            payload.len()
        );
        if stream.write_all(response.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// The update poll: confirm everything before the offset, then answer what
/// remains, after one short wait when nothing is pending yet.
async fn get_updates(state: &Arc<TelegramState>, body: &Value) -> Value {
    let offset = body.get("offset").and_then(Value::as_i64);
    if let Some(offset) = offset {
        state
            .updates
            .lock()
            .expect("the update list locks")
            .retain(|update| update["update_id"].as_i64().unwrap_or(0) >= offset);
    }
    for waited in [false, true] {
        let pending: Vec<Value> = state.updates.lock().expect("the update list locks").clone();
        if !pending.is_empty() || waited {
            return json!({ "ok": true, "result": pending });
        }
        let _ = tokio::time::timeout(Duration::from_millis(200), state.wake.notified()).await;
    }
    json!({ "ok": true, "result": [] })
}

/// The scripted completions server: every POST answers with server-sent
/// events and the end marker, recording the request's decoded body — what
/// lets a test join the prompt files to the wire. In the default mode every
/// turn streams [`ANSWER`]; with a tool script, the opening turn streams
/// one tool call and the turn whose request carries the tool's answer
/// streams [`ANSWER`] — scripted by request content, like the suites' other
/// scripted providers.
pub struct CompletionsServer {
    base: String,
    requests: Arc<Mutex<Vec<Value>>>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for CompletionsServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

/// What one scripted completion answers: the text delta stream, or one tool
/// call with the given name and arguments JSON.
fn completion_events(tool_script: Option<&(String, String)>, body: &Value) -> String {
    if let Some((tool, arguments)) = tool_script {
        let request = body.to_string();
        // A request already carrying a tool-voiced message is the closing
        // turn; anything else — the title derivation included, which never
        // acts on tools — gets the scripted call.
        if !request.contains("\"role\":\"tool\"") {
            let call = serde_json::json!({
                "choices": [{ "delta": { "tool_calls": [{
                    "index": 0,
                    "id": "call-1",
                    "type": "function",
                    "function": { "name": tool, "arguments": arguments }
                }]}}]
            });
            return format!(
                "data: {call}\n\n\
                 data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}]}}\n\n\
                 data: [DONE]\n\n"
            );
        }
    }
    format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{ANSWER}\"}}}}]}}\n\n\
         data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\n\
         data: [DONE]\n\n"
    )
}

impl CompletionsServer {
    pub async fn start() -> Self {
        Self::start_scripted(None).await
    }

    /// Start with an optional tool script: the tool's registered name and
    /// its arguments JSON, sent as one scripted call on every opening turn.
    pub async fn start_scripted(tool_script: Option<(String, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let accept_requests = Arc::clone(&requests);
        let accept_task = tokio::spawn(async move {
            let tool_script = tool_script;
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let requests = Arc::clone(&accept_requests);
                let tool_script = tool_script.clone();
                tokio::spawn(async move {
                    let mut buffered = Vec::new();
                    let Some((_, body, _)) = read_request(&mut stream, &mut buffered).await else {
                        return;
                    };
                    let events = completion_events(tool_script.as_ref(), &body);
                    requests.lock().expect("the request log locks").push(body);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{events}",
                        events.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            base: format!("http://{addr}"),
            requests,
            accept_task,
        }
    }

    pub fn base(&self) -> String {
        self.base.clone()
    }

    pub fn hit_count(&self) -> usize {
        self.requests().len()
    }

    /// Every completion request's decoded body, in arrival order.
    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("the request log locks").clone()
    }
}

/// One request a scripted lookup endpoint recorded: its path and its
/// authorization header, if one was sent — what the token pins read.
#[derive(Debug, Clone)]
pub struct LookupRequest {
    pub path: String,
    pub authorization: Option<String>,
}

/// A scripted lookup endpoint — the forge or the mirror: answers every GET
/// with the one JSON body it was started with, recording each request's
/// path and authorization header.
pub struct LookupServer {
    base: String,
    requests: Arc<Mutex<Vec<LookupRequest>>>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for LookupServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

impl LookupServer {
    pub async fn start(body: Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let accept_requests = Arc::clone(&requests);
        let accept_task = tokio::spawn(async move {
            let body = body.to_string();
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let requests = Arc::clone(&accept_requests);
                let body = body.clone();
                tokio::spawn(async move {
                    let mut buffered: Vec<u8> = Vec::new();
                    let header_end = loop {
                        if let Some(position) =
                            buffered.windows(4).position(|window| window == b"\r\n\r\n")
                        {
                            break position;
                        }
                        if read_more(&mut stream, &mut buffered).await.is_none() {
                            return;
                        }
                    };
                    let head = String::from_utf8_lossy(&buffered[..header_end]).into_owned();
                    let path = head
                        .lines()
                        .next()
                        .and_then(|line| line.split(' ').nth(1))
                        .unwrap_or_default()
                        .to_owned();
                    let authorization = head.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("authorization")
                            .then(|| value.trim().to_owned())
                    });
                    requests
                        .lock()
                        .expect("the request log locks")
                        .push(LookupRequest {
                            path,
                            authorization,
                        });
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            base: format!("http://{addr}"),
            requests,
            accept_task,
        }
    }

    pub fn base(&self) -> String {
        self.base.clone()
    }

    pub fn requests(&self) -> Vec<LookupRequest> {
        self.requests.lock().expect("the request log locks").clone()
    }

    pub async fn await_requests(&self, count: usize) -> Vec<LookupRequest> {
        let deadline = Instant::now() + DEADLINE;
        loop {
            let recorded = self.requests();
            if recorded.len() >= count {
                return recorded;
            }
            assert!(
                Instant::now() < deadline,
                "timed out awaiting {count} lookup requests; have {}",
                recorded.len()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// Read one whole request; answer the method name (the path's last segment),
/// the decoded body, and how many buffered bytes were consumed. `None` when
/// the peer closed.
async fn read_request(
    stream: &mut TcpStream,
    buffered: &mut Vec<u8>,
) -> Option<(String, Value, usize)> {
    let header_end = loop {
        if let Some(position) = buffered.windows(4).position(|window| window == b"\r\n\r\n") {
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

/// The compiled binary as a child process, killed on drop so a failed
/// assertion never leaks a poller into the next test.
pub struct BinaryRun {
    child: Child,
}

impl BinaryRun {
    /// Start the binary with the given arguments and environment additions,
    /// stderr captured into the named file.
    pub fn spawn(arguments: &[&Path], env: &[(&str, &str)], stderr_file: &Path) -> Self {
        let stderr = std::fs::File::create(stderr_file).expect("the stderr capture file creates");
        let mut command = Command::new(env!("CARGO_BIN_EXE_assistant"));
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr));
        for (name, value) in env {
            command.env(name, value);
        }
        Self {
            child: command.spawn().expect("the binary spawns"),
        }
    }

    /// Send SIGTERM to the child.
    pub fn terminate(&self) {
        let delivered = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .expect("the kill utility runs");
        assert!(delivered.success(), "SIGTERM must reach the child");
    }

    /// Await the child's exit within the bound, or name the stall.
    pub async fn wait_exit(&mut self, bound: Duration) -> ExitStatus {
        let deadline = Instant::now() + bound;
        loop {
            if let Some(status) = self.child.try_wait().expect("the child's status reads") {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "the process did not exit within {bound:?}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for BinaryRun {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Assert one secret string appears nowhere in the named file. The file
/// must exist: a scan whose target is missing proves nothing, and a store
/// path that silently changed shape would otherwise pass the scan vacuously.
pub fn assert_absent(path: &Path, secret: &str, what: &str) {
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "the scanned file {} must exist for the scan to mean anything: {error}",
            path.display()
        )
    });
    assert!(
        !bytes
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()),
        "{what} must not appear in {}",
        path.display()
    );
}

/// Scan a file that may legitimately be absent — the store's `-wal` and
/// `-shm` sidecars are merged away when the store closes cleanly. Only the
/// sidecars go through here; every primary file gets [`assert_absent`] and
/// its existence check.
pub fn assert_absent_if_present(path: &Path, secret: &str, what: &str) {
    if path.exists() {
        assert_absent(path, secret, what);
    }
}
