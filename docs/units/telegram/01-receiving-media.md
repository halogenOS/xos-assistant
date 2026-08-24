# Telegram 01 — the files a member sends reach the assistant

Date: 2026-08-25, revised the same day against two independent reviews of the first draft.
Today a message with no text never reaches the core at all: translation skips it as
`Skip::NoText` (decision 0017), so a captionless photo, a voice note and an attached log file
are invisible to the assistant. This unit makes a message that carries a file a recorded
message, fetches the bytes off the platform in a stream, writes them to plain files on the
local encrypted volume, reads the media type out of the leading bytes instead of believing
the sender, and tells the model plainly that a file arrived. The bytes themselves are not
sent anywhere: showing the model a picture is the next unit, and it needs its own privacy
change. The receipts below come from the live Bot API documentation read on 2026-08-25 and
from both repositories.

## What the reviews changed, stated at the top

Two things in the first draft were not buildable, and one platform claim in it was false.
They are corrected below rather than carried:

1. **There is no way to write the message row and the framework attachment record in one
   transaction, and the draft's contract said there was.** `create_attachment` and
   `add_attachment_range` each open their own transaction inside their own store job
   (`agent-ledger/crates/agent-ledger/src/store/attachments.rs:45,154`), and `StoreTx` is a
   channel to the writer thread, not a database handle
   (`agent-ledger/crates/agent-ledger/src/store/mod.rs:178`). Atomicity across those writes
   and the block append needs a framework change this unit does not make. The unit replaces
   the promise of one transaction with **an ordering plus a sweep** (decision 0110), which is
   buildable today and repairs every crash state, including the one erasure could otherwise
   leave.
2. **A video is not one file.** `Video` carries `qualities`, a list of `VideoQuality` each
   with its own `file_id` and `file_size`. The draft claimed a member could send a video the
   assistant physically cannot fetch and that no code changes it; for video that is now often
   false, because a 2 GB video routinely offers a small rendition a bot may download. The
   size cap is still absolute for documents, audio and voice. The rendition choice moves into
   the core as one rule that covers photo sizes and video qualities alike (decision 0117).
3. **The draft mis-dated `VideoQuality` to the 10.x line.** It shipped in Bot API 9.4,
   9 February 2026. Corrected in the grounding.

Nothing else in the reviews proved the feature impossible. Where a review's claim did not
survive checking, it is answered as a rejected alternative below rather than dropped.

## Grounding

### The platform

- **The live documentation is Bot API 10.3, dated 24 August 2026** (top entry of
  `/bots/api-changelog`), not 10.1 as the brief assumed — 10.1 shipped 11 June 2026, 10.2 on
  14 July 2026, 10.0 on 8 May 2026, 9.4 on 9 February 2026.
- **Two recent releases do touch file reception, contrary to the first draft.** 10.1 added
  Rich Messages: `Message.rich_message`, whose `RichMessage` carries an array of `RichBlock`,
  and the block variants include picture, video, audio, document, animation, voice-note,
  collage and slideshow blocks — several downloadable files inside one `Message`. 10.2 added
  ephemeral messages. `VideoQuality` and `Video.qualities` came earlier, in 9.4.
- **The media fields on `Message`**, verified against the live field list in order:
  `rich_message`, `animation`, `audio`, `document`, `live_photo`, `paid_media`, `photo`
  (Array of `PhotoSize`), `sticker`, `story`, `video`, `video_note`, `voice`, then `caption`,
  `caption_entities`, `show_caption_above_media`, `has_media_spoiler`, and `media_group_id`
  earlier in the object. `caption` is documented as "Caption for the animation, audio,
  document, paid media, photo, video or voice" — paid media included.
- **`message_id`, verbatim**: "Unique message identifier inside this chat; 0 for ephemeral
  messages. In specific instances (e.g., a message containing a video sent to a big chat),
  the server might automatically schedule a message instead of sending it immediately. In
  such cases, this field will be 0 and the relevant message will be unusable until it is
  actually sent." An id of 0 is not a usable reply target.
- **`Video`, verbatim field list**: `file_id`, `file_unique_id`, `width`, `height`,
  `duration`, `thumbnail`, `cover`, `start_timestamp`, `qualities`, `file_name`, `mime_type`,
  `file_size`. `qualities` is "Optional. List of available qualities of the video", and
  `VideoQuality` carries `file_id`, `file_unique_id`, `width`, `height`, `codec`,
  `file_size`. So one video message offers several fetchable renditions of one thing, exactly
  as `photo` offers several `PhotoSize` entries.
- **`LivePhoto`, verbatim**: `photo` is "Optional. Available sizes of the corresponding
  static photo"; `file_id` is "Identifier for the **video** file which can be used to
  download or reuse the file"; `duration` is "Duration of the video in seconds as defined by
  the sender". The object's own `file_id` is the video, not the picture — a fact that decides
  which file is fetched (decision 0117).
- **Two fields alias others for backward compatibility, and the dispatch order depends on
  it.** `animation`: "For backward compatibility, when this field is set, the document field
  will also be set." `live_photo`: "For backward compatibility, when this field is set, the
  photo field will also be set." So the dispatch has to test `animation` before `document`
  **and** `live_photo` before `photo` — one rule, applied to two pairs, not one order that
  happens to work for one of them.
- **`getFile`, verbatim**: "Use this method to get basic information about a file and prepare
  it for downloading. For the moment, bots can download files of up to 20MB in size. On
  success, a File object is returned. The file can then be downloaded via the link
  `https://api.telegram.org/file/bot<token>/<file_path>`, where `<file_path>` is taken from
  the response. It is guaranteed that the link will be valid for at least 1 hour. When the
  link expires, a new one can be requested by calling getFile again." Its one parameter is
  `file_id` (String, required). The `File` object carries `file_id`, `file_unique_id`,
  optional `file_size` and **optional `file_path`**; its description repeats "The maximum
  file size to download is 20 MB".
- **The size cap is still the most important platform fact here, with one softening.**
  Telegram's own FAQ states members may send files "of up to 2 GB each", 4 GB with Premium; a
  bot may download 20 MB. For a document, an audio file, a voice note or an animation there
  is no second rendition, so **a member can send a file the assistant physically cannot
  fetch, and no code changes that.** For a photo and for a video the platform offers smaller
  renditions, so the honest rule is "fetch the largest rendition a bot may download", not
  "refuse the message". The only documented escape from the cap itself is running a local Bot
  API server ("Download files without a size limit"), a deployment change and out of scope.
- **`file_id` versus `file_unique_id`**: `file_id` is the "Identifier for this file, which
  can be used to download or reuse the file"; the documentation adds under "Sending files"
  that "file_id is unique for each individual bot and can't be transferred from one bot to
  another" and "a file can have different valid file_ids even for the same bot".
  `file_unique_id` is "supposed to be the same over time and for different bots. Can't be
  used to download or reuse the file." One is a short-lived capability, the other a stable
  name.
