# Telegram unit 02 — sending media, and the caller it does not yet have

Date: 2026-08-25, revised the same day against two independent reviews. The platform lets
a bot send a photo, a document, an audio file, a video, an animation, a voice note, or an
album of two to ten of them. This unit states exactly how that would work here: the
methods, the upload shape, the caption limit against the message limit, the neutral
vocabulary an outbound item needs so the core never names this platform, and how bytes
move without being held in memory.

Two things must be said before the design, because both reviews found them and both are
right.

**The picture arrives before the words, and it cannot be made to arrive after.** The
earlier draft promised the answer first and the illustration second. That promise is not
satisfiable by the mechanism this unit reuses. The outbound edge reads undelivered blocks
in ledger order behind one ascending cursor, and the module says so about itself:
`deliver_answers_and_reports` is documented as yielding blocks "in ledger order — which
puts a turn's report ahead of its answer, because the tool filed before the answer
finalized" (`crates/core/src/outbound.rs:293-296`, loop at `:322-329`). A tool that
appends during the turn allocates a lower block id than the answer, which commits at
`StreamDone`. So the illustration is delivered first, exactly as a filed report already
is. The design below accepts that order and is written for it: the picture carries its own
description and stands alone, and the answer follows a moment later. What would have to
change for words-first is named in the decisions, and it is a new core seam with one user,
which is not something this unit will invent.

**Nothing in this assistant has anything to send yet.** The wiki the lookup reads carries
no images at all. Re-verified on 2026-08-25 two ways: the rendered index yields fifteen
content page names, every one of them fetches 200, and not one carries `![`, `<img`, or a
`.png`/`.jpg`/`.jpeg`/`.gif`/`.webp` reference; and a clone of the wiki repository holds
sixteen files, all of them `.md` — fifteen content pages plus the sidebar, and no binary
of any kind. The releases are ROM archives, hundreds of times past the platform's 50 MB
upload cap. No core module produces a file. The one shape that would produce one, handing
a person their own stored data, is refused by two published documents and would be reckless
in a group channel besides. So the design below is written to be implemented the day a
caller exists, and this unit adds no unreachable variant before then.

## Grounding

### The platform (core.telegram.org/bots/api and /bots/api-changelog, read 2026-08-25)

- **Version.** The changelog's newest entry is **Bot API 10.3, dated August 24, 2026**;
  10.2 is dated July 14, 2026 and 10.1 June 11, 2026. Anything describing 10.1 as current
  is two releases behind.
- **The send methods changed twice inside the last six weeks**, and the earlier draft's
  claim that nothing in 10.0 through 10.3 touched them was false. Verbatim from 10.2:
  "Added the parameters `receiver_user_id` and `callback_query_id` to the methods
  sendMessage, sendAnimation, sendAudio, sendDocument, sendLivePhoto, sendPhoto,
  sendSticker, sendVideo, sendVideoNote, sendVoice, sendContact, sendLocation, sendVenue."
  Verbatim from 10.3: "Added the class `EphemeralMessageParameters` and replaced the
  parameters `receiver_user_id` and `callback_query_id` in the methods sendMessage,
  sendAnimation, sendAudio, sendDocument, sendLivePhoto, sendPhoto, sendSticker, sendVideo,
  sendVideoNote, sendVoice, sendContact, sendLocation and sendVenue with the parameter
  `ephemeral_message_parameters`." Both hit `sendMessage`, which the adapter already calls.
  10.2 also added the class `InputMediaVoiceNote`; 10.0 added `LivePhoto` and
  `sendLivePhoto` and allowed live photos in `sendMediaGroup`; 8.3 added `cover` and
  `start_timestamp` to `sendVideo`. The surface moves on a scale of weeks, not years.
- **The methods and their media parameter.** `sendPhoto`(`photo`),
  `sendDocument`(`document`), `sendAudio`(`audio`), `sendVideo`(`video`),
  `sendAnimation`(`animation`), `sendVoice`(`voice`), `sendMediaGroup`(`media`).
- **What the six actually share**, read off the live parameter tables, not assumed:
  `business_connection_id`, `chat_id`, `message_thread_id`, `direct_messages_topic_id`,
  `ephemeral_message_parameters`, the media parameter itself, `caption`, `parse_mode`,
  `caption_entities`, `disable_notification`, `protect_content`, `allow_paid_broadcast`,
  `message_effect_id`, `suggested_post_parameters`, `reply_parameters`, `reply_markup`.
  Only `sendPhoto`, `sendVideo` and `sendAnimation` carry `show_caption_above_media` and
  `has_spoiler`; `sendAudio`, `sendDocument` and `sendVoice` carry neither. `thumbnail` is
  on `sendAudio`, `sendDocument`, `sendVideo` and `sendAnimation`, and on neither
  `sendPhoto` nor `sendVoice`. `sendVideo` and `sendAnimation` add `duration`, `width`,
  `height`. `reply_parameters` is present on every one of them, so an attachment threads
  onto a member's message exactly as a text reply does.
- **Caption against message.** Every media method's caption is documented as
  "0-1024 characters after entities parsing". `sendMessage`'s text is "1-4096 characters
  after entities parsing". A caption is therefore a quarter of a message, and "after
  entities parsing" means the markup itself is not counted — the limit applies to the text
  a member sees, not to the HTML sent.
- **The three ways to send a file**, verbatim from "Sending files": a `file_id`, for which
  "There are no limits for files sent this way"; an HTTP URL the platform fetches itself,
  "5 MB max size for photos and 20 MB max for other types of content"; or "Post the file
  using multipart/form-data in the usual way that files are uploaded via the browser.
  10 MB max size for photos, 50 MB for other files."
