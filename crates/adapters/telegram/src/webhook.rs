//! The webhook intake: the platform pushes each update to a loopback door,
//! and the response code is the acknowledgement (2026-08-29).
//!
//! This module is the second feeder of the shared per-update step
//! ([`crate::driver::Intake`]); it decides nothing about updates. The door
//! authenticates a delivery by its secret token, bounds and parses it, and
//! hands it to ONE consumer over a bounded queue with a one-shot answer
//! channel. The consumer owns the step and takes deliveries strictly one at a
//! time — the same serial discipline the poll loop keeps — and its outcome
//! becomes the status code: 200 exactly on [`Step::Acknowledged`], 500
//! otherwise, so the platform's own retry plays the part the offset file
//! plays for polling. The offset file is untouched here: it is neither read
//! nor written.
//!
//! What the door refuses, it refuses without reading anything into the
//! pipeline: another path is 404, another method 405 naming the one method it
//! takes, a delivery without the right secret 403, a body past the bound 413,
//! one that does not parse 400, and a full queue 503 — honest backpressure the
//! platform's retry absorbs. Every refusal that asks the platform to come back
//! again (500, 503) leaves one structural line naming the update id and the
//! reason, never content, so an update the platform eventually gives up on
//! leaves its trail.
//!
//! What the door serves is bounded in the same spirit: at most
//! [`MAX_CONNECTIONS`] connections at once, each with
//! [`HEAD_READ_TIMEOUT`] to send the head of its next request. A reverse
//! proxy forwards the public internet to this port, and neither an unbounded
//! spawn nor a socket that opens and says nothing may cost the deployment
//! anything past what those two numbers state.
//!
//! TLS, the hostname and the public path belong to the reverse proxy in front
//! of this listener; the door speaks plain HTTP and binds loopback only.