- **`file_size` is documented as possibly larger than 2^31**, with "at most 52 significant
  bits, so a signed 64-bit integer or double-precision float type are safe". **It is optional
  on every media object**, including `PhotoSize` and `VideoQuality` — so no rule may assume a
  declared size exists.
- **The declared media type is the sender's claim.** On `Animation`, `Audio`, `Document`,
  `Video`, `Voice` and `LivePhoto` the field is documented as "MIME type of the file as
  defined by the sender". `PhotoSize`, `VideoQuality` and `VideoNote` carry no media type at
  all. `getFile` adds: "Note: This function may not preserve the original file name and MIME
  type."
- **An album is several updates, not one.** `media_group_id` is "The unique identifier inside
  this chat of a media message group this message belongs to" — each item arrives as its own
  `Message`, and only one of them carries the caption.
- **Privacy mode decides whether media arrives at all.** A bot in a group with privacy mode
  on receives only commands aimed at it, replies to it, and service messages; ordinary media
  messages are filtered out. This is already the operator's documented obligation at
  `docs/reference/group-operator-contract.md:8-17`, and nothing in this unit works without
  it.
- Field lists used below, verbatim from the documentation: `PhotoSize` (`file_id`,
  `file_unique_id`, `width`, `height`, optional `file_size`); `Animation`, `Audio`,
  `Document` and `Video` (all with optional `file_name`, `mime_type`, `file_size`; `Audio`,
  `Video` and `Animation` with `duration`); `VideoNote` (`length`, `duration`, optional
  `file_size`, no `file_name`, no `mime_type`); `Voice` (`duration`, optional `mime_type`,
  optional `file_size`).

### Our tree

- **The skip that blocks the whole feature.** `translate.rs:165-167` returns
  `Translation::Skip(Skip::NoText)` when `text_of(message)` is `None`; the variant is at
  `translate.rs:53` and its reason line at `translate.rs:481` reads "a message with neither
  text nor caption". `text_of` (`translate.rs:466-472`) is `text` then `caption`, filtered
  non-empty — so **a captioned media message of any kind already records today**, including
  captioned paid media. Only the captionless ones are lost. Decision 0017 is what the skip
  restates.
- **The adapter decodes no media at all.** `Incoming` (`client.rs:125-144`) holds
  `message_id`, `date`, `chat`, `from`, `sender_chat`, `text`, `caption`,
  `reply_to_message`, `pinned_message` and nothing else; unknown fields are dropped by the
  decoder. The same technique is already used deliberately to keep a field out of the
  process: the sender's display name "is not decoded at all, so a display name never enters
  the process as a typed value" (`client.rs:212-215`).
- **The wire client's shapes.** Requests are built as `{root}/bot{token}/{method}`
  (`client.rs:532-545`), the token never leaves the module in an error string (module doc,
  `client.rs:1-9`, and `redact`). `REQUEST_TIMEOUT` is 35 seconds for the whole request
  (`client.rs:22`), applied on the client builder (`client.rs:291`) — the long poll plus
  transport. That timeout would abort a legitimate slow 20 MB download, so the download must
  set its own per-request timeout.
- **The rate-limit ceiling is a parameter, and zero means never park.** `request(method,
  body, wait_ceiling)` (`client.rs:505-526`) retries up to `RATE_LIMIT_ATTEMPTS = 3`
  (`client.rs:29`) and fails at once with `RateLimitWaitOverCeiling` when the stated wait
  exceeds the ceiling; `MAX_RATE_LIMIT_WAIT` is one minute (`client.rs:45`). Passing
  `Some(Duration::ZERO)` therefore means "take the answer or fail, never sleep" — which is
  what a media fetch inside the sequential batch needs.
- **The batch is strictly sequential.** `driver.rs:319-320` is `for update in &updates {
  process(...).await }`, one at a time. The codebase already reasons about this exact hazard
  at `driver.rs:395-399`: "a refused chat's flood would otherwise park the sequential batch
  one bounded wait per message." Any per-message media work is per-batch work multiplied by
  the batch size, so it needs its own budget.
- **Streaming is available with no new dependency.** In the pinned `reqwest` 0.13.4,
  `Response::chunk(&mut self) -> Result<Option<Bytes>>` is unconditional
  (`src/async_impl/response.rs:310`) — the `stream` feature and `bytes_stream` are only
  needed for the `futures_core::Stream` form (`:349-351`). `RequestBuilder::timeout`
  (`src/async_impl/request.rs:294`) overrides the client timeout for one request. A chunk
  dereferences to `&[u8]`, so `bytes` never has to be named as a dependency.
- **The core does not write files today, and its tokio is minimal.**
  `crates/core/Cargo.toml` has `tokio = { version = "1", features = ["rt", "sync"] }`; only
  `crates/assistant/src/{prompt,config,main}.rs` touch the filesystem. The core-owned sink
  changes that, so the manifest gains two tokio **features** — not a dependency (decision
  0107).
- **Where a fetch can run.** `process` (`driver.rs:359-473`) already resolves a group's
  first contact and the core's withdrawal *before* ingestion (`driver.rs:377-393`), then
  resolves authority, then calls `assistant.ingest`. `Assistant::ingest`
  (`assembly.rs:623-780`) takes the erasure fence as a read hold on its first line
  (`assembly.rs:624`), admits the channel at `:629`, resolves the writing sender at `:632`,
  takes the stamp lock at `:660`, and has **three early `return Ok(...)`** (channel refusal,
  `Disregarded` from an unresolved sender, `Disregarded` from the under-lock suppression
  re-read) plus `?` propagation throughout before the final `Ok(IngestOutcome::Recorded)`.
  There is no single exit point today; the first draft's claim that there is was wrong, and
  decision 0110 names the wrapper that creates one. A download inside `ingest` would hold the
  erasure fence for its whole duration and park every later ingestion behind a pending
  erasure.
- **The message kind and how a line is composed.** `ChatMessage` at `kind.rs:376-431`, its
  stored field map at `kind.rs:444-485`, the descriptor's column list at `kind.rs:575-597`.
  `projected_text` (`kind.rs:555-569`) returns `ERASED_MARKER` when `text` is `None`, else
  builds `{speaker}: {text}` and then wraps that whole line as `{origin mark} {speaker}:
  {text}` — the origin mark is **outermost**, not innermost as the first draft's grounding
  said. The file line therefore belongs inside `text`, ahead of the caption, and an erased
  row never reaches the composition at all. Erasure's two writes live at `kind.rs:688`
  (`erase_principal_content`) and `kind.rs:743` (`erase_message_named`, the deletion mirror).
- **`InboundMessage`'s origin is already optional.** `message.rs:203-205`: "The platform's own
  id for the message, opaque, kept for later reply threading", typed `Option<String>`. A
  message whose `message_id` is 0 needs no new shape.