- **Sending by URL is judged on the host's MIME type, not on our file name.** Verbatim:
  "When sending by URL the target file must have the correct MIME type (e.g., audio/mpeg
  for sendAudio, etc.)", "In sendDocument, sending by URL will currently only work for .PDF
  and .ZIP files", "To use sendVoice, the file must have the type audio/ogg and be no more
  than 1MB in size. 1-20MB voice notes will be sent as files", and "Other configurations
  may work but we can't guarantee that they will." A `image/webp` photo and an `image/gif`
  animation both sit inside that last sentence.
- **Per-method caps.** `sendPhoto`: "The photo must be at most 10 MB in size. The photo's
  width and height must not exceed 10000 in total. Width and height ratio must be at most
  20." `sendDocument`, `sendAudio`, `sendVideo`, `sendAnimation`, `sendVoice`: "up to 50 MB
  in size, this limit may be changed in the future".
- **Multipart shape.** `InputFile` is "the contents of a file to be uploaded. Must be
  posted using multipart/form-data in the usual way that files are uploaded via the
  browser." For the single-item methods the file rides in the form field named after the
  method's own parameter. Inside `sendMediaGroup` and for thumbnails and video covers, the
  JSON refers to a form field by name: pass `attach://<file_attach_name>` "to upload a new
  one using multipart/form-data under `<file_attach_name>` name".
- **`sendMediaGroup`.** `media` is "A JSON-serialized Array describing messages to be sent,
  must include 2-10 items" of `InputMediaAudio`, `InputMediaDocument`,
  `InputMediaLivePhoto`, `InputMediaPhoto` and `InputMediaVideo`. "Documents and audio
  files can be only grouped in an album with messages of the same type." It returns an
  array of `Message`. Per-item fields differ and the earlier draft flattened them:
  `InputMediaPhoto` is type, media, caption, parse_mode, caption_entities,
  show_caption_above_media, has_spoiler; `InputMediaAudio` is type, media, thumbnail,
  caption, parse_mode, caption_entities, duration, performer, title, with neither
  show_caption_above_media nor has_spoiler; `InputMediaDocument` is type, media, thumbnail,
  caption, parse_mode, caption_entities, disable_content_type_detection, likewise with
  neither. One item is not a valid album; the minimum is two.