use std::collections::{HashSet, VecDeque};
use std::convert::Infallible;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use assistant_core::Assistant;
use http_body_util::{BodyExt, Empty, LengthLimitError, Limited};
use hyper::body::Incoming;
use hyper::header::{ALLOW, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use crate::client::{BotClient, Update};
use crate::driver::{Feeder, Intake, Step};
use crate::state::{self, SecretToken};
use crate::{AdapterError, BoundListener, Sleep, WebhookConfig};

/// How many deliveries wait between the door and the consumer. The local
/// queue only smooths bursts between consumer steps — an ingest is local and
/// quick — while the platform's own day-long queue with its retries is the
/// real buffer; a deeper queue would hide a wedged consumer longer without
/// saving a single update.
const QUEUE_DEPTH: usize = 64;

/// The longest the door waits for the consumer's outcome before answering
/// that the platform should come back. Past it, a wedged platform call inside
/// the step would pin HTTP connections open until the platform's delivery
/// pool fills and the deployment goes deaf while looking busy.
///
/// What the step awaits is stated exactly, because the bound only makes
/// sense against it: the translation is pure, the platform calls the step
/// makes carry the client's own request timeout and its rate-limit waits,
/// the core's ingest appends the message and returns, and a rules delta
/// draws one completion the core bounds itself. The ANSWER TURN is never
/// awaited here — the append emits it onto the ledger's event edge and the
/// reply arrives later on the outbound edge — so the acknowledgement this
/// deadline bounds is durable acceptance, never a finished answer.
///
/// A step slower than the bound that then succeeds loses nothing: the door
/// has already answered 500, the consumer finishes the step and records the
/// update id, the platform redelivers, and the redelivery meets
/// [`Acknowledged`] and is answered 200 without a second ingest. Convergence,
/// at the cost of one redelivery.
const ANSWER_DEADLINE: Duration = Duration::from_secs(25);

/// The most body bytes read from one delivery. An update is kilobytes; a
/// megabyte is room to spare, and past it the door answers 413 instead of
/// reading whatever arrives.
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// How many acknowledged update ids the consumer remembers, evicting the
/// OLDEST at the cap: clearing the memory whole would re-open the duplicate
/// window for exactly the ids most likely to be retried.
const ACKNOWLEDGED_MEMORY: usize = 1024;

/// The header every delivery carries the secret token back in.
const SECRET_HEADER: &str = "x-telegram-bot-api-secret-token";

/// How long the accept loop rests after a failed accept, so a listener whose
/// accepts keep failing does not spin.
const ACCEPT_REST: Duration = Duration::from_secs(1);

/// The most delivery connections served at once. A reverse proxy forwards
/// the public internet to this port, so a task spawned per accepted socket
/// with no cap is a bill anyone who finds the port could write. At the cap
/// the loop does not accept at all: the waiting sockets stay in the kernel's
/// backlog until a connection finishes, and whatever the backlog turns away
/// is a delivery the platform retries.
///
/// It is derived from [`QUEUE_DEPTH`] and must stay above it. A cap at or
/// below the depth would make the queue's own 503 unreachable — no more
/// deliveries could be in flight than the queue holds — and would replace
/// honest backpressure with a connection that is simply never accepted. Two
/// deep leaves room for the refusals that never queue at all: a wrong path,
/// a wrong method, a wrong secret, a body past the bound.
const MAX_CONNECTIONS: usize = 2 * QUEUE_DEPTH;

/// The longest one connection may go without a request head arriving: the
/// wait for its first request, the wait for the next one on a kept-alive
/// connection, and the read of a head sent a byte at a time. Past it the
/// connection is closed and its place under [`MAX_CONNECTIONS`] returns, so
/// a peer that opens sockets and says nothing cannot hold the door's
/// capacity. It sits above [`ANSWER_DEADLINE`] so a connection whose
/// delivery is merely slow is never cut while its answer is still coming.
const HEAD_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether a refused secret has already been reported at warning level in
/// this process. A scanner knocking in a loop would otherwise write one
/// warning per hit and bury everything else in the log, so the first
/// refusal warns and every later one is a debug line.
static SECRET_REFUSAL_WARNED: AtomicBool = AtomicBool::new(false);

/// The public address the platform is told to call, parsed once: the whole
/// address travels to the platform, and its PATH is what the listener
/// answers — one recorded value, so the address called and the path served
/// cannot diverge.
#[derive(Debug, Clone)]
pub struct WebhookAddress {
    url: String,
    path: String,
}

/// The scheme the platform calls and the only one this address takes.
const HTTPS_SCHEME: &str = "https";

impl WebhookAddress {
    /// The address a deployment configured, refused unless it is an `https`
    /// address with a host and a path and nothing else — the platform
    /// requires HTTPS, and user information, a query string or a fragment
    /// would be a part of the address the listener could not match a
    /// delivery against.
    ///
    /// The grammar is the one the HTTP client already parses every address
    /// with, so the authority, the path and everything refused above are
    /// read by the parser instead of by a second reading of the same grammar
    /// written here. The recorded address is the parser's own rendering, so
    /// the address handed to the platform and the path matched against a
    /// delivery come from one parse and cannot disagree.
    ///
    /// The root is not a path: an address whose path is `/` — written that
    /// way or left off entirely — is refused, because a door at a host's
    /// root answers for everything else that host serves.
    ///
    /// # Errors
    ///
    /// [`WebhookAddressError`], naming which of those the address is missing.
    pub fn parse(address: &str) -> Result<Self, WebhookAddressError> {
        let address = address.trim();
        let parsed = reqwest::Url::parse(address).map_err(|_| {
            // Past the scheme, what the parser refuses in an address
            // carrying no query and no fragment is its authority — an
            // address that names https and still does not parse names no
            // host the platform could resolve.
            if address.to_ascii_lowercase().starts_with(HTTPS_SCHEME) {
                WebhookAddressError::NoHost
            } else {
                WebhookAddressError::NotHttps
            }
        })?;
        if parsed.scheme() != HTTPS_SCHEME {
            return Err(WebhookAddressError::NotHttps);
        }
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(WebhookAddressError::NotBare);
        }
        if parsed.host_str().is_none_or(str::is_empty) {
            return Err(WebhookAddressError::NoHost);
        }
        if parsed.path() == "/" {
            return Err(WebhookAddressError::NoPath);
        }
        Ok(Self {
            path: parsed.path().to_owned(),
            url: parsed.as_str().to_owned(),
        })
    }

    /// The whole address, as the registration hands it to the platform.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The path component, as the listener matches a delivery against it.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Why a configured public address is not one the platform can call.
#[derive(Debug, thiserror::Error)]
pub enum WebhookAddressError {
    /// Not an `https` address; the platform calls nothing else.
    #[error("the address must begin with https://")]
    NotHttps,
    /// No host between the scheme and the path.
    #[error("the address names no host")]
    NoHost,
    /// No path for the listener to answer on, the bare host and the root
    /// path alike.
    #[error("the address names no path past the root")]
    NoPath,
    /// User information, a query string or a fragment — none of which a
    /// delivery would carry back.
    #[error("the address must carry no user information, no query string and no fragment")]
    NotBare,
}

/// One delivery on its way to the consumer: the update, and the one-shot
/// channel the outcome — and with it the status code — comes back on.
struct Delivery {
    update: Update,
    answer: oneshot::Sender<Step>,
}

/// Start the webhook intake: identity, secret, bind, announce, register — a
/// refusal at any of them refuses the start.
///
/// The order is the spec's. The identity comes first because the shared step
/// cannot translate without it, and translating blind would record wrong
/// facts into a durable ledger. The secret follows, because the address is
/// registered with it: a secret that can be neither generated nor kept
/// leaves nothing to authenticate a delivery by across the next restart. The
/// listener binds third, so the platform is never pointed at a port nothing
/// serves, and the address it bound is announced to whoever asked for it.
/// Registration comes last and refuses loudly, because a webhook deployment
/// that silently cannot register sits deaf; a registration left by an
/// earlier run is simply overwritten.
///
/// # Errors
///
/// [`AdapterError::Identity`] when the identity read fails,
/// [`AdapterError::Secret`] when no secret can be generated or persisted,
/// [`AdapterError::Listener`] when the port does not bind, and
/// [`AdapterError::Registration`] when the platform refuses the address.
pub(crate) async fn start<'a>(
    config: &'a WebhookConfig,
    state_file: &Path,
    client: &'a BotClient,
    sleep: &'a Sleep,
    assistant: &'a Assistant,
    wake: Option<&'a str>,
    announce_bound: Option<&BoundListener>,
) -> Result<Feeder<'a>, AdapterError> {
    let me = client
        .get_me()
        .await
        .map_err(|error| AdapterError::Identity(error.to_string()))?;
    let secret = Arc::new(
        state::webhook_secret(state_file)
            .map_err(|error| AdapterError::Secret(error.to_string()))?,
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.listen_port))
        .await
        .map_err(|error| AdapterError::Listener {
            port: config.listen_port,
            detail: error.to_string(),
        })?;
    let bound = listener
        .local_addr()
        .map_err(|error| AdapterError::Listener {
            port: config.listen_port,
            detail: error.to_string(),
        })?;
    tracing::info!(%bound, "the webhook listener is bound");
    if let Some(announce) = announce_bound {
        announce(bound);
    }
    client
        .set_webhook(config.address.url(), secret.expose())
        .await
        .map_err(|error| AdapterError::Registration(secret.scrubbed(&error.to_string())))?;
    tracing::info!(address = %config.address.url(), "the webhook address is registered");
    let (deliveries, queued) = mpsc::channel(QUEUE_DEPTH);
    let door = Door {
        path: Arc::from(config.address.path()),
        secret,
        deliveries,
        sleep: Arc::clone(sleep),
    };
    let intake = Intake::new(me, wake, client, assistant);
    // Both halves run in the returned feeder, and either one ending ends it:
    // a consumer that died with a live listener would refuse every delivery
    // at the deadline forever, and a deaf webhook deployment must never keep
    // running quietly. The run entry's select then ends the run, and the
    // supervisor restarts the process.
    Ok(Box::pin(async move {
        tokio::select! {
            () = serve(listener, door) => Ok(()),
            outcome = consume(queued, intake) => outcome,
        }
    }))
}