- **Schema growth is append-only.** The shipped `CREATE TABLE` is frozen (decision 0026,
  restated at `schema.rs:49-51`); every change since is an appended step with its vocabulary
  list frozen at the moment it shipped (`schema.rs:113-123`, steps from `schema.rs:159`).
- **The framework's attachment store, and what it does and does not give.**
  `create_attachment(id, url, filename, mime, total_size, headers)`
  (`agent-ledger/crates/agent-ledger/src/store/attachments.rs:45`), `find_attachment` (`:79`),
  `add_attachment_range` with merge-on-insert (`:154`), `has_attachment_range` (`:226`),
  `missing_ranges` (`:254`), `update_attachment_mime` (`:283`), `delete_attachment` (`:300`).
  The module doc says "No file bytes pass through here… the caller owns the file itself"
  (`:1-4`) and `delete_attachment` states "The caller is responsible for removing the file
  from disk" (`:294-295`). `filename` is `TEXT NOT NULL` (`store/migrations.rs:301`); the
  tables are created at `store/migrations.rs:297-322`. `store/descriptors.rs:317` records
  that attachments and their sidecar tables are deliberately **not** ledger content and are
  not announced by the row-change hook — which is why writing and deleting rows there does
  not touch the append-only rule. Foreign keys are on (`store/mod.rs:282`) with
  `ON DELETE CASCADE` on both sidecars, so one `delete_attachment` removes the ranges too.
  What it does **not** give is a seam to write an attachment row inside another store job:
  each call is its own `self.run` closure with its own transaction.
- **The model wire can already carry a picture, and it whole-buffers.**
  `ContentPart::Image { mime, data: Vec<u8> }`
  (`agent-ledger/crates/agent-ledger/src/agency/projection.rs:72`), merged with the
  framework's own media-wire slice (`agent-ledger/docs/slices/09-user-authored-media-wire.md`,
  which also records that the gateway silently drops every inline audio shape for the
  configured model). `data` is a whole file in memory, which is why this unit does not use it
  (see decision 0111).
- **Four published statements this unit would make false.**
  `docs/privacy/records-of-processing.md:61` — "No media, no files, no voice, no stickers";
  the same document at `:144` — "Text only, no media, no files, no voice, no stickers, no
  edits (decision 0017)"; `docs/privacy/dpia.md:129-131` — the same sentence; and the
  member-facing `docs/privacy/bot-assistant-privacy-policy.md:20-22` — "We do not store the
  media itself". Shipping the code without the amendment is the defect, not the follow-up.
- **The prompt is a directory of files the operator ships.** `prompt_dir`
  (`crates/assistant/src/config.rs:30`). The model has never been handed a line about a file
  it cannot open, so it needs telling (decision 0118).
- **The platform-vocabulary check scans words, not concepts.** `crates/core/tests/vocabulary.rs`
  greps the core's sources for the whole words in `docs/platform-vocabulary.txt`, which lists
  platform and SDK names only. It cannot catch a platform *concept* wearing a neutral name,
  so the attachment-form vocabulary has to earn its neutrality by argument (decision 0109).
- **Configuration is one file.** `Configuration` (`crates/assistant/src/config.rs:24`) with
  `deny_unknown_fields`, `store_path` at `:26` documented "Created on first start", and the
  `Protection` sub-table at `:326-336` as the pattern for a defaulted table.
- **Decision numbers 0106 and up are free.** The highest recorded is
  `docs/decisions/0105-the-fixed-line-is-the-acknowledgments-fallback.md`, and no sibling
  telegram unit spec claims a number in this range.
- The unit also matches the media architecture the operator adopted for the assistant, kept
  outside this repository: bytes on the local disk and never in the database, the media type
  read from the bytes, and one honest line naming a file the model cannot be shown. What this
  unit builds is that architecture's lowest row — the file is named, nothing more is claimed.

## Decisions taken with this unit

- **0106 — a message that carries a file is a recorded message, 2026-08-25.** Decision 0017's
  rule ("text is what this unit records") is superseded for files: the inbound text becomes
  the caption, or the empty string when there is none, and the message is recorded with the
  file's facts beside it. `Skip::NoText` is renamed `Skip::NothingToRecord` and now means a
  message with neither text, caption nor a file this adapter carries. What still skips, named
  exactly: a captionless sticker, a forwarded story, a contact, a location, a poll, dice, a
  captionless paid-media post, and a `rich_message`. What does **not** change: a *captioned*
  message of any of those kinds already records today through `text_of`'s caption fallback
  (`translate.rs:466-472`) and still records after this unit, text only — the first draft's
  claim that the skip "covers paid media" was wrong for the captioned case, and the
  documentation's own `caption` field list names paid media. The projection never comes out
  empty, because a message with no caption always has a file line to show. *Rejected:*
  recording a captionless photo with an invented caption such as "(photo)" — the text column
  is what the person typed, verbatim (`message.rs:200-202`), and an invented sentence in it
  would be indistinguishable from a real one on every later read. *Rejected:* carrying
  stickers in the same change — a sticker is a fixed drawing from a set with its own emoji,
  closer to punctuation than to a file, and whether the emoji stands in for it is a separate
  decision. *Rejected:* carrying `rich_message` in this unit — one rich message can hold many
  file blocks, and decision 0109's one-file-per-row shape does not hold for it; it is named
  as a skip with a reason and recorded as a follow-up rather than silently swallowed by the
  catch-all. *Rejected:* carrying paid media — its files sit behind a purchase, and a bot
  fetching them is a payments question, not a media question.

- **0107 — the bytes are plain files on a configured local directory, and the database holds
  identity only, 2026-08-25.** The `[media]` table in the configuration carries `path`
  (required when the table is present), `max_total_bytes`, `fetch_timeout_seconds`
  (default 60) and `batch_fetch_budget_seconds` (default 120). Files live at
  `<path>/media/<attachment id>`, created mode 0600 through
  `std::os::unix::fs::OpenOptionsExt::mode` at open time — never created and then chmodded,
  which would leave a readable window on personal data — inside a directory created 0700, on
  the encrypted volume the operator names. A staging subdirectory `<path>/staging` holds a
  file while it is being fetched. Both directories are created and probed writable **at
  startup**; a failure there is a startup failure, not a per-message `fetch_failed` that
  would tell members for months that the platform refused when the truth is a typo in a path.
  The core gains `tokio`'s `fs` and `io-util` features for the sink — a feature on a crate
  the manifest already names, not a new dependency. *Rejected:* base64 in the ledger or a
  SQLite blob — a member's video would then ride every block scan and every backup of the
  ledger, and the operator decided the database holds no bytes in any form. *Rejected:*
  object storage — it provisions a service for a single machine that already has an encrypted
  disk. *Rejected:* `max_attachment_bytes`, which the first draft declared and never defined
  — the platform already caps a fetch at 20 MB, an operator ceiling underneath it would need
  a fifth withheld reason meaning "over the operator's own limit", and no operator has asked
  for one; `max_total_bytes` is the ceiling that matters. *Rejected:* blocking `std::fs`
  writes on the async runtime, or wrapping every chunk in `spawn_blocking` — the first
  stalls the batch on disk, the second pays a task hop per 8 KiB.