- **Media is a separate permission from text, and animations sit under a third one.**
  `ChatPermissions` lists `can_send_messages` ("text messages, rich messages, contacts,
  giveaways, giveaway winners, invoices, locations and venues"), `can_send_photos`,
  `can_send_audios`, `can_send_documents`, `can_send_videos`, `can_send_voice_notes` — and
  `can_send_other_messages`, whose live wording is "True, if the user is allowed to send
  animations, games, stickers and use inline bots". An animation is therefore not covered
  by `can_send_photos`. A group can let the assistant answer questions, permit it photos,
  and refuse it every gif, at any time, without telling it. Any design in which the words
  depend on the picture is broken by that alone.
- **Resending by `file_id`.** "It is not possible to change the file type when resending by
  file_id", "file_id is unique for each individual bot and can't be transferred from one
  bot to another", and thumbnails cannot be resent.

### Our tree

- **The wire client speaks JSON only.** Every call goes through `post`
  (`crates/adapters/telegram/src/client.rs:531-545`) as `.json(body)`. There is no
  multipart path and no `tokio::fs` use in the adapter. The adapter does open files: the
  offset sidecar reads and writes through `std::fs` (`state.rs:13`, `:44-45`). The earlier
  draft's "no file handle anywhere in the adapter" was wrong; what is true is narrower and
  is what matters here — nothing in the adapter streams a file to a socket today.
- **`request` cannot carry an upload.** `request` (`client.rs:505-527`) takes
  `&serde_json::Value` and re-posts the same body up to `RATE_LIMIT_ATTEMPTS = 3`
  (`client.rs:29`) through `post`, which is `.json(body)`-only. A `multipart::Form` is not
  `Clone`, and a streamed body is consumed on send. An upload therefore needs its own
  transport function that rebuilds the form on each attempt; it cannot reuse `request`, and
  the earlier draft was wrong to say it could.
- **The message cap already lives in the adapter, and only there.**
  `MESSAGE_UTF16_UNIT_LIMIT = 4096` (`client.rs:34`) and `chunks_within_cap`
  (`client.rs:599-616`) split an over-long reply into consecutive sends; the core knows no
  number. `send_message` (`client.rs:371-391`) threads only the first chunk and reports how
  many chunks reached the chat before a failure, and `consume_replies`
  (`driver.rs:730-761`) logs "dropped" and "cut short" as different outcomes. This is the
  precedent for every platform limit in this spec: the core states the rule, the adapter
  states the number.
- **Threading is already carried.** `OutboundReply.reply_target`
  (`crates/core/src/message.rs:389`) is translated into `reply_parameters` with
  `allow_sending_without_reply: true` (`client.rs:451-454`), so a deleted target degrades
  to a plain send.
- **Formatting exists and has a failure path.** `formatting::to_html`
  (`crates/adapters/telegram/src/formatting.rs`) renders a bounded markdown subset into the
  platform's HTML, and `send_chunk` retries unformatted when the refusal names the
  formatting (`client.rs:411-436`, predicate at `:478-485`).
- **The outbound vocabulary.** `OutboundReply { channel, text, kind, reply_target }`
  (`message.rs:373-390`) and `ReplyKind { Answer, Notice, Report }` (`message.rs:331-340`),
  re-exported at `crates/core/src/lib.rs:85-89`. Neither carries an attachment today, and
  no field of either can hold one; adding the capability means changing that type and every
  construction site. `deliverable_of` (`crates/core/src/outbound.rs:478-499`) maps one block
  to one reply; the delivery loop (`outbound.rs:322-384`) swallows an empty `Answer`
  (unit 22) and folds the first-interaction disclosure into an `Answer` only.
- **Delivery order is ledger order, and the module knows it.** The cursor is the highest
  delivered block id per conversation (`outbound.rs:26-32`, `:322-329`), and the doc
  comment on the delivery function states that a turn's report goes ahead of its answer
  because the tool filed first (`outbound.rs:293-296`). The report tool appends inside its
  own `execute` (`crates/core/src/tools/report.rs:393-404`). There is no reordering step
  and no post-answer append site in the core.
- **A failed turn still delivers what the turn already filed.** On `CoreEvent::StreamError`
  the edge runs the same stored-state delivery before the failure notice, deliberately
  (`outbound.rs:210-247` and its comment). So a turn that files and then dies puts the
  filed item in the chat followed by "something went wrong".
- **A tool round runs in parallel.** `ReportTool` holds a filing lock precisely because
  "the runner executes same-round calls in parallel tasks, so without this lock two calls
  naming one origin both scan before either appends"
  (`crates/core/src/tools/report.rs:336-341`). Any tool that must be limited per turn needs
  the same shape; the answering budget bounds turns, not calls inside one.
- **The wiki lookup and its address shape.** Page content comes from
  `{base}/wiki/{ORGANIZATION}/{WIKI_REPOSITORY}/{page}.md` (`crates/core/src/tools/wiki.rs:209-212`),
  with `DEFAULT_BASE_URL = "https://raw.githubusercontent.com"` (`wiki.rs:50`) and a second
  configured base `DEFAULT_INDEX_BASE_URL = "https://github.com"` (`wiki.rs:57`) for the
  rendered index. Page names allow letters, digits, dash, underscore, dot and parentheses
  (`page_name_char`, `wiki.rs:92-95`); a page result is cut at `RESULT_LIMIT = 10_000`
  characters with a truncation marker (`wiki.rs:73-78`, `bounded_result` at `:291-298`).
  The per-process cache holds `Result<String, String>` keyed by the full address, five
  minute TTL, cap 64, cleared whole at the cap (`wiki.rs:80-118`).
- **The shared lookup contract reads whole bodies and refuses redirects.**
  `bounded_get` (`lookup.rs:41-56`) and `bounded_get_text` (`lookup.rs:77-91`) both call
  `read_body` (`lookup.rs:132-152`), which downloads the body and refuses past
  `MAX_BODY_BYTES = 1024 * 1024` (`lookup.rs:17`). `checked_success` (`lookup.rs:117-129`)
  makes any 3xx a refusal, and `lookup_client` sets `redirect::Policy::none()`
  (`lookup.rs:29-35`). There is no status-only path in that module. The earlier draft
  claimed to reuse this contract while reading no bytes and declining at 5 MB; both reviews
  showed that to be three separate contradictions, and the design below no longer says it.
- **Tools are declared and registered in different places.** `crates/core/src/tools/mod.rs:31-38`
  is the module list. Registration is `ToolSet::admit` (`mod.rs:112-118`), called from
  `ToolSet::production_lookups` (`mod.rs:91-109`) for the three lookups and from the
  assembly for the report tool; the palette is derived from the same set (`mod.rs:120-134`).
  A new tool needs both sites, and the earlier draft named only the module list.
- **The framework tracks attachments without touching bytes.**
  `agent-ledger/crates/agent-ledger/src/store/attachments.rs:1-4`: "No file bytes pass
  through here. This module tracks what an attachment is and which parts of it are already
  on disk; the caller owns the file itself." `Attachment { id, url, filename, mime,
  total_size, created_at }` (`:12-27`), `create_attachment` (`:45-72`), and sparse range
  tracking (`:154-219`).
- **`reqwest` can stream an upload with no new crate.** Verified against reqwest 0.13.4
  source (`Cargo.lock:1111-1113`): `multipart::Part::stream_with_length<T: Into<Body>>(value,
  length)` exists (`async_impl/multipart.rs:264`); `impl From<tokio::fs::File> for Body` is
  compiled under feature `stream` and is `Body::wrap_stream(ReaderStream::new(file))`
  (`async_impl/body.rs:224-231`); the form's `Content-Length` is set only when
  `compute_length()` answers `Some` (`async_impl/request.rs:331-332`), which needs every
  part to report a length. `Part::file` is the blocking API, which is why the design calls
  `stream_with_length` directly. The manifest currently asks for
  `reqwest = { version = "0.13", features = ["json"] }` and
  `tokio = { ..., features = ["macros", "sync", "time"] }`
  (`crates/adapters/telegram/Cargo.toml`), so an upload needs the `multipart` and `stream`
  features plus tokio's `fs`, and no new dependency. reqwest 0.13.4 is the current stable
  release (crates.io, 2026-08-25).
- **A dropped response body is not downloaded.** reqwest streams a response body lazily;
  a `Response` dropped after its status and headers are read closes the connection without
  pulling the body. That is what makes a status-and-headers check on an image address cost
  headers, not megabytes.
- **The scripted test server answers `sendMessage` and nothing media-shaped.** Its own
  header says it "answers the three methods the adapter speaks" (`tests/adapter/server.rs:1-3`),
  and `SendScript` is "one scripted send outcome, consumed in order, one per `sendMessage`"
  (`server.rs:41`), dispatched at `server.rs:368` and `:398`. `read_request`
  (`server.rs:303-338`) parses a `Content-Length` body and falls back to `{}` on a decode
  failure. Two consequences, the second corrected from the earlier draft: a multipart
  request records an empty body, and a request with no `Content-Length` does not hang — the
  header parse answers `0` (`server.rs:324`), so `body_end` equals `header_end + 4`, the
  wait loop never blocks, and the chunked bytes are left in the buffer to be misread as the
  next request's head. Either way the fixture must learn multipart before it can assert
  anything about an upload, and it must gain a media-method script before it can assert
  anything about a media send.
- **Privacy documents are per-block-kind and already reach a consumer kind.**
  `docs/privacy/records-of-processing.md` lists categories D1 to D9 with their storage
  location, D7 being the report block's content (`records-of-processing.md:67`), and the
  erasure table reaches into it (`:115`, decision 0063). The DPIA carries the same list
  (`dpia.md:129-130`) and states there is no export (`dpia.md:392`). The "No media, no
  files, no voice, no stickers" sentence at `records-of-processing.md:61` is about what is
  collected from members, not about what the assistant sends.

### The caller question — verified, not assumed

- **The wiki carries no images.** The lookup enumerates pages from the rendered index
  (`wiki.rs:225-247`, scan at `:274-289`). Reproducing that scan against the live index on
  2026-08-25 yields fifteen names: AIDL-HALs, Button-Backlight-Control, Code-of-Conduct,
  Configurable-Dark-Mode-Tones, Contact-and-maintainership, Encryption-auto-detection,
  Fixing-errors, Fixing-runtime-errors, Font-System, Home,
  Integrating-Sandboxed-Google-Play-(16.2), Porting-from-other-ROMs,
  Porting-from-other-ROMs-(Legacy), Project-Standards, System-AIDL-Services. All fifteen
  fetch 200 from the raw base, with and without percent-encoded parentheses; Home.md is 134
  bytes. None carries `![`, `<img`, or any image file reference. A clone of the wiki
  repository confirms it from the other side: sixteen files, all `.md`, no binary at all.
- **An image added through the wiki's web editor would not live in that repository.**
  The raw wiki base serves only what is committed to the wiki repository. GitHub stores a
  picture dragged into the wiki editor on its own attachment host and writes an absolute
  URL into the page instead. So a design that can only address `{wiki base}/wiki/{org}/{repo}/{name}`
  would decline the ordinary way a maintainer adds a screenshot. This is why the resolution
  below reads the address out of the page instead of composing one.
- **A build cannot be sent.** The release lookup points at ROM archives. 50 MB by upload
  and 20 MB by URL are the ceilings; a build is far past both. The assistant can only ever
  answer with a link, and the answering teaching should keep saying so.
- **No core module produces bytes.** Every tool performs "one bounded HTTP GET against its
  configured base URL" and returns text (`crates/core/src/tools/mod.rs:9-12`).
- **The one file-shaped idea is refused by our own published documents.** Handing a person
  their stored data as a document would contradict
  `docs/privacy/bot-assistant-privacy-policy.md:140-157`, which routes access requests to a
  person by email and says "the commands are the one place a machine acts", and
  `docs/privacy/dpia.md:392`, which states there is no export. In a group channel it would
  also disclose one member's data to every other member in the room.

## Decisions taken with this unit

- **The design is written now; the code arrives with its first caller, 2026-08-25.**
  A capability with no caller is dead code: an unreachable variant on `OutboundReply`, an
  unreachable arm in `consume_replies` and an unreachable block kind, each of which every
  later reader has to reason about. It is also the wrong way round — the caller's source of
  bytes is the single fact that decides the shape, and picking a shape before knowing it is
  how the first attempt at the media subsystem became unbuildable. *Rejected:* building the
  whole send path now so it is ready (dead code, and a shape chosen blind). *Rejected:*
  answering "the assistant never sends media" and closing the question — one screenshot
  added to the wiki makes it real the same day, and this spec is what makes that day cheap.

- **The first real use is an illustration a project page already carries, 2026-08-25.**
  When a wiki page documents a screen — a settings screen, a recovery menu — the answer
  that cites the page sends the picture beside the words. It fits what the assistant already
  is: everything substantive comes from a lookup (decision 0096), and a picture the
  project's own page references has exactly that provenance. Stated plainly: today it would
  fire never, because no page has an image. *Rejected:* an assistant-authored file (a
  transcript, a summary, a data export) as the first use — the export shape is refused
  above, and the rest are text the platform already carries.

- **The picture goes out before the words, and the design is written for that order,
  2026-08-25.** The tool appends during the turn, the answer commits at the end of it, block
  ids ascend, and the edge delivers by ascending id behind one cursor. The order is not a
  preference this unit can express; it is what the delivery mechanism does, and the module
  already documents the same outcome for a filed report. Three consequences are accepted
  deliberately. The picture's description is written to stand alone, because it may be the
  first thing the group sees. A turn that illustrates and then fails puts a described
  picture plus the failure notice in the chat with no answer, exactly the shape a filed
  report already has on a failed turn. And the safety argument is restated correctly: the
  answer never depends on the picture, so a media send refused for permission, size or a
  dead address costs the picture and not the answer — that holds in either order, and it
  was the only part of the earlier draft's reasoning that actually needed the order.
  *Rejected:* reordering inside the outbound edge so the answer overtakes the illustration.
  It contradicts the cursor's whole contract, it would have to hold a delivered item back
  on a guess about what is coming, and a turn that never answers would strand it.
  *Rejected:* moving the append out of the tool, so the tool returns a resolved reference
  and something appends the illustration block after the answer commits. It is the only
  shape that yields words-first, and it needs a post-answer append site in the core that
  nothing else wants, plus turn-scoped state living outside the ledger between the tool call
  and the commit. If a second caller ever needs words-first, that is the seam to build, and
  it is a unit of its own.

- **The model names a page and an image the page itself references; the core resolves the
  address from the page, 2026-08-25.** The tool is `show_wiki_image`, required authority
  `Member`, with two parameters: the page name and the image reference. The core fetches the
  page through the existing wiki path — same address shape, same bounded GET, same cache —
  scans its markdown for image references, and matches the model's string against them by
  the reference's own text or by its last path segment. No match is a fixed decline. The
  address is then the page's own: a relative reference resolves against the configured wiki
  base under the existing page address shape, with subdirectory paths allowed because the
  path was written by a maintainer and not by the model; an absolute reference is accepted
  only over https and only when its host is one of a small configured set, which is the raw
  wiki host and the forge's attachment host, held beside the two wiki bases in
  `LookupEndpoints`. Anything else declines. This makes the page parameter actually do
  something, gives the model a real way to learn an image's name (it read the page through
  the wiki tool and saw the reference in the markdown), and covers the web-uploaded case the
  composed-address form structurally could not. *Rejected:* composing
  `{base}/wiki/{org}/{repo}/{name}` from a model-supplied file name, which was the earlier
  draft. It cannot address an image in a subdirectory, it cannot address the attachment host
  at all, and it leaves the page parameter with no effect, so the model could pair any page
  with any file name. *Rejected:* a `url` parameter — any member could then talk the
  assistant into posting any picture from anywhere into the group under its own name, and an
  answering discipline based on lookups that accepts an unchecked address is not a
  discipline. *Rejected:* letting the model paste an image link into its prose and having the
  adapter detect it — a magic string in prose is the workaround unit 22 removed.
  *Known and named, not hidden:* a reference past the page result's 10,000-character cut is
  invisible to the model, so a very long page can carry an image the model never learns
  about. The core still resolves against the whole fetched page, so a model that names the
  reference some other way still succeeds.

- **The check is one request whose body is never read, with its own redirect policy,
  2026-08-25.** A new helper in the lookup module sends one GET to the resolved address and
  drops the response after the status and headers, sharing the module's failure wording and
  its named-refusal style. It follows up to three redirects, through its own client rather
  than the shared `lookup_client`: the shared no-redirect rule exists so that page *content*
  can never come from an unnamed host, and here nothing is read, the address is already
  restricted to the allowed hosts, and the attachment host answers a redirect to an object
  store as its normal behaviour. The platform's own fetcher follows redirects too, so
  refusing one would decline images the platform would have sent. The media type comes from
  the response's `Content-Type` when it names an image type, and from the reference's
  extension only when the header is absent; a media type that is not an image declines,
  which is also what keeps a `.png`-named HTML error page from being sent as a picture. The
  outcome — the resolved address and its media type, or the decline — is cached with the same
  TTL and cap as the page cache, in the image resolution's own map keyed by resolved address,
  so a repeated ask costs nothing and a missing image is negatively cached the way a missing
  page is. *Rejected:* reusing `bounded_get_text`, which was the earlier draft's claim. It
  downloads the whole body, refuses at one mebibyte, and treats every redirect as a failure —
  three behaviours the check must not have. *Rejected:* filing the block unchecked — the model
  would promise a picture that silently never arrives, which is the failure this project keeps
  designing against. *Rejected:* a `HEAD` request, because a 405 from a host that serves the
  image perfectly well would read as a missing image.

- **The core states no byte limit for this feature, 2026-08-25.** The earlier draft had the
  core decline where a host stated a `Content-Length` past 5 MB. That put a platform number
  in the core, which the naming rule forbids; it was also the wrong number for one of its own
  media types, since the URL ceiling is 5 MB for photos and 20 MB for other content, and a gif
  goes out as an animation. The platform judges the size itself, authoritatively, at the moment
  it fetches, and a refusal costs the picture and is logged like any other refused send. If a
  ceiling is ever needed before the send, it belongs in the adapter as named constants beside
  `MESSAGE_UTF16_UNIT_LIMIT`, one per method. *Rejected:* the adapter pre-checking the size
  itself, which would mean a second fetch of the same headers to predict a verdict the platform
  is about to give for real.

- **Geometry is the platform's refusal to give, 2026-08-25.** `sendPhoto` requires width plus
  height under 10000 and a ratio no worse than 20 to 1. Neither can be known without decoding
  the image, and this design reads no bytes, so a tall stitched screenshot — precisely the kind
  of picture a settings-screen page would carry — is refused by the platform and logged. That
  is stated here so nobody reads the absence of a check as an oversight. *Rejected:* fetching
  and decoding the image to measure it, which breaks the no-bytes rule at the one place it is
  cheap to break, pulls every picture through this machine for a verdict the platform gives
  anyway, and needs an image-decoding dependency. *Rejected:* falling back to `sendDocument`
  on a geometry refusal, which silently changes what the member sees, needs its own permission,
  and cannot be distinguished from other refusals without matching on the platform's wording.

- **An attachment is an address, a media type and a file name, and it rides on the reply kind,
  2026-08-25.** The neutral vocabulary added to `crates/core/src/message.rs` is
  `OutboundAttachment { address: String, media_type: String, filename: String }`, and
  `ReplyKind` gains `Illustration(OutboundAttachment)`. The reply's `text` is the picture's
  description; the attachment travels inside the variant that means "this reply is a picture",
  so a reply cannot be a picture without one or carry one without being one. `ReplyKind` stops
  being `Copy` as a result; the two comparisons in the delivery loop are equality checks against
  `Answer` and keep working under `PartialEq`. The adapter maps the media type to a method:
  `image/gif` to `sendAnimation`, any other `image/*` to `sendPhoto`, anything else to
  `sendDocument` as the total-match fallback. *Rejected:* an `Option<OutboundAttachment>` field
  beside a marker variant, which makes two illegal states representable and forces every reader
  to check the pairing by hand. *Rejected:* `Presentation::{Inline, AsFile}` from the earlier
  draft, along with the audio and video rows of its table — the one caller produces an inline
  image and nothing else, so `AsFile` and four table rows had no producer, which is the
  unreachable code this unit's first decision forbids. *Rejected:* an `AttachmentSource` enum
  with one `Address` variant, for the same reason; when a caller with bytes on disk exists, the
  field becomes an enum then, in that unit. *Rejected:* an enum in the core naming
  Photo/Document/Audio/Video/Animation/Voice — five of those six are this platform's own
  presentation names, so the core would be carrying one platform's method list under
  generic-sounding words, which is the naming check passing while the invariant fails.
  *Rejected:* the core passing the method name as a string, which is the same leak with no
  disguise.

- **The picture's text is a description, called a label, bounded in the core, 2026-08-25.**
  The tool takes a third parameter: a short description of what the picture shows. The core
  requires it to be non-empty after trimming and no longer than 300 characters, and declines
  with a fixed result over either bound instead of truncating anyone's words. It is called a
  label and not a caption throughout the core, because `caption` is the literal parameter name
  of all six platform methods and this is the one place the naming rule would have been broken
  in plain sight. Three hundred characters is inside the platform's 1024 by any measure: 300
  scalar values are at most 600 UTF-16 units, and markup removal only shrinks the parsed length.
  *Rejected:* an optional or empty description — the picture arrives first, so an unlabelled one
  is a non sequitur with no answer yet beside it, and the delivery loop's empty-text swallow
  applies to answers only. *Rejected:* one reply carrying both the answer prose and the
  attachment — an answer runs to 4096 units and a caption stops at 1024, so the adapter would be
  splitting it in the usual case, and the split would be invisible in the ledger.

- **At most one illustration per turn, 2026-08-25.** The tool holds a filing lock and, under it,
  scans the conversation's blocks for an illustration already appended in this turn; a second
  call declines with a fixed result. Without it a round of ten parallel calls yields ten
  pictures appended concurrently, which is both a flood and a nondeterministic order. This is
  the report tool's own shape and for the same recorded reason (`tools/report.rs:336-341`).
  *Rejected:* leaning on the per-turn answering budget, which bounds answered turns and not
  calls inside one. *Rejected:* an album, which needs two to ten items and has no caller.

- **The caption cap is the adapter's number, measured on the source text in UTF-16 units,
  2026-08-25.** A label longer than the platform accepts is sent as a following ordinary
  message with the media sent captionless, never truncated — decision 0019's rule applied to
  the second cap. The measure is the source text, not the HTML: the limit is "after entities
  parsing", so the markup `to_html` adds does not count. UTF-16 units match the existing
  chunker (`client.rs:34`) and are the conservative reading. With the core's 300-character
  bound in place this path should never run; it is written because "should never" is not
  "cannot", and the alternative is a refused send. *Rejected:* computing the fit in the core —
  the number is the platform's, and so is the renderer that expands it.

- **The ledger records the intent; the log records the outcome, 2026-08-25.** The illustration
  block is appended when the tool succeeds, and it is never rewritten. A refused send is logged
  and dropped, exactly as a refused answer send is (`driver.rs:745-758`). This matches what
  already happens to an answer whose send fails: the block stands, the chat did not see it, the
  log says so. The block stores the page name, the image reference, the resolved address, the
  media type and the model's label.

- **The label is model prose and is treated as such by the privacy record, 2026-08-25.** The
  earlier draft concluded that no privacy document changes, arguing that the illustration block
  records no personal field. That is not established: the label is free model text, and decision
  0067 lets the model address people by their shown handle, so a label can name a member. The
  precedent runs the other way too — decision 0063 reaches erasure into the report block, a
  consumer kind. So: `docs/privacy/records-of-processing.md` gains one category row for the
  illustration record and one erasure row nulling its label, and `docs/privacy/dpia.md` gains
  the same category in its data list. Nothing else changes, and the reasoning for each of the
  other three questions is stated, not assumed. Nothing new reaches the model provider:
  the model names a page and a reference, and never sees the picture. No bytes reach our disk.
  The recipients are unchanged: the group already receives the assistant's messages, and the
  platform fetching a public project file from the wiki host carries no member's data. The
  "No media, no files, no voice, no stickers" sentence at `records-of-processing.md:61`
  describes what is collected from members and stays true. *Rejected:* declaring the label
  non-personal by construction and shipping no document change, which is the shape that makes a
  published statement false and calls the correction a follow-up.

- **`sendMediaGroup` is not built, 2026-08-25.** The facts are recorded above so nobody
  re-derives them: two to ten items, documents and audio only grouping with their own type, an
  array of `Message` returned, per-item fields that differ by type, and `attach://` naming for
  uploaded items. One illustration is one picture, and an album cannot express what no caller
  has asked for. *Rejected:* building the album path because the API has it.

- **`file_id` reuse is not built, 2026-08-25.** A returned `file_id` would let a repeat send
  skip the platform's fetch entirely, with "no limits" attached. It also means storing platform
  identifiers — in the core, which may not name them, or in an unbounded adapter memory.
  *Rejected for now:* the saving is unmeasured and the same picture being sent often is a
  supposition about a use that does not exist yet. Revisit when one does.

- **The upload-from-disk form is specified, not built, 2026-08-25.** The URL form ships nothing
  through this machine at all: the platform fetches the file itself, and the existence check
  drops the response without reading its body, so the strongest answer to the streaming rule
  here is that no byte stream exists on our side. When a caller has bytes on disk, the address
  field becomes an enum with a stored variant; the adapter opens the file with
  `tokio::fs::File::open`, builds `multipart::Part::stream_with_length(file, len)` with
  `.file_name()` and `.mime_str()` set from the neutral fields, puts the scalar fields in as
  `Form::text` parts, and names the file part after the method's own parameter — `attach://`
  only where the JSON has to refer to it, which is albums, thumbnails and video covers. Nothing
  is read into memory: `From<File> for Body` streams the file through a reader stream, chunk by
  chunk from disk to the socket. **Never `Part::stream` without a length**: reqwest computes the
  form's `Content-Length` only when every part reports one, and without it the request goes out
  chunked, which the platform does not document as accepted and which the scripted server
  misframes as the next request's head (`server.rs:303-338`). The file's length comes from its
  metadata, so there is no reason to omit it. The upload gets its own transport function beside
  `post`, because `request` re-posts one `&Value` and neither a form nor a streamed body can be
  re-sent; its retry rebuilds the form by reopening the file, under the same
  `RATE_LIMIT_ATTEMPTS` and the same stated-wait discipline. *Rejected:* reading the file into a
  `Vec` and using `Part::bytes` — it breaks the streaming rule at the one place it matters, a
  50 MB upload on a machine serving a whole group. *Rejected:* reusing `request` unchanged,
  which the earlier draft specified and which does not compile against a multipart body.

- **The naming check is extended where it can be, and stated honestly where it cannot,
  2026-08-25.** `docs/platform-vocabulary.txt` today holds platform and SDK names only, so it
  cannot detect a method name or a byte limit in the core; the earlier draft's claim that the
  scan pinned that property was not true of the instrument named. The file gains the six method
  names, `sendmediagroup`, and the fields this feature would most plausibly leak — `caption`,
  `spoiler`, `thumbnail` — all of which the core is clean of today, verified by scan. Numbers
  are not added: the scan matches runs of digits as words, and `1024` is already a legitimate
  byte constant in the core's own lookup bound. So the acceptance criterion splits: the names
  are checked by the scan, and the absence of platform numbers is a review criterion, called
  that instead of called pinned. *Rejected:* adding numerals and living with the false
  positives, which trains people to widen the ignore list until the check means nothing.

- **The tool is admitted through the palette and taught in one sentence, 2026-08-25.** It joins
  through `ToolSet` at `Member` authority like the lookups, so the palette written at every
  conversation's creation names it and admission fails closed (decisions 0041, 0043). The
  answering teaching gains one sentence: illustrate only from a page already cited this turn,
  and only when the picture shows the thing being asked about. The sentence is written in the
  teaching module by a maintainer, which is what decision 0046 requires. *Rejected:* adding the
  tool with no teaching, which leaves the model to infer when a picture helps and produces
  either silence or noise.

- **Nothing here gives the assistant power over a person, 2026-08-25.** Decision 0070 stands
  untouched: sending a picture files nothing, hides nothing, removes nothing and restricts
  nobody. The only harm a send can do is noise, and the one-per-turn cap bounds it.

## The unit's contract

When the first caller exists, the core gains one tool that takes a page name, an image
reference and a short label; resolves the reference against that page's own markdown to an
address that is either the configured wiki base's path form or an https address on one of the
configured allowed hosts; verifies the address answers, reading its status and headers and
never its body; derives the media type from the response's content type and falls back to the
reference's extension; and appends one illustration block per turn carrying the page name, the
reference, the resolved address, the media type and the label. The outbound edge yields that
block as a reply whose kind carries an `OutboundAttachment`, in ledger order, which places it
ahead of the answer of the same turn; the adapter translates that reply into the platform
method the media type selects, with the label rendered through the existing formatter and sent
as the method's caption, threaded through the existing reply parameters, and subject to the
existing rate-limit and formatting-refusal contracts. The answer never depends on the picture:
a media send refused for permission, size, geometry or a dead address costs the picture alone
and is logged as such. The core names no method, no field and no limit of this platform; the
adapter decides nothing beyond which method a media type maps to and how a label that will not
fit is carried. Until that caller exists, no code changes: this document is the unit.

## Acceptance criteria

These are the criteria of the build, to be met when the caller arrives.

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary scan (`crates/core/tests/vocabulary.rs`) and the token scan
  (`crates/adapters/telegram/tests/token_scan.rs`) clean. The merged tree's
  `docs/dependency-review.md` carries an entry for each newly enabled crate feature, with its
  version checked on the day; no new crate.
- **AC2** Two separate checks, because one instrument cannot do both. The naming half:
  `docs/platform-vocabulary.txt` has gained `sendphoto`, `senddocument`, `sendaudio`,
  `sendvideo`, `sendanimation`, `sendvoice`, `sendmediagroup`, `caption`, `spoiler` and
  `thumbnail`, and the vocabulary scan passes over `crates/core` with them in place. The
  numbers half is a review criterion and is stated as one: a reviewer reads the new core module
  and confirms no platform byte limit, dimension or ratio appears in it.
- **AC3** The model cannot choose an address. Given a page whose markdown carries one relative
  image reference, a call naming that reference resolves to the configured wiki base's path form
  for the reference's own path, including its subdirectory. Given a page carrying an absolute
  reference to a host outside the configured allowed set, the call declines with the fixed
  result and appends nothing. Given a model string that matches no reference on the page —
  including a string that is itself a well-formed URL — the call declines with the fixed result
  and appends nothing. Given a page whose reference resolves to an allowed host, the resolved
  address is byte-for-byte the page's own.
- **AC4** A missing image declines instead of promising: the check answering 404 yields the
  fixed decline to the model, appends no block, sends nothing, and is served from the negative
  cache on an immediate second call without a second request — pinned against a scripted host.
- **AC5** The check reads no body: against a scripted host that answers a large body, the tool
  succeeds on the status and headers alone and the number of body bytes the fixture served is
  zero. A 302 to a second allowed address is followed and succeeds; a fourth redirect declines.
  A response whose content type is not an image declines even when the reference ends in
  `.png`.
- **AC6** The order is the ledger's and the failure isolation holds: a turn that illustrates and
  answers yields the illustration reply before the answer reply, matching the ledger's block
  order; with the media send refused by the scripted host, the answer is still delivered and the
  log records the failure for the picture only. A turn that illustrates and then fails delivers
  the illustration and then the failure notice.
- **AC7** One picture per turn: two illustration calls in one round yield one appended block and
  one fixed decline, with the calls executed concurrently in the test so the lock is what is
  being checked.
- **AC8** The label travels as the caption: a within-bound label is sent as the media method's
  `caption` with `parse_mode: HTML` through the existing renderer, and the method follows the
  media type — `image/png` to `sendPhoto`, `image/gif` to `sendAnimation` — pinned per row
  against a scripted host taught those methods. The core declines an empty-after-trimming label
  and a label past 300 characters with the fixed result, appending nothing.
- **AC9** An over-cap label loses no words: with the adapter's caption limit lowered in the
  test, the media goes out captionless and the label follows as an ordinary message, chunked by
  the existing rule.
- **AC10** The existing contracts still hold on the new call: the illustration threads onto the
  same target with `allow_sending_without_reply: true`, a formatting refusal retries the label
  unformatted, and a rate-limited media send honours the send ceiling and the bounded retries —
  each pinned against the scripted host, which has gained a media-method script kind and
  per-method request recording for the purpose.
- **AC11** The ledger tells the truth: the illustration block is appended once, is never
  rewritten, records the page name, the image reference, the resolved address, the media type
  and the label, and an erasure of a member named in a label nulls that label while the block
  itself stands — the same treatment decision 0063 gives the report line.
- **AC12** The privacy record is true on the day it merges: `records-of-processing.md` carries
  the illustration record as a data category with its storage location and an erasure row, and
  `dpia.md` carries the same category. A reviewer confirms no other statement in either document
  became false.
- **AC13** The tool is admitted and taught: the palette written at a new conversation's creation
  names the tool, a call from a provenance below `Member` is declined by the admission wrapper,
  and the composed system prompt carries the one illustration sentence — each pinned.
- **AC14** No unreachable code ships: every enum this unit adds has all of its variants produced
  by the merged code, and `sendMediaGroup`, `file_id` reuse, a stored-file source and a
  presentation enum appear nowhere.
- **AC15** (only when the stored-file source is built) The upload streams and is provable: the
  multipart request carries a `Content-Length` equal to the form's computed length, the scripted
  server parses the body by its boundary and records the file part's field name, file name and
  byte count for a 50 MB fixture, and the adapter crate contains no `Part::bytes`, no
  `fs::read`, and no whole-file buffer on the send path — the last checked by a source scan
  not by a memory measurement, because peak resident memory in a test binary is an
  allocator property and cannot be judged pass or fail.

## Notes for launch

- Branches from `main`. Nothing merges until a caller exists; if the wiki gains a screenshot,
  that is the trigger, and this document is the whole implementation brief.
- Core sites: `crates/core/src/message.rs` — `OutboundAttachment` beside `OutboundReply` at
  `:373-390`, and the new `ReplyKind::Illustration` variant at `:331-340`, which drops the
  type's `Copy`; `crates/core/src/lib.rs:85-89`, the re-export list;
  `crates/core/src/kind.rs:1118-1140`, the illustration block kind beside `Report`;
  `crates/core/src/outbound.rs:478-499`, where `deliverable_of` gains one arm, and `:322-384`,
  the delivery loop, whose empty-answer swallow at `:339-352` stays untouched because the new
  kind is not an `Answer`; `crates/core/src/tools/wiki.rs:209-212` for the address shape and
  `:80-118` for the cache shape to copy; `crates/core/src/tools/lookup.rs` for the new
  status-and-headers helper beside `bounded_get_text` at `:77-91`;
  `crates/core/src/tools/mod.rs:31-38` for the module declaration **and** `:91-118` for the
  registration, which are two different sites; `crates/core/src/teaching.rs` for the one taught
  sentence, beside the sourcing discipline at `:144-158`.
- Adapter sites: `crates/adapters/telegram/src/client.rs`, one media method beside
  `send_message` at `:371-391`, going through `request` at `:505-527` for the URL form, which is
  JSON and therefore fits; `crates/adapters/telegram/src/driver.rs:730-761`, where
  `consume_replies` matches the new kind; `crates/adapters/telegram/Cargo.toml` for reqwest's
  `multipart` and `stream` features and tokio's `fs`, only when the stored form is built.
- Test sites: `crates/adapters/telegram/tests/adapter/server.rs` needs a media-method script
  kind beside `SendScript` at `:41-49` and dispatch beside `:398` before AC8 and AC10 can be
  written at all; the multipart body parse at `:303-338` is only needed for AC15.
- Documents: `docs/platform-vocabulary.txt` gains the ten terms in AC2;
  `docs/privacy/records-of-processing.md` gains a category row after D9 at `:70` and an erasure
  row after `:115`; `docs/privacy/dpia.md` gains the same category near `:129`.
- Decision files: the next free number in `docs/decisions` is 0106. This unit produces several,
  and the delivery-order one is the important one to write down, because it is the promise a
  reader will otherwise assume runs the other way.
- The platform facts above were read from the live documentation and changelog on 2026-08-25 and
  quoted verbatim where they are numbers. Re-read them before implementing, full stop: the send
  methods' parameter lists changed in July and again in August 2026, so a six-week-old reading
  of this page is already stale, and the 50 MB sentence says in the platform's own words that
  "this limit may be changed in the future".
- One note about a neighbouring specification, not an edit to it. When this document was first
  written it claimed `docs/units/telegram/` held nothing else; that was false, and there are six
  siblings. `01-receiving-media.md` overlaps this unit in three places and the two must be
  reconciled by whichever merges second, not silently. It puts its own attachment vocabulary
  into `crates/core/src/message.rs`, so the two should agree on one attachment record instead of
  each keeping their own. It writes received files to disk, which makes it the caller the stored
  source in this unit is waiting for, so the stored variant belongs in whichever unit merges
  later. And it amends `records-of-processing.md`, including the "No media, no files, no voice,
  no stickers" sentence this unit reasons against; if that sentence has already changed, this
  unit's privacy reasoning must be re-read against the new text before AC12 can be judged.

## Two review claims that did not survive checking

Both reviews were checked line by line against the tree, the wiki and the live API before
anything here was rewritten. Almost everything they found is accepted above. Two claims are
recorded as refused, with the evidence, so nobody re-opens them.

- **"The page enumeration truncates parenthesised names, so the wiki has thirteen pages, not
  fifteen."** Refused. `page_name_char` (`crates/core/src/tools/wiki.rs:92-95`) accepts
  parentheses explicitly, and the rendered index writes the links unencoded. Running the
  enumeration's own scan against the live index on 2026-08-25 yields fifteen names with
  `Integrating-Sandboxed-Google-Play-(16.2)` and `Porting-from-other-ROMs-(Legacy)` intact, and
  all fifteen fetch 200 from the raw base — with literal parentheses and with percent-encoded
  ones. The wiki repository clone holds fifteen content pages plus a sidebar, which agrees. The
  404s the review saw came from names it had truncated itself.
- **"Home.md is 131 bytes."** Refused; it is 134, as the original draft said and as the other
  review independently confirmed. Measured again on 2026-08-25.