/// The door every delivery arrives at, shared by the connections: what to
/// match, what to authenticate against, where to queue, and the wait the
/// answer is bounded by.
#[derive(Clone)]
struct Door {
    path: Arc<str>,
    secret: Arc<SecretToken>,
    deliveries: mpsc::Sender<Delivery>,
    sleep: Sleep,
}

/// Accept connections and serve each one until the future is dropped, at
/// most [`MAX_CONNECTIONS`] at a time. The connections are tracked instead of
/// forgotten, so dropping this future takes every open delivery down with it
/// and a finished connection is reaped instead of accumulating.
async fn serve(listener: TcpListener, door: Door) {
    let mut connections = tokio::task::JoinSet::new();
    loop {
        if connections.len() >= MAX_CONNECTIONS {
            // At the cap the accept is not attempted at all: one connection
            // must finish before another is taken, so what the door holds
            // open is this number and not whatever a peer opens.
            if let Some(finished) = connections.join_next().await {
                report_ended(finished);
            }
            continue;
        }
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    connections.spawn(serve_connection(stream, door.clone()));
                }
                Err(error) => {
                    tracing::warn!(%error, "the webhook listener did not accept; resting");
                    (door.sleep)(ACCEPT_REST).await;
                }
            },
            Some(finished) = connections.join_next() => report_ended(finished),
        }
    }
}