- **0108 — the media type is read from the leading bytes, never from the sender's claim,
  2026-08-25.** The `mime_type` field is not decoded by the adapter at all, exactly as the
  display name is not decoded (`client.rs:212-215`), so it cannot be used by accident. The
  core sniffs from a window of at most the first 2048 bytes, which is the only buffer held
  in flight: magic numbers first (`FF D8 FF` JPEG, the eight-byte PNG signature, `GIF87a` /
  `GIF89a`, `%PDF-`, `PK\x03\x04`, `1F 8B`, `OggS`, the EBML signature `1A 45 DF A3`, `ID3`
  and the MPEG frame sync, the ADTS sync `FF F1` / `FF F9`), then two container splits —
  `RIFF` is `image/webp` or `audio/wav` depending on bytes 8..12, and a `ftyp` box at bytes
  4..8 is read by its brand (`M4A `/`M4B ` audio, `avif`/`heic`/`heif` image, `qt  `
  QuickTime, `3gp*` 3GPP, otherwise `video/mp4`) — then a UTF-8 probe over the window that
  treats a multibyte sequence cut by the window edge as text, and finally
  `application/octet-stream`. **An empty window classifies as `application/octet-stream`**,
  not as text: a zero-byte file makes no claim to being anything. There is no content-kind
  enum: the media type is the classification. *Rejected:* trusting the declared type — it is
  the sending client's word, the documentation warns twice that it may be wrong or absent,
  and a mislabelled file would be handed to a later handler that cannot read it. *Rejected:*
  deriving the type from the file name — the extension is the same claim with less
  information behind it. *A review asked why `duration` is then trusted, since `LivePhoto.duration`
  is documented "as defined by the sender" too.* The answer is in what each one is used for,
  and it is written into the projection: the media type routes bytes to a handler, so a wrong
  one is a failure, while the length is only ever shown to the model as a stated number. The
  projection therefore prints it as the sender's claim — `18 s stated` — and never as a
  measured fact. *Rejected:* dropping the length — it is the one cheap fact that tells the
  model whether a voice note is a sentence or a lecture.

- **0109 — the file's facts are columns on the message row, with one frozen form vocabulary,
  2026-08-25.** One appended migration step adds seven nullable columns to
  `block_chat_message`: `attachment_form`, `attachment_id`, `attachment_name`,
  `attachment_media_type`, `attachment_bytes`, `attachment_seconds` and
  `attachment_withheld`. Two of them carry a vocabulary frozen in the migration step, in the
  shape `schema.rs:113-123` already requires, and both lists are given here because a list
  named and not written is not a deliverable:

  - `attachment_form` is exactly one of **`picture`**, **`video`**, **`animation`**,
    **`audio`**, **`voice_recording`**, **`file`**. Six values, no more. An animation records
    `animation` and never `file`, even though the platform sets `document` beside it; a
    document records `file`; a video note records `video`; a live photo records `picture`.
  - `attachment_withheld` is exactly one of **`too_large`**, **`fetch_failed`**,
    **`not_attempted`**, **`no_room`**, **`not_configured`**, or NULL when the bytes are
    present.

  The vocabulary is a neutral taxonomy of what a person attached, not a copy of the
  platform's field names: every value maps onto Matrix's `m.image` / `m.video` / `m.audio` /
  `m.file` plus its voice-message flag, and an adapter with no notion of a voice recording
  simply never emits that value. The two shapes with no neutral counterpart — the round video
  message and the live photo — are **mapped onto existing values rather than given their
  own**, precisely so a Telegram-only concept never becomes a column value the core has to
  know about. The kind's descriptor gains the columns, `stored_fields` writes them, and
  `projected_text` renders the file line. Columns rather than a side table is what makes the
  projection possible at all: the block loader loads a kind's own content row and nothing
  else, so a one-to-many side table would be invisible to `Projection`. Every form this unit
  carries carries exactly one file, so the columns lose nothing — the first draft justified
  this by claiming no platform puts several files in one message, which `rich_message`
  refutes; the correct justification is that `rich_message` is not carried (0106).
  *Rejected:* a separate attachment block ahead of the message — it doubles the block count
  for every media message and leaves the message block holding an empty text that the wire's
  empty-message rule may drop, and the two blocks would have to duplicate the origin,
  authority and addressing facts to stay useful. That shape is the right one the day rich
  messages or tool-produced media are carried; nothing here obstructs it.

- **0110 — one ordering and one sweep, in place of one transaction, 2026-08-25.** The first
  draft promised the ledger would record the file "in one transaction". It cannot: each
  framework attachment call is its own store job with its own transaction
  (`attachments.rs:45,154`) and `StoreTx` is a channel, not a connection (`store/mod.rs:178`).
  What this unit guarantees instead is an **ordering in which the block append is last**, so
  no crash ever leaves a message row pointing at something absent:

  1. `process` opens a sink on the core and streams the body into a staged file.
  2. The sink finishes: the bytes are flushed and the file synced.
  3. The staged file is renamed into the media directory — one atomic rename on the same
     filesystem.
  4. `create_attachment` writes the identity row; `add_attachment_range(id, 0, size - 1)`
     records the whole extent, and is **skipped for a zero-byte file**, whose range would be
     `(0, -1)`.
  5. `ingest` appends the block carrying `attachment_id`.

  A crash at any point before step 5 leaves an attachment id nothing references. The
  reconciliation that removes it is the **startup sweep**: the staging directory is emptied,
  and every file in the media directory whose name appears in no `block_chat_message.attachment_id`
  is deleted along with its framework record. One pass answers three separate failures — a
  partial write, a message refused after its file was staged, and an erasure whose unlink did
  not happen. `ingest` gets the single exit point it does not have today by becoming a thin
  wrapper around the existing body: the wrapper holds the staged file, calls the inner
  function, and promotes on `Recorded` or discards on **every other outcome and on every
  error**, including `CoreError::Store` — the first draft pinned only `Withdraw` and
  `Disregarded` and would have leaked a staged file on the error path. Two bounds keep the
  batch moving: `fetch_timeout_seconds` per fetch and `batch_fetch_budget_seconds` across one
  update batch (decision 0119). *Rejected:* fetching inside `ingest` — it holds the erasure
  fence (`assembly.rs:624`) for the download's whole duration, so one slow file would park
  every later message behind a pending erasure. *Rejected:* fetching after `ingest` returns —
  the block insert wakes the model, so an addressed media message could reach the model
  before the file's own facts exist, and correcting them afterwards means a second fact
  superseding the first for no gain. *Rejected:* dropping the framework attachment record and
  keeping only the message-row columns, which would make the ordering trivially safe — the
  sibling unit `02-sending-media.md:368` expects an outbound attachment to share one
  attachment record with this unit, and throwing the framework's store away to buy an
  atomicity the sweep already delivers would break that before it is written. *Rejected:*
  asking the framework for a combined write — a real option, but a framework change this unit
  is not scoped to make; recorded as a follow-up.

