//! The scripted forge and mirror: one loopback HTTP server the lookup tools
//! are pointed at, following the pattern of the suite's other scripted
//! wires — a plain listener on the loopback interface, scripted answers,
//! recorded requests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// One recorded lookup request: the path asked and the request headers,
/// lowercased names — what the authorization pins read.
#[derive(Debug, Clone)]
pub struct RecordedLookup {
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl RecordedLookup {
    /// The value of one header, by its lowercase name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }
}

/// What the server answers.
#[derive(Clone)]
pub enum LookupAnswer {
    /// A JSON body under the given status.
    Json(u16, Value),
    /// A plain-text body under the given status — the raw wiki host's
    /// shape.
    Text(u16, String),
    /// Sleep first, then answer 200 with an empty object — long enough past
    /// a short constructed client bound to be a timeout there.
    Stall(Duration),
    /// A 302 pointing at the given location — what pins that a lookup
    /// treats a redirect as a tool error instead of following it.
    Redirect(String),
}

/// The scripted lookup server: every request is recorded, every answer
/// follows the one script it was started with.
pub struct LookupServer {
    base: String,
    requests: Arc<Mutex<Vec<RecordedLookup>>>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for LookupServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

impl LookupServer {
    pub async fn start(answer: LookupAnswer) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let accept_requests = Arc::clone(&requests);
        let accept_task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(serve(answer.clone(), Arc::clone(&accept_requests), stream));
            }
        });
        Self {
            base: format!("http://{addr}"),
            requests,
            accept_task,
        }
    }

    /// The base URL a tool is constructed over.
    pub fn base(&self) -> String {
        self.base.clone()
    }

    /// Every recorded request, in arrival order.
    pub fn requests(&self) -> Vec<RecordedLookup> {
        self.requests.lock().expect("the request log locks").clone()
    }
}

/// Serve one connection: read the request head, record it, answer per the
/// script. The lookups send GETs, so no body is read.
async fn serve(
    script: LookupAnswer,
    requests: Arc<Mutex<Vec<RecordedLookup>>>,
    mut stream: TcpStream,
) {
    let mut buffered: Vec<u8> = Vec::new();
    let header_end = loop {
        if let Some(position) = buffered.windows(4).position(|window| window == b"\r\n\r\n") {
            break position;
        }
        let mut chunk = [0_u8; 4096];
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => buffered.extend_from_slice(&chunk[..read]),
        }
    };
    let head = String::from_utf8_lossy(&buffered[..header_end]).into_owned();
    let mut lines = head.lines();
    let path = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or_default()
        .to_owned();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_lowercase(), value.trim().to_owned()))
        })
        .collect();
    requests
        .lock()
        .expect("the request log locks")
        .push(RecordedLookup { path, headers });

    let (status, location, content_type, body) = match script {
        LookupAnswer::Json(status, body) => (status, None, "application/json", body.to_string()),
        LookupAnswer::Text(status, body) => (status, None, "text/plain; charset=utf-8", body),
        LookupAnswer::Stall(wait) => {
            tokio::time::sleep(wait).await;
            (200, None, "application/json", "{}".to_owned())
        }
        LookupAnswer::Redirect(location) => {
            (302, Some(location), "application/json", "{}".to_owned())
        }
    };
    let location_header =
        location.map_or_else(String::new, |location| format!("Location: {location}\r\n"));
    let response = format!(
        "HTTP/1.1 {status} Scripted\r\nContent-Type: {content_type}\r\n\
         {location_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