/// Report a connection task that ended abnormally; a cancelled one is the
/// ordinary shutdown and says nothing.
fn report_ended(finished: Result<(), tokio::task::JoinError>) {
    if let Err(error) = finished
        && !error.is_cancelled()
    {
        tracing::warn!(%error, "a delivery connection ended abnormally");
    }
}

/// One connection: HTTP/1.1, every request on it answered by the door, and
/// [`HEAD_READ_TIMEOUT`] on the wait for each request's head — the bound
/// that keeps a silent peer from holding a place under [`MAX_CONNECTIONS`].
/// The timer is the runtime's own and not the injectable sleep: the test
/// seam answers a wait at once, which would close every connection before
/// its request arrived.
async fn serve_connection(stream: tokio::net::TcpStream, door: Door) {
    let service = service_fn(move |request| {
        let door = door.clone();
        async move { Ok::<_, Infallible>(door.answer(request).await) }
    });
    if let Err(error) = http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(HEAD_READ_TIMEOUT)
        .serve_connection(TokioIo::new(stream), service)
        .await
    {
        // A peer that hangs up mid-request is ordinary, and so is one the
        // head timeout closed; neither says anything about the door.
        tracing::debug!(%error, "a delivery connection ended");
    }
}

/// A status-only answer — every answer this door gives. What the platform
/// reads is the code; a body would only describe the door to whoever knocked.
fn answered(status: StatusCode) -> Response<Empty<&'static [u8]>> {
    Response::builder()
        .status(status)
        .body(Empty::new())
        .expect("a status-only response builds")
}

/// The refusal a method other than the door's own earns. The one answer
/// carrying a header: a 405 without `Allow` leaves the caller to guess which
/// method it should have used, and the door takes exactly one.
fn method_not_allowed() -> Response<Empty<&'static [u8]>> {
    let mut response = answered(StatusCode::METHOD_NOT_ALLOWED);
    response
        .headers_mut()
        .insert(ALLOW, HeaderValue::from_static("POST"));
    response
}

/// Report one delivery refused for its secret: the first refusal of the
/// process warns and names the level the rest arrive at, every later one is
/// a debug line. Counter-grade either way — the door discards strangers
/// without describing itself.
fn report_refused_secret() {
    if SECRET_REFUSAL_WARNED.swap(true, Ordering::Relaxed) {
        tracing::debug!("a delivery carried no valid secret token; discarded");
    } else {
        tracing::warn!(
            "a delivery carried no valid secret token; discarded — \
             further refusals are logged at debug level"
        );
    }
}