- **0111 — the model is told a file arrived and is not shown its content, 2026-08-25.** The
  projection composes one line from the recorded columns, through a single function, and
  places it **inside** the text, ahead of the caption, because `projected_text`
  (`kind.rs:555-569`) wraps the whole speaker line in the origin mark from outside. The full
  wording, given here because "the exact projected string" is a criterion:

  | form | with bytes | file line |
  | --- | --- | --- |
  | `picture` | yes | `[sent a picture, image/jpeg, 1245184 bytes]` |
  | `video` | yes | `[sent a video, video/mp4, 42 s stated, 8123904 bytes]` |
  | `animation` | yes | `[sent an animation, video/mp4, 3 s stated, 481280 bytes]` |
  | `audio` | yes | `[sent an audio file "track.mp3", audio/mpeg, 214 s stated, 5242880 bytes]` |
  | `voice_recording` | yes | `[sent a voice recording, audio/ogg, 18 s stated, 96114 bytes]` |
  | `file` | yes | `[sent a file "logcat.txt", text/plain, 245760 bytes]` |

  A name is quoted only when the platform declared one; `picture`, `voice_recording` and a
  video message never carry one, so their line has no quoted part. A length is printed only
  when the platform declared one, always with `stated` (decision 0108). When the bytes are
  absent the line names the form and the reason, in these exact words:

  | reason | file line |
  | --- | --- |
  | `too_large` | `[sent a video the assistant could not fetch: the platform does not let bots download files this large]` |
  | `fetch_failed` | `[sent a video the assistant could not fetch: the download did not complete]` |
  | `not_attempted` | `[sent a video the assistant did not fetch: it was busy with other messages]` |
  | `no_room` | `[sent a video the assistant did not fetch: its file storage is full]` |
  | `not_configured` | `[sent a video the assistant did not fetch: it keeps no files]` |

  with the form's own noun substituted (`a picture`, `an animation`, `an audio file`, `a
  voice recording`, `a file`). The file name is the only part the sender controls, so it is
  stripped of control characters and clipped to 255 characters before it is rendered. An
  erased row never reaches this function at all — `projected_text` returns `ERASED_MARKER`
  from its first line when `text` is `None` — so no file line survives erasure without any
  extra branch. No bytes reach the model provider in this unit, which is why the recipients
  section of the record of processing does not change. *Rejected:* rendering
  `ContentPart::Image` here — the variant takes `Vec<u8>` (`projection.rs:72`), so the whole
  file would sit in memory for every request that replays the conversation, against the
  standing streaming constraint, and it sends a new category of personal data to a processor,
  which is its own decision with its own privacy amendment. *Rejected:* saying nothing to the
  model about an unfetchable file — silence would read as "no file was sent", which is false.

- **0112 — one size number, so the label is never a guess, 2026-08-25.** The platform's
  documented ceiling is written "20MB" and could mean 20 000 000 or 20 971 520 bytes. This
  unit uses **20 000 000 for both the pre-refusal and the label**: a declared size at or above
  it records `too_large` with no `getFile` call, and a `getFile` refusal for anything below it
  records `fetch_failed`. One number means there is no band in which the two rules disagree —
  the first draft used the larger number for the pre-refusal and the smaller one for the
  label, and a 20 500 000-byte file satisfied both antecedents at once. **The declared size is
  optional on every media object**, so the pre-refusal often cannot fire; the sink is
  therefore given a byte allowance and stops mid-stream, recording `too_large` when the served
  bytes cross the platform ceiling and `no_room` when they cross the remaining disk allowance
  (decision 0113). No error string is ever matched: the wire client discards error bodies on
  non-success status (`docs/follow-ups.md:13-18`), so matching is not available even if it
  were wise. *Rejected:* skipping the message when the bytes cannot be fetched — the message
  happened, it may be the one an administrator later asks about, and dropping it would leave
  the group's record with a hole. *Rejected:* the larger reading, 20 971 520 — it would
  attempt a fetch the platform may refuse and then label the refusal `fetch_failed`, which is
  the dishonest label this decision exists to prevent; refusing a file in the last megabyte
  before the limit as `too_large` is true in substance.

- **0113 — the disk ceiling is enforced in the stream, not guessed before it, 2026-08-25.**
  Before opening a sink the core sums `attachment_bytes` over the rows that still hold an
  attachment id, and hands the sink an allowance of `max_total_bytes` minus that sum, capped
  at the platform ceiling. The sink counts bytes as they pass and refuses past its allowance,
  so **stored bytes never exceed `max_total_bytes`** — the first draft checked the sum before
  the size was known and could overshoot the configured ceiling by a whole file per message,
  which made the key's name a promise it did not keep. A refused sink records
  `attachment_withheld = 'no_room'` and a warning names the ceiling; recording is never
  refused. Decision 0030 — protection limits answering, never recording — is untouched.
  *Rejected:* deleting the oldest files to make room — retention is a documented decision
  (0003: message history is kept without a timer), and a disk counter quietly deleting
  members' data would be an unwritten retention policy. *Rejected:* no ceiling — a public
  group can put twenty megabytes on the disk per message, and an assistant that fills its own
  volume stops answering. *Rejected:* accepting the file and discarding it afterwards when
  the sum turns out too high — it spends the bandwidth and the disk anyway, for nothing.

- **0114 — fetching is off until the operator configures it, 2026-08-25.** With no `[media]`
  table the assistant records that a file arrived and fetches nothing
  (`attachment_withheld = 'not_configured'`). *Rejected:* on by default with a directory
  derived from `store_path` — starting to keep members' photos is a deliberate act by the
  operator, and a derived path could put them on a volume nobody chose for personal data.

- **0115 — the attachment id is opaque, adapter-supplied and checked before it becomes a
  path, 2026-08-25.** The id is composed as `<adapter name>-<stable file name>-<receipt
  instant in nanoseconds>`: the adapter's registered name, the platform's stable name for the
  file (`file_unique_id`, documented stable across bots and time), and the instant, so two
  receipts of the same file are two independent records. The core refuses any part outside
  `[A-Za-z0-9_-]` with a typed error, so an adapter-supplied string can never escape the media
  directory. The character class is chosen to **accept a real `file_unique_id`**, which is
  base64url text over exactly that alphabet — a class one character narrower would refuse
  every genuine media message while still passing every hand-written path-escape test, which
  is why the criteria pin a real id alongside the hostile ones. The short-lived `file_id` is
  used for the `getFile` call and never stored. *Rejected:* storing one copy per distinct
  file and sharing it between senders — one person's erasure would then depend on whether
  someone else sent the same bytes, and reference counting a person's right to erasure is the
  wrong shape. Deduplication stays a named follow-up.

- **0116 — erasure reaches the bytes before it forgets where they are, 2026-08-25.** The
  order inside `erasure.rs` is load-bearing and is stated here because getting it wrong loses
  the files permanently. Erasure collects the principal's attachment ids **first** — ahead of
  the column nulls, and ahead of step 2, which deletes the principal's direct conversations
  whole (`erasure.rs` module doc, step 2) and would otherwise take the rows the ids live on.
  It then unlinks each file and calls `delete_attachment` for each record, and **only then**
  nulls `attachment_name` and `attachment_id` beside the text. An unlink that fails on
  anything other than "already absent" fails the erasure step, so the person is never told
  their files are gone while they remain; because the identity rows are concluded last
  (`erasure.rs` module doc, step 3), a retried erasure finds the principal again and the
  unlink is idempotent for the files already removed. A crash between the unlink and the
  nulls leaves rows naming files that are gone, which every reader must already tolerate —
  and the startup sweep of 0110 collects the mirror-image case. The deletion mirror
  (`erase_message_named`) does the same for its one row. Nothing is rewritten: the block
  header, the form, the size and the media type stay, so the ledger still says a file was
  there, and the erased message projects only the existing marker. *Rejected:* deleting the
  content row — the ledger is append-only and personal columns are nulled, which is how every
  other personal column here already behaves. *Rejected:* nulling first and unlinking after,
  as the first draft had it — a failure or a crash in between loses the only pointer to the
  bytes, leaving the confirmation true on paper and false on disk until a sweep happens to
  run.

- **0117 — several renditions of one thing, and the core picks, 2026-08-25.** A photo arrives
  as an array of `PhotoSize`; a video arrives as a `Video` plus its `qualities` array of
  `VideoQuality`; a live photo arrives as a video `file_id` plus an optional array of static
  `PhotoSize`. These are the same shape, so they get one rule. The adapter translates a media
  message into a neutral `DeclaredFile` carrying the form, the optional name, the optional
  stated length, **the platform's fetch ceiling in bytes**, and a list of `Rendition { source
  id, stable id, pixels, declared bytes }` in the platform's own order. The **core** chooses:
  the greatest `pixels` among renditions whose declared bytes are absent or below the
  ceiling; ties broken by declared bytes descending, treating absent as zero; ties after that
  broken by the adapter's delivered order, which makes the choice total for any input. If
  every rendition declares a size at or above the ceiling, nothing is fetched and the row
  records `too_large`. A live photo offers its static sizes as renditions and its video only
  when there are none, so the fetched bytes match the recorded form wherever the platform
  gives the choice. This also removes the first draft's split where the adapter decided
  `too_large` and the core decided `no_room`: **every withheld reason is now the core's**, and
  the adapter supplies platform facts and no judgements, which is what the adapter invariant
  actually asks for. *Rejected:* fetching `Video.file_id` alone, as the first draft did — a
  2 GB video that ships a 6 MB rendition would be recorded unfetchable and the group would be
  told, in plain words, that the platform will not allow it, which is false for that video.
  *Rejected:* letting the adapter pick the rendition — it is a decision, and an adapter
  decides nothing; it is also the same decision on both platforms, so it belongs in one place.
  *Rejected:* fetching the *smallest* rendition to save disk — the assistant would keep a
  thumbnail of everything and be unable to answer about any of it.

- **0118 — the model is taught what a file line means, 2026-08-25.** A prompt file in
  `prompt_dir` gains a short passage: a bracketed file line is the assistant's own note that a
  file arrived, the assistant cannot see, hear or read the contents, and it must not describe
  or summarise them or imply it has. It may say what the line says — that a picture arrived,
  its type and its size — and it may ask the member to describe or paste the contents. A line
  naming a file the assistant could not fetch is said plainly, with the reason. Without this,
  a model handed `[sent a picture, image/jpeg, 1245184 bytes]` as an addressed member's whole
  turn will describe a picture it never saw, against decision 0096 (substantive answers come
  from lookups) and decision 0080 (answer honestly about what the assistant is). *Rejected:*
  relying on the bracket convention to be self-evident — the existing prompt teaches every
  other convention explicitly, and a convention only the code knows is a convention the model
  will break under pressure.

- **0119 — the media fetch has a budget for the whole batch, not just for one file,
  2026-08-25.** Updates are processed strictly sequentially (`driver.rs:319-320`), so any
  per-message wait is multiplied by the batch. `getFile` therefore runs through the existing
  `request` path with `Some(Duration::ZERO)` as the ceiling — the client fails at once rather
  than sleeping when the platform states a wait (`client.rs:505-526`), and a rate-limited
  `getFile` records `fetch_failed`. Each download carries `fetch_timeout_seconds` (default 60)
  as its own `RequestBuilder::timeout`, plus a per-chunk stall deadline. Across one batch the
  media work is bounded by `batch_fetch_budget_seconds` (default 120): once spent, every
  remaining media message in that batch is recorded with `attachment_withheld =
  'not_attempted'` and no fetch is made, so the assistant keeps answering. That is why the
  vocabulary carries `not_attempted` at all — calling it `fetch_failed` would say the download
  failed when it never started. *Rejected:* `Some(MAX_RATE_LIMIT_WAIT)`, as the first draft
  had it — three attempts times one minute of sleeping per media message, so a ten-item album
  parks the assistant for half an hour and a busy batch for most of a day. *Rejected:*
  fetching concurrently across the batch — it breaks the sequential ordering the offset
  bookkeeping and the stamp reasoning rest on. *Rejected:* an unbounded batch with only the
  per-file timeout — a hundred media updates at 60 seconds each is the same outage, arrived at
  more slowly.

## The unit's contract

A message carrying a photo, video, animation, document, audio file, voice note, video note or
live photo is recorded like any other message, with its caption as the text and the file's
facts on the same row; only a message carrying neither text, caption nor a file the adapter
carries is skipped, and rich messages and paid media are named skips rather than silent ones.
The adapter reports every rendition the platform offers and the platform's own fetch ceiling,
and decides nothing; the core picks the largest rendition it may download, and every reason a
file is absent is the core's own. When the operator has configured a media directory, the
adapter asks `getFile` without ever parking the batch and streams the body chunk by chunk into
a sink the core owns, which writes straight to a staged file, holds only a 2048-byte sniff
window in memory, reads the media type out of those bytes, counts the bytes against an
allowance so neither the platform ceiling nor the disk ceiling is ever exceeded, and hands
back an identity. The staged file is synced, renamed into the media directory, given its
framework attachment record, and only then named by the appended block — so the one crash
state possible is a file nothing references, which the startup sweep removes along with any
record erasure could not unlink. A message the core refuses, for any outcome and on any error,
leaves nothing on disk. What the platform will not give up is recorded as such and said
plainly. The model reads one line naming the file, its media type, its size and, for a
recording, the length the sender stated, and the prompt tells it that it cannot see the
contents; it is shown no bytes, and nothing new reaches the model provider. Erasure reaches
the files before it forgets where they are. The record of processing, the impact assessment,
the legitimate-interest assessment and the member-facing policy stop saying the assistant
keeps no media, because it now does. No new dependency, and the assistant still assesses
nothing about a file and takes no action against anyone (decision 0070 untouched).

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary check and the secret scan clean; **no new dependency in any manifest** —
  the only manifest change permitted is adding the `fs` and `io-util` features to the tokio
  entry already present in `crates/core/Cargo.toml`, and the download uses `Response::chunk`
  and `RequestBuilder::timeout` on the `reqwest` version already pinned. Pinned by a test that
  reads both manifests and asserts the dependency name set is unchanged.
- **AC2** A captionless photo is recorded: an update carrying `photo` and no text produces an
  `IngestOutcome::Recorded`. `Skip::NothingToRecord` is returned for a captionless sticker, a
  forwarded story, a location, a captionless paid-media post and a `rich_message` — each
  pinned — while a *captioned* sticker and a *captioned* paid-media post still record their
  caption as text with no attachment columns, also pinned, because that is today's behaviour
  and this unit does not change it.
- **AC3** The six forms round-trip from update to stored row, against the exact frozen
  vocabulary of decision 0109: `photo` records `picture`, `video` records `video`,
  `video_note` records `video`, `animation` records `animation` (**not** `file`, even though
  the platform sets `document` beside it), `document` records `file`, `audio` records `audio`,
  `voice` records `voice_recording`, and `live_photo` records `picture`. Pinned per form, with
  the two backward-compatibility aliases pinned as ordering tests: an update with both
  `animation` and `document` set records `animation`, and an update with both `live_photo` and
  `photo` set records `picture` and fetches from the **static** sizes, not from
  `LivePhoto.file_id`, which the documentation calls the video file. A migration test asserts
  the CHECK constraint refuses a seventh form and refuses a sixth withheld reason.
- **AC4** The rendition choice is total and prefers the largest fetchable one, pinned as one
  rule over both shapes: given a `photo` array in shuffled order the greatest width-times-
  height entry is fetched; given a `Video` declaring 40 000 000 bytes with a `qualities` entry
  declaring 6 000 000, **the quality is fetched and the row is not `too_large`**; given two
  renditions of equal pixels with no declared size, the adapter's delivered order decides and
  the same input twice gives the same choice; given every rendition declaring at or above
  20 000 000, nothing is fetched and the row records `too_large`.
- **AC5** The fetch streams, pinned on what this unit owns: a scripted server delivering a
  file in many chunks yields a file on disk byte-identical to what was served, for a file
  larger than any single chunk; `Sink::peak_retained()` never exceeds the 2048-byte sniff
  window across that transfer; and a source scan in the shape of `crates/core/tests/vocabulary.rs`
  asserts the adapter's download path calls neither `Response::bytes` nor `Response::text`.
  What `reqwest`'s transport retains internally is outside this unit and is not claimed.
- **AC6** The media type comes from the bytes: a file served with a declared type of
  `text/plain` whose bytes begin with the PNG signature is recorded `image/png`; a `RIFF`
  container is split into `image/webp` and `audio/wav` by its bytes 8..12; a `ftyp` box with
  an `M4A ` brand records `audio/mp4`; plain UTF-8 text with a multibyte character cut by the
  window edge records `text/plain`; a zero-byte file records `application/octet-stream` and no
  attachment range; unrecognised bytes record `application/octet-stream`. Each pinned, plus a
  source scan asserting the string `mime_type` appears nowhere in `crates/adapters/telegram/src/client.rs`.
- **AC7** The platform ceiling is honoured and named, on one number: a message declaring a
  file at or above 20 000 000 bytes records `attachment_withheld = 'too_large'` with **no
  `getFile` request** reaching the scripted server (pinned by asserting the server saw no
  `getFile` call, not that it saw no request at all — it also serves `getUpdates`); a `getFile`
  refusal for a file declaring 1 000 000 bytes records `fetch_failed`; a media object
  declaring **no** size whose served body crosses 20 000 000 bytes mid-stream records
  `too_large`; a `getFile` that succeeds with no `file_path` records `fetch_failed`; and a
  rate-limited `getFile` records `fetch_failed` **without the test clock advancing**, proving
  the batch was not parked. Each projected line asserted against the exact wording in
  decision 0111.
- **AC8** The ordering holds and the sweep repairs it. After ingestion the message row carries
  the form, the sniffed type, the true byte count and the attachment id; the framework
  attachment record exists with that id, that type, that size and a `url` of `None`; and
  `has_attachment_range(id, 0, size - 1)` is true. When the platform declared no file name —
  the flagship captionless-photo case — `attachment_name` is NULL on the message row and the
  framework's NOT NULL `filename` column holds the empty string, and the projected line quotes
  no name; pinned, because the framework column cannot be null and an invented name would be
  indistinguishable from a real one. The bot token appears in no stored column. Separately
  pinned: a file promoted into the media directory with no block appended is deleted by the
  startup sweep together with its framework record, and a file that **is** referenced survives
  the sweep.
- **AC9** A refused message leaves nothing behind, on every exit: a message from an unadmitted
  group (`Withdraw`), one from a suppressed sender (`Disregarded`), one from a sender the
  under-lock re-read suppresses, and one whose append fails with `CoreError::Store` each leave
  no file in the media directory, no file in staging and no attachment record — four pins, one
  per exit of the wrapper, with the error path pinned explicitly because it is the one a
  `?` would leak through.
- **AC10** The model reads one honest line: a captionless picture projects as the file line
  alone inside the origin mark and speaker prefix, in the order `{origin mark} {speaker}: {file
  line}`; a captioned one projects the file line then the caption; a voice recording projects
  its stated length with the word `stated`; each of the five withheld reasons projects its own
  wording; an erased message projects only the existing erased marker and no file line. Each
  pinned against the exact strings tabulated in decision 0111, with a file name of 400
  characters proven clipped to 255 and stripped of control characters.
- **AC11** Erasure reaches the disk in the right order: after `erase_principal` the person's
  files are gone from the media directory, their framework attachment records are gone,
  `attachment_name` and `attachment_id` are NULL while the form, size and media type remain,
  and a second erasure reports completion without failing. Pinned again for a media message in
  a **direct** conversation, proving the ids were collected before the conversation deletion of
  step 2 took the rows. Pinned again for a failing unlink: the erasure reports the failure, the
  columns are not yet nulled, and a retry succeeds. Pinned again for the deletion mirror's
  single row.
- **AC12** The ceilings are ceilings: with the stored sum one megabyte below `max_total_bytes`,
  a served file of two megabytes is **cut off mid-stream**, records `no_room`, and leaves the
  stored total at or below `max_total_bytes` — the assertion is on the total, not on the label,
  because that is the property the config key promises. With `max_total_bytes` already reached,
  a further media message records `no_room` and makes no `getFile` call. With no `[media]`
  table, every media message records `not_configured` and the assistant still answers normally.
- **AC13** The path cannot escape, and a real id is not refused: an adapter-supplied stable
  name containing `/`, `..` or a null byte is refused by the core with a typed error and
  recorded as `fetch_failed`, and no file is written outside the media directory; **and** a
  genuine base64url `file_unique_id` containing both `-` and `_` is accepted and its file
  written, pinned as its own case, because a character class one class too strict would pass
  every hostile test while refusing every real message. Files are proven created 0600 by mode
  at open, in a directory created 0700, with no window in which the mode is wider.
- **AC14** Startup owns the directories: with `[media]` configured at an unwritable or absent
  path, the assistant **fails to start** with a message naming the path, rather than starting
  and recording `fetch_failed` on every media message; with a writable path, both directories
  exist at 0700 after start and staging is empty.
- **AC15** Nothing is logged that names a person's file: a completed fetch, a failed fetch and
  a refused id each emit log lines containing the attachment id, the form, the media type and
  the byte count and **not** the file name and **not** any URL — pinned by capturing the
  tracing output and asserting the sender-supplied name and the token are absent.
- **AC16** The published privacy documents match the running system in the same commit:
  `records-of-processing.md` gains a media category and loses the "No media, no files, no
  voice, no stickers" sentence at `:61` **and** the "Text only, no media, no files, no voice,
  no stickers" data-minimisation line at `:144` — both, because leaving either one shipped
  leaves a false published statement; `dpia.md` 3.2, 3.6 and 3.7 gain a dated addendum
  covering media as a higher-sensitivity category, the local encrypted storage, the ceilings,
  the erasure path, **and the transient staged write of a file belonging to a message that is
  afterwards refused** — a real processing step the assessment must describe; `lia.md` 4.1,
  4.2 and 5 cover files as well as text; the member-facing policy stops saying "We do not
  store the media itself" and says what is kept and that deletion removes it.
  `docs/compliance/ai-act.md` is checked and stated unchanged, because no file content reaches
  the model in this unit.
- **AC17** The record of the work exists: `docs/decisions/0106-*.md` through `0119-*.md` each
  carry `Date: 2026-08-25` and a `## Rejected alternatives` section, pinned by a test in the
  shape of `crates/assistant/tests/docs.rs:379-399`; and every follow-up named below is
  appended to `docs/follow-ups.md` naming this unit, in the form its header requires.

## Notes for launch

- Branches from `main` into its own worktree; builds against the agent-ledger checkout as it
  stands. **This unit needs no framework change** — the attachment store and its byte ranges
  already exist and are used as they are. It also gets no atomicity from them; decision 0110
  is what replaces that.
- Adapter sites: `client.rs` — add the media objects to `Incoming` (a shared shape with
  `file_id`, `file_unique_id`, optional `file_name`, optional `file_size` as `i64`, optional
  `duration`, plus the `qualities` array on video and the `photo` array on live photo;
  **no `mime_type` field on any of them**), add `get_file` through the existing `request` path
  with `Some(Duration::ZERO)` so it never parks the sequential batch, and add a
  `download_file` that GETs `{root}/file/bot{token}/{file_path}` with its own
  `RequestBuilder::timeout` and the same `redact` on every error; `translate.rs:53,165-167,481`
  — rename the skip, add the named skips for `rich_message` and captionless paid media, build
  the neutral `DeclaredFile` with its renditions and the platform ceiling, and record
  `origin = None` when `message_id` is 0; `driver.rs:377-425` — open the sink after the
  resting-withdrawal check and before authority resolution, hold the batch budget across the
  `for` loop at `driver.rs:319-320`, stream, then build the `InboundMessage`.
- Core sites: `message.rs` — `AttachmentForm`, `WithheldReason`, `Rendition`, `DeclaredFile`,
  `InboundAttachment` and the `attachment` field on `InboundMessage`; a new `media` module
  owning `Sink` (with `write_chunk`, `finish`, `peak_retained`, its byte allowance and its
  sniff window), the sniff table, the rendition choice of 0117, the id validation of 0115, the
  directory layout and the startup sweep; `kind.rs:444-485,555-569,575-597` — the stored
  fields, the file line composed inside the text ahead of the caption, the descriptor columns;
  `kind.rs:688,743` — both erasure writes plus the id collection; `erasure.rs` — the id
  collection ahead of step 2, then the unlinks and `delete_attachment` calls, then the nulls,
  in that order; `schema.rs` — one appended step adding the seven columns with the two frozen
  vocabulary lists of 0109 and a covering index on `(attachment_id, attachment_bytes)` for the
  total-bytes sum; `assembly.rs:623` — `ingest` becomes a wrapper holding the staged file
  around the existing body, promoting on `Recorded` and discarding on every other outcome and
  every error.