impl Door {
    /// One request: matched, authenticated, bounded, parsed, queued, and
    /// answered with what the consumer made of it.
    async fn answer(&self, request: Request<Incoming>) -> Response<Empty<&'static [u8]>> {
        if request.uri().path() != &*self.path || request.uri().query().is_some() {
            return answered(StatusCode::NOT_FOUND);
        }
        if request.method() != Method::POST {
            return method_not_allowed();
        }
        let offered = request
            .headers()
            .get(SECRET_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !self.secret.matches(offered) {
            report_refused_secret();
            return answered(StatusCode::FORBIDDEN);
        }
        let update = match self.read_update(request.into_body()).await {
            Ok(update) => update,
            Err(status) => return answered(status),
        };
        self.hand_over(update).await
    }

    /// The delivered update, or the status its body earns: a body past the
    /// bound is 413 and one that does not parse is 400 — bounded refusals,
    /// never a read of whatever arrives and never a panic.
    async fn read_update(&self, body: Incoming) -> Result<Update, StatusCode> {
        let collected = Limited::new(body, MAX_BODY_BYTES)
            .collect()
            .await
            .map_err(|error| {
                if error.downcast_ref::<LengthLimitError>().is_some() {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                }
            })?;
        serde_json::from_slice(&collected.to_bytes()).map_err(|_| StatusCode::BAD_REQUEST)
    }