- Configuration and prompt: a `[media]` table in `crates/assistant/src/config.rs` beside
  `Protection`; the startup creation, permission and writability check plus the sweep in
  `crates/assistant/src/main.rs`; the passage of decision 0118 in the file under `prompt_dir`
  that carries the assistant's standing conventions; and a line in the deployment notes that
  the directory must be on the encrypted volume.
- **Cross-unit ordering, stated because six sibling specs were written the same day and this
  one is not free to edit them.** `03-editing-messages.md:107,250` rewrites the same sentence
  at `bot-assistant-privacy-policy.md:22` that AC16 rewrites; whichever unit merges second
  must read the merged text rather than the drafted text. `02-sending-media.md:368` expects an
  outbound attachment to share one framework attachment record with this unit; this unit
  writes inbound records only and claims no ownership of that shared shape — the sending unit
  should define it, and decision 0110's ordering is what an inbound record guarantees.
  `02-sending-media.md:109` cites the framework path correctly as
  `agent-ledger/crates/agent-ledger/src/store/attachments.rs`; the first draft of this spec
  cited it a level short, and the citations above are corrected.
- Named follow-ups, recorded and not built: `rich_message` carries several files in one
  platform message and is skipped, so carrying it means the separate-attachment-block shape
  decision 0109 rejected for the single-file case; albums are not grouped (each item of a
  `media_group_id` set is its own message, and grouping needs a neutral "sent together" key in
  the core, not a buffer in the adapter, which would put a decision where the invariant forbids
  one); thumbnails are not fetched, though `Video.thumbnail` and `Video.cover` are the cheapest
  way to let the model see something of a video it may not download; `has_media_spoiler` is not
  carried, and a spoiler is a member's explicit request not to have something shown, which
  deserves its own decision before any unit shows the model a picture; deduplication by the
  platform's stable file name is not done (decision 0115); the framework has no way to write a
  block and its attachment record in one transaction, which decision 0110 works around with an
  ordering and a sweep and which a framework change could close properly; the 20 MB ceiling can
  only be lifted by running a local Bot API server, which is a deployment choice for the
  operator; and showing the model a picture is the next unit, which must answer the
  whole-buffer shape of `ContentPart::Image` and carries its own privacy amendment for a new
  category of data reaching the processor.