    /// Queue one update for the consumer and answer with its outcome: 200
    /// exactly on an acknowledged step, and otherwise a status that asks the
    /// platform to deliver it again.
    async fn hand_over(&self, update: Update) -> Response<Empty<&'static [u8]>> {
        let update_id = update.id;
        let (answer, outcome) = oneshot::channel();
        if let Err(error) = self.deliveries.try_send(Delivery { update, answer }) {
            return match error {
                mpsc::error::TrySendError::Full(_) => {
                    tracing::warn!(
                        update_id,
                        "the delivery queue is full; refused for redelivery"
                    );
                    answered(StatusCode::SERVICE_UNAVAILABLE)
                }
                mpsc::error::TrySendError::Closed(_) => {
                    tracing::warn!(update_id, "the consumer is gone; refused for redelivery");
                    answered(StatusCode::INTERNAL_SERVER_ERROR)
                }
            };
        }
        tokio::select! {
            reported = outcome => match reported {
                Ok(Step::Acknowledged) => answered(StatusCode::OK),
                // Halted and stopped are one answer to the platform: the
                // update was not handled, so it redelivers. Which of the two
                // it was decides what the consumer does next, not what this
                // delivery is told.
                Ok(Step::Halted | Step::Stopped) => {
                    tracing::warn!(update_id, "the ingest failed; refused for redelivery");
                    answered(StatusCode::INTERNAL_SERVER_ERROR)
                }
                Err(_) => {
                    tracing::warn!(update_id, "the outcome was lost; refused for redelivery");
                    answered(StatusCode::INTERNAL_SERVER_ERROR)
                }
            },
            () = (self.sleep)(ANSWER_DEADLINE) => {
                tracing::warn!(
                    update_id,
                    "the outcome did not arrive within the deadline; refused for redelivery"
                );
                answered(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// The one consumer: take queued deliveries strictly one at a time through
/// the shared step, remember what was acknowledged, and report each outcome
/// back to the door that is waiting on it.
///
/// Deliveries are taken in ARRIVAL order, not update-id order: a refused
/// older update that redelivers after its successors were ingested appends
/// late, the way a late-delivered message reads to a human. The ledger
/// records arrival truth and nothing re-sorts.
///
/// It ends by itself only when the core states that it cannot serve anything
/// further (2026-09-01). The waiting delivery is refused before that, and
/// where the run ends before the refusal reaches the wire the connection is
/// cut instead — the two are the same fact to the platform, which holds an
/// update nothing acknowledged and redelivers it to the replacement process.
async fn consume(
    mut queued: mpsc::Receiver<Delivery>,
    mut intake: Intake<'_>,
) -> Result<(), AdapterError> {
    let mut acknowledged = Acknowledged::new();
    while let Some(delivery) = queued.recv().await {
        let update_id = delivery.update.id;
        let step = if acknowledged.holds(update_id) {
            // A retry that raced its original, or one that arrived after it:
            // because the step is serial, the retry queued behind its
            // original and meets what the original left here.
            tracing::debug!(
                update_id,
                "a duplicate of an acknowledged update; not ingested again"
            );
            Step::Acknowledged
        } else {
            let step = intake.take(&delivery.update).await;
            if step == Step::Acknowledged {
                acknowledged.record(update_id);
            }
            step
        };
        if delivery.answer.send(step).is_err() {
            tracing::debug!(update_id, "the delivery's answer was no longer awaited");
        }
        if step == Step::Stopped {
            return Err(AdapterError::CoreCannotServe);
        }
    }
    Ok(())
}

/// The bounded memory of acknowledged update ids: the ids in arrival order
/// beside the set they are looked up in, so the OLDEST leaves at the cap.
///
/// Its window is this process and these [`ACKNOWLEDGED_MEMORY`] ids, and
/// nothing wider: a restart forgets the memory whole, and an id evicted at
/// the cap is forgotten too, so a redelivery past either re-ingests one
/// update. That is at-least-once, the promise both intakes make — reached
/// differently than the poll intake's, whose offset file survives a restart
/// and whose window is therefore the writes it missed, not every id
/// this process acknowledged.
struct Acknowledged {
    order: VecDeque<i64>,
    ids: HashSet<i64>,
}

impl Acknowledged {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            ids: HashSet::new(),
        }
    }

    /// Whether this update was already acknowledged inside the memory.
    fn holds(&self, update_id: i64) -> bool {
        self.ids.contains(&update_id)
    }

    /// Remember one acknowledged update, evicting the oldest at the cap.
    fn record(&mut self, update_id: i64) {
        if !self.ids.insert(update_id) {
            return;
        }
        self.order.push_back(update_id);
        if self.order.len() > ACKNOWLEDGED_MEMORY
            && let Some(oldest) = self.order.pop_front()
        {
            self.ids.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one recorded value: the whole address goes to the platform and
    /// its path is what the door matches, so the two cannot diverge.
    #[test]
    fn an_address_yields_the_platform_url_and_the_listener_path() {
        let address = WebhookAddress::parse("  https://xenia.example.org/telegram/webhook  ")
            .expect("a plain https address with a path parses");
        assert_eq!(address.url(), "https://xenia.example.org/telegram/webhook");
        assert_eq!(address.path(), "/telegram/webhook");
    }

    /// Everything the platform could not call, or the door could not match,
    /// is refused where it is configured instead of at the first delivery.
    #[test]
    fn an_address_the_platform_cannot_call_is_refused() {
        for address in [
            "http://xenia.example.org/hook",
            "xenia.example.org/hook",
            "https:///hook",
            "https://xenia.example.org",
            "https://xenia.example.org/",
            "https://xenia.example.org/hook?token=1",
            "https://xenia.example.org/hook#fragment",
            "https://someone:secret@xenia.example.org/hook",
            "https://someone@xenia.example.org/hook",
        ] {
            assert!(
                WebhookAddress::parse(address).is_err(),
                "the address {address:?} must be refused"
            );
        }
    }

    /// The memory keeps its cap by dropping the OLDEST id, never by clearing
    /// whole: the ids most likely to be retried are the newest.
    #[test]
    fn the_acknowledged_memory_evicts_the_oldest_at_its_cap() {
        let mut acknowledged = Acknowledged::new();
        for id in 0..=i64::try_from(ACKNOWLEDGED_MEMORY).expect("the cap fits an id") {
            acknowledged.record(id);
        }
        assert!(!acknowledged.holds(0), "the oldest id left at the cap");
        assert!(acknowledged.holds(1), "the second-oldest stayed");
        assert!(
            acknowledged.holds(i64::try_from(ACKNOWLEDGED_MEMORY).expect("the cap fits an id")),
            "the newest id is remembered"
        );
        // A repeat of a remembered id neither grows the memory nor re-orders
        // it: an id is recorded once.
        acknowledged.record(1);
        assert_eq!(acknowledged.order.len(), ACKNOWLEDGED_MEMORY);
    }
}
