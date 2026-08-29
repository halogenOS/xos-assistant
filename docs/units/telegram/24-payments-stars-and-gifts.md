# Telegram unit 24 — payments, Stars, paid media and gifts: the assistant handles no money

Date: 2026-08-27. This unit examines the platform's whole commerce surface — invoices, the
pre-checkout and shipping answers, Telegram Stars with their transactions, refunds and
subscriptions, paid media, and the gift methods — and declines all of it. Like unit 08 it
spends its implementation making the decline a property of the code instead of an accident of
it, and unlike unit 08 it has to, because there is no switch anywhere outside this repository
that keeps it off: a bot can take money in Stars with no configuration at all, so the only
thing standing between this assistant and a member's wallet is the code.

Three findings force the refusal, and each one is sufficient on its own. The first is a hard
real-time contract: a pre-checkout query must be answered inside ten seconds or the payment
is cancelled, and this adapter's inbound path is a strictly sequential batch carrying waits
that reach a minute per attempt on one lookup and are unbounded on another — no model turn and
no member's message may ever sit inside that window. The second is legal: a community
assistant that takes money is a different kind of controller than one that answers questions,
and the receipt it receives carries an order and possibly a postal address, which German tax
law says must be kept for years and the published privacy notice promises to delete on
request. The third is smaller and sharper: every gift and Star method spends from the bot's
own balance, irreversibly, and nothing in the mechanism puts a human between a model reading
members' text and money leaving.

## The three findings

### 1. Ten seconds is a real-time contract, and this inbound path cannot enter it

The platform states the deadline twice, in the reference and in both payment guides:
"**The Bot API must receive an answer within 10 seconds after the pre-checkout query was
sent**" (`answerPreCheckoutQuery`), and "Your bot must reply using answerPrecheckoutQuery
within 10 seconds after receiving this update or the transaction is canceled" (the payments
guides for physical and for digital goods alike). There is no retry and no extension.

Our poll cannot promise anything of the kind, and the reasons are structural, not
tunable:

1. **The batch is strictly sequential.** `poll_loop` iterates one update at a time and
   `process` is awaited to completion for each (`driver.rs:318-323`). One long poll returns
   up to a hundred updates.
2. **A single update can stall for minutes, and one of its two lookups has no ceiling at
   all.** A group's first contact runs `get_chat` under the rate-limit wait ceiling
   `MAX_RATE_LIMIT_WAIT`, one minute (`client.rs:45`, `client.rs:334-338`) — a ceiling *per
   stated wait*, applied across three attempts (`client.rs:29`), so the bounded case is
   already minutes rather than a minute. The administrator fetch behind `authority_for` is
   **not** bounded the same way: `chat_administrators` passes `None` as its ceiling
   (`client.rs:469`), and `request` refuses only where a caller supplies one
   (`client.rs:517`). The module says so in terms: "The callers that park nothing honor
   whatever the limiter states: the identity fetch and the poll ... and the administrator
   fetch, whose failure leaves the message authority-unresolved" (`client.rs:498-504`). So a
   rate-limited administrator fetch honours whatever the limiter states, twice, inside the
   sequential batch. This is a deliberate and correct choice for its own caller — waiting is
   strictly better than leaving a message authority-unresolved — and it is fatal to a
   ten-second promise made by anything queued behind it. The ceiling's own comment gives the
   reason ceilings exist where they do: an unbounded stated wait "would park every later
   reply" (`client.rs:41-42`), and `get_chat`'s own documentation says the same for the batch,
   "would park every later update" (`client.rs:330-332`).
3. **A halted batch redelivers everything behind it.** A transient ingest failure breaks the
   loop without advancing the offset, sleeps `POLL_BACKOFF` — two seconds (`driver.rs:71`) —
   and re-polls from where it was (`driver.rs:322-341`). A pre-checkout query queued behind
   a message that halts is delivered again after the pause, by which time its ten seconds
   have been spent on somebody else's message.
4. **The model is nowhere near fast enough, by this repository's own numbers.** The shortest
   bounded model call in the tree is the rules acknowledgment at
   `GENERATION_TIMEOUT: Duration = Duration::from_secs(10)` (`acknowledgment.rs:40`) — the
   whole budget, for the smallest completion we make. The ordinary answering path is a
   streamed turn that stops and resumes once per tool call (decision 0103), which is why a
   composing cue exists at all (`crates/core/src/composing.rs`, decisions
   `0064-the-composing-signal-rides-the-turn-lifecycle.md` and
   `0102-the-composing-cue-tracks-the-model.md`; the platform half is unit 16's).

So an implementation would have to answer deterministically, from facts already stored before
the query arrived, on a path that cannot queue behind a member's message. That means a second
inbound route with its own deadline discipline beside the one route this adapter has. And
answering is a decision — "Specify True if everything is alright (goods are available, etc.)
and the bot is ready to proceed with the order" — which the first project invariant keeps out
of the adapter, so the decision belongs in the core, which would then need an order, a price
and a deadline in its vocabulary. The core today has messages, observations, assessments and
replies, and one command family; nothing in it can express "this order may proceed" and
nothing in it owns a hard deadline.

The redeeming detail is the one that makes the refusal safe and not merely honest: **not
answering cancels the transaction.** The member's money is not taken and the platform tells
them so. Silence is the correct answer for a bot that sells nothing, which is why the
fail-safe skip specified below is enough on its own.

### 2. The receipt is data we would be obliged to keep and have promised to delete

`SuccessfulPayment` carries `invoice_payload`, `total_amount`, `currency`,
`telegram_payment_charge_id`, `provider_payment_charge_id`, an optional `shipping_option_id`
and an optional `order_info`. `OrderInfo` is `name`, `phone_number`, `email` and
`shipping_address`; `ShippingAddress` is `country_code`, `state`, `city`, `street_line1`,
`street_line2` and `post_code`. And the postal address does not wait for the payment: a
`ShippingQuery` carries `id`, `from`, `invoice_payload` and `shipping_address` — a member's
home address, delivered on a query the bot is expected to answer, before any money moves.

Four things in this repository become untrue on the day that lands:

- **The purposes.** The record of processing names three purposes, P1 answering questions, P2
  reading context, P3 keeping the assistant available, every one on Article 6(1)(f)
  (`docs/privacy/records-of-processing.md` §3). A sale is a contract: Article 6(1)(b) for
  performing it, Article 6(1)(c) for the records the law then obliges. Neither basis appears
  anywhere in the activity today, and the record says plainly that "Consent is not used and is
  not collected anywhere in this activity".
- **The erasure promise.** Erasure nulls the personal columns of a person's messages and
  removes their direct conversations (`erasure.rs:1-30`, decision 0003), and the notice
  members read says "We keep messages until somebody asks us to delete them", with exactly one
  named exception, and "ask, and it goes"
  (`docs/privacy/bot-assistant-privacy-policy.md:108-118`). A payment record is the first row
  the operator would be forbidden to delete: accounting vouchers must be kept eight years
  under §147(3) of the German Fiscal Code since the Fourth Bureaucracy Relief Act took effect
  on 1 January 2025 (previously ten; read at dejure.org on 2026-08-27), and Article 17(3)(b)
  GDPR withholds the erasure right where processing is necessary for compliance with a legal
  obligation. The ledger has no vocabulary for a row that erasure may not reach, and erasure
  has one answer per row today. Giving it a second answer for one row type doubles the erasure
  model to serve a capability nobody has asked for — the same structural argument unit 08
  makes about admission.
- **The recipients.** For a non-Stars sale a payment provider joins the recipients table as an
  independent controller; for a Stars sale the platform itself collects the money and pays out
  later. The record's §6 lists the model processor, its sub-processors, the chat platform and
  the other members of the group. Neither case is in it.
- **The balancing.** The legitimate-interests assessment rests on "Messages people chose to
  post to an open community group ... not private correspondence, not observed behavior"
  (`docs/privacy/lia.md:117-119`). A postal address given to complete an order is none of
  those things, and it is not covered by an assessment about reading a group's messages. A
  sale does not belong under the legitimate-interest analysis at all; it needs its own.

Beyond data protection, the platform's own documents put the whole merchant burden on the
operator, in terms: "You as the bot owner have full responsibility in case any conflicts or
disputes arise"; "You are solely responsible for processing and rectifying legitimate user
disputes for digital goods and services sold by your bots"; and "You must notify your users
that Telegram support or bot support will not be able to help them with purchases made via
your bot". The Stars guide's Live Checklist requires a `/terms` route ("make sure your bot can
respond to a /terms command (or offers a similarly easy way of accessing your Terms and
Conditions)") and a `/support` route ("Your bot must provide support for its customers, either
by responding to a /support command or by some other clearly communicated means"); the
`/paysupport` obligation is real but sits in the guide's FAQ, not in the Live Checklist —
"Your bots and mini apps must be able to respond to the command `/paysupport` and process user
requests regarding payment issues" (all four re-read 2026-08-27). That is a trader with
consumer duties, a support obligation, a refund policy and a tax retention regime. The
operator today is a private controller running a community assistant on legitimate interest,
with none of those things. This is what "a different legal entity" means concretely, and it is
why the answer is no in code before it is ever a product question.

There is also a way the operator can end up owing money without any of this being deliberate:
`SuccessfulPayment` warns that a buyer's chargeback "may be debited from your balance ...
outside of Telegram's control", `StarTransaction` repeats it for Stars bought through Apple or
Google, and `StarAmount.amount` is documented as being able to go negative. A community
assistant whose balance can go negative is a liability with a chat interface.

### 3. Spending is an effect, and nothing puts a human in front of it

Every outbound commerce method spends from the bot's own balance: `sendGift` (the gift "can't
be converted to Telegram Stars by the receiver"), `giftPremiumSubscription` (1000, 1500 or
2500 Stars for three, six or twelve months), `transferBusinessAccountStars` (1 to 10000),
`sendPaidMedia` crediting proceeds either way, and the `allow_paid_broadcast` parameter that
spends 0.1 Stars per message. `refundStarPayment` refunds an incoming payment; nothing
reverses a gift.

Tools are product behaviour and live in the core (decision 0040); they are admitted per
conversation and checked at the call against the turn's provenance (`tools/mod.rs:14-24`).
A spending tool would be the first tool in this project whose failure mode is money, invoked
by a model whose input is text that members write and that the project already treats as
capable of misleading it — the moderation teaching and unit 16's lookup discipline exist for
that reason. Decision 0070 settles that the assistant assesses and a human decides, and states
the principle for moderation effects: "Any future administrative tool ... ships only behind a
mechanism where a human approves the concrete action before it takes effect". Moving value is
an effect of the same kind, and it is irreversible in a way a report is not. This unit records
the extension instead of assuming it.

Unlike the two refusals above, this one already has a check standing behind it, and this unit
does not have to build it: `crates/core/tests/spine/tools.rs:653-705` asserts that a created
conversation's palette "names exactly the registered set" — the three lookups plus the
always-registered privacy tool — for a direct and a group conversation alike. A spending tool
registered anywhere in the core fails that assertion by existing. The unit cites it rather than
adding anything, which also keeps the diff off `crates/core/` (AC1).

## Grounding

### The platform, read 2026-08-27

Fetched from `core.telegram.org/bots/api`, `core.telegram.org/bots/payments`,
`core.telegram.org/bots/payments-stars` and the changelog at
`core.telegram.org/bots/api-changelog` on 27 August 2026. Every sentence in quotation marks
was read from those pages on that date. The brief for this series named Bot API 10.1
(11 June 2026) as current; the changelog's newest entry is **Bot API 10.3, dated
24 August 2026**, with 10.2 on 14 July 2026 — the same correction unit 08 recorded.

**The methods, with their real parameters and limits.**

- **`sendInvoice(chat_id, message_thread_id, direct_messages_topic_id, title, description,
  payload, provider_token, currency, prices, max_tip_amount, suggested_tip_amounts,
  start_parameter, provider_data, photo_*, need_name, need_phone_number, need_email,
  need_shipping_address, send_phone_number_to_provider, send_email_to_provider, is_flexible,
  disable_notification, protect_content, allow_paid_broadcast, message_effect_id,
  suggested_post_parameters, reply_parameters, reply_markup)`** returns the sent `Message`.
  `title` is 1-32 characters, `description` 1-255, `payload` "1-128 bytes ... not displayed to
  the user". `provider_token`: "Pass an empty string for payments in Telegram Stars".
  `currency`: "Pass 'XTR' for payments in Telegram Stars". `prices` "Must contain exactly one
  item for payments in Telegram Stars". `reply_markup`: "If empty, one 'Pay total price'
  button will be shown. If not empty, the first button must be a Pay button."
- **`createInvoiceLink`** — "Returns the created invoice link as String on success", the same
  product parameters plus `business_connection_id` ("For payments in Telegram Stars only") and
  `subscription_period`: "The number of seconds the subscription will be active for before the
  next payment ... Currently, it must always be 2592000 (30 days) if specified. Any number of
  subscriptions can be active for a given bot at the same time ... Subscription price must no
  exceed 10000 Telegram Stars."
- **`answerShippingQuery(shipping_query_id, ok, shipping_options, error_message)`** returns
  True. It arrives only "If you sent an invoice requesting a shipping address and the parameter
  is_flexible was specified", and the physical-goods guide says "The bot must respond using
  answerShippingQuery either with a list of possible delivery options and the relevant delivery
  prices, or with an error". **No answer deadline is documented for it** — the reference's only
  ten-second sentence sits on `answerPreCheckoutQuery`, and both guides carry the ten seconds
  only there. The shipping answer is therefore obligatory with no stated bound, which is worse
  to design against than a number and not better: an obligation whose deadline is undocumented
  can be tightened by the platform without a changelog entry.
- **`answerPreCheckoutQuery(pre_checkout_query_id, ok, error_message)`** returns True, with the
  ten-second note quoted above. `error_message` is "Required if ok is False ... Telegram will
  display this message to the user."
- **`getMyStarBalance`** — "Requires no parameters. On success, returns a StarAmount object."
  `StarAmount` is `amount` ("rounded to 0; can be negative") and an optional `nanostar_amount`.
- **`getStarTransactions(offset, limit)`** — limit "between 1-100 ... Defaults to 100",
  returning `StarTransactions`, a list of `StarTransaction` with `id`, `amount`,
  `nanostar_amount`, `date`, and a `source` or `receiver` of type `TransactionPartner`.
  `TransactionPartnerUser` carries the full `User`, the `transaction_type` ("invoice_payment",
  "paid_media_payment", "gift_purchase", "premium_purchase", "business_account_transfer"), the
  `invoice_payload`, the `paid_media`, the `gift` and the `subscription_period`.
- **`refundStarPayment(user_id, telegram_payment_charge_id)`** — "Refunds a successful payment
  in Telegram Stars."
- **`editUserStarSubscription(user_id, telegram_payment_charge_id, is_canceled)`** — "cancel or
  re-enable extension of a subscription paid in Telegram Stars".
- **`sendPaidMedia(business_connection_id, chat_id, message_thread_id,
  direct_messages_topic_id, star_count, media, payload, caption, parse_mode, caption_entities,
  show_caption_above_media, ...)`** — `star_count` is "The number of Telegram Stars that must
  be paid to buy access to the media; **1-25000**", `media` is "up to **10** items" of
  `InputPaidMedia`, `payload` "0-128 bytes". "If the chat is a channel, all Telegram Star
  proceeds from this media will be credited to the chat's balance. Otherwise, they will be
  credited to the bot's balance." `InputPaidMediaPhoto` and `InputPaidMediaVideo` take a
  `file_id`, an HTTP URL or `attach://<file_attach_name>` with multipart upload;
  `InputPaidMediaLivePhoto` takes no URL form.
- **`getAvailableGifts`** — "Requires no parameters. Returns a Gifts object", each `Gift`
  carrying `id`, `sticker`, `star_count`, optional `upgrade_star_count`, `total_count` and
  `remaining_count` for limited gifts, and `personal_total_count` / `personal_remaining_count`
  for what this bot may still send.
- **`sendGift(user_id | chat_id, gift_id, pay_for_upgrade, text, text_parse_mode,
  text_entities)`** — "Sends a gift to the given user or channel chat. **The gift can't be
  converted to Telegram Stars by the receiver.**" `text` is 0-128 characters; "limited gifts
  can't be sent to channel chats".
- **`giftPremiumSubscription(user_id, month_count, star_count, text, ...)`** — `month_count`
  "must be one of 3, 6, or 12", `star_count` "must be 1000 for 3 months, 1500 for 6 months, and
  2500 for 12 months".
- **The business-account gift methods are unreachable without a business connection.**
  `getBusinessAccountGifts`, `convertGiftToStars`, `upgradeGift`, `transferGift`,
  `getBusinessAccountStarBalance`, `transferBusinessAccountStars` and
  `setBusinessAccountGiftSettings` each take a required `business_connection_id` and each names
  a business bot right it requires — `can_view_gifts_and_stars`, `can_convert_gifts_to_stars`,
  `can_transfer_and_upgrade_gifts`, `can_transfer_stars`. A connection is established by a
  person connecting the bot to their own Telegram Business account, and it is reported through
  the `business_connection` update type.
- **Two gift methods need no connection and no permission at all.** Bot API 9.3 added
  `getUserGifts(user_id, ...)` — "Returns the gifts owned and hosted by a user", returning
  `OwnedGifts`, with a `limit` of 1-100 — and `getChatGifts(chat_id, ...)`. For `getUserGifts`
  the documentation names no right, no membership and no consent requirement at all: any bot
  holding a token can enumerate what any user owns, by numeric id. `getChatGifts` names one
  right, and only as a filter on the answer, not as a precondition for calling it: its
  `exclude_unsaved` is "Always True, unless the bot has the can_post_messages administrator
  right in the channel" — so an unprivileged bot still gets the chat's saved gifts.
- **Two Stars-charging methods on the administrative surface belong to unit 19, not to this
  unit.** `createChatSubscriptionInviteLink` mints a link whose holder pays "The amount of
  Telegram Stars a user must pay initially and after each subsequent subscription period to be
  a member of the chat; 1-10000", and `editChatSubscriptionInviteLink` edits it; both require
  only `can_invite_users` and both are channel-only. Unit 19 grounds them
  (`19-chat-administration.md:98-99, 151-154`) and refuses them by name on the shared list
  (`19-chat-administration.md:469-480`). This unit names them so that "the Stars surface" is
  closed on paper as well as in the tree, and claims neither.

**The updates, and which of them arrive by default.**

- Four update types carry commerce: `shipping_query` ("New incoming shipping query. Only for
  invoices with flexible price"), `pre_checkout_query` ("New incoming pre-checkout query.
  Contains full information about checkout"), `purchased_paid_media` ("A user purchased paid
  media with a non-empty payload sent by the bot in a non-channel chat", carrying
  `PaidMediaPurchased{from, paid_media_payload}`), and — added in Bot API 10.2 on
  14 July 2026 — `subscription`, carrying `BotSubscriptionUpdated{user, invoice_payload,
  state}` where state is "canceled", "active" or "failed".
- **All four are in the platform's default set.** `getUpdates` says: "Specify an empty list to
  receive all update types except `chat_member`, `message_reaction`, and
  `message_reaction_count` (default)." So a bot that does not name its `allowed_updates`
  receives pre-checkout queries. The same paragraph carries the transition warning this unit's
  fail-safe exists for: "this parameter doesn't affect updates created before the call to
  getUpdates, so unwanted updates may be received for a short period of time."
- **Nine commerce facts ride the ordinary `message` update type**, which this adapter does
  subscribe to: `invoice`, `successful_payment`, `refunded_payment`, `gift` (a `GiftInfo`),
  `unique_gift` (a `UniqueGiftInfo`), `gift_upgrade_sent` ("Service message: upgrade of a gift
  was purchased after the gift was sent"), and the three suggested-post money notices
  `suggested_post_paid` ("Service message: payment for a suggested post was received"),
  `suggested_post_refunded` ("Service message: payment for a suggested post was refunded") and
  `suggested_post_approval_failed` ("Service message: approval of a suggested post has
  failed"). Beside them ride the service message `paid_message_price_changed` and the
  per-message field `paid_star_count` ("The number of Telegram Stars that were paid by the
  sender of the message to send it"). `RefundedPayment` carries `currency` ("Currently, always
  'XTR'"), `total_amount`, `invoice_payload` and the charge ids. The enumeration is written
  out in full because it is what a later unit widening the message decode has to check itself
  against; which of the nine this unit pins, and why the rest are named rather than pinned, is
  in the decisions below.
- **`InlineKeyboardButton.pay`** — "Specify True, to send a Pay button ... This type of button
  must always be the first button in the first row and can only be used in invoice messages."
  This is the door from unit 07's keyboards into commerce.
- **`InputInvoiceMessageContent`** exists so an inline result can be an invoice; unit 08
  already refuses inline mode, which closes that door for a second reason.

**What the platform requires of the seller.**

- Stars are mandatory for digital goods: "all transactions must be carried out in Telegram
  Stars, with currency tag XTR", and "Payments for digital goods and services must be carried
  out exclusively in Telegram Stars ... Telegram cannot display your bot or mini-app to mobile
  users if you attempt to sell digital goods and services via other currencies".
- **No configuration is required to start taking money.** "You may find that some API methods
  for Payments request a `provider_token`. This parameter is only needed for sales of physical
  goods and services – for digital ones, you can leave it empty." The BotFather Payments
  setting exists only for third-party providers. This is the exact inverse of unit 08's
  situation, where the switch lives in BotFather and no code can reach it: here the switch is
  the code.
- The seller obligations and where each is documented are quoted in finding 2: `/terms` and
  `/support` in the Stars guide's Live Checklist, `/paysupport` in its FAQ.
- The physical-goods page adds, read the same day: "Telegram does not process payments from
  users and instead allows developers to integrate directly with different third-party payment
  providers", and a special note that "Due to Apple's limitations, bot developers are currently
  not allowed to accept payments for digital goods and virtual services from iOS users" —
  carrying its own later amendment, "UPD 2024: Thanks to recent changes in the Apple Review
  Guidelines, users will soon be able to pay for digital goods and services with Telegram Stars
  on all platforms". Both halves are quoted because the note alone would overstate the
  restriction; the point that survives the amendment is that the platform an operator sells
  through is subject to two app stores' rules, which the operator does not control and cannot
  read from any API.

### Our tree, at `7fb217d`

- **The poll names its update types on every request, so none of the four commerce updates
  arrives today.** `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`client.rs:103`), passed as `allowed_updates` on every poll (`client.rs:311-321`) because
  "an absent selection would inherit whatever an earlier setting left on the token"
  (`client.rs:99-102`). The existing wire test asserts the exact array
  (`crates/adapters/telegram/tests/adapter/group_context.rs:22-35`). Because the platform's
  default set *includes* the commerce types, this explicit naming is not a detail — it is the
  whole reason a pre-checkout query never reaches this process.
- **A commerce update that arrived anyway is already skipped, by accident and not by
  design.** The decoded `Update` has three optional payload fields beside its id
  (`client.rs:105-121`); an update carrying only `pre_checkout_query` decodes with all three
  absent, reaches `Translation::Skip(Skip::NonMessage)` (`translate.rs:126-127`), and `process`
  acknowledges it and advances the offset (`driver.rs:368-370`). Nothing is fetched, nothing is
  written, nothing is answered. That is the behaviour this unit wants and nothing proves it.
- **Payment and gift service messages ride a type we do subscribe to, and are skipped for a
  different reason.** `Incoming` (`client.rs:125-144`) decodes `message_id`, `date`, `chat`,
  `from`, `sender_chat`, `text`, `caption`, `reply_to_message` and `pinned_message` — no
  `successful_payment`, no `invoice`, no `gift`. A payment or gift service message therefore
  reaches `text_of` (`translate.rs:466-472`) with neither text nor caption and is skipped at
  `translate.rs:165-166`. Unit 01 rewrites exactly that skip, which is why this unit pins the
  *outcome* and not the variant name.
- **The batch's real-time properties**, all cited in finding 1: sequential processing
  (`driver.rs:318-323`), halt-and-redeliver with a two-second pause (`driver.rs:71`,
  `driver.rs:322-341`), a one-minute ceiling on the chat lookup's stated wait (`client.rs:45`,
  `client.rs:334-338`), **no ceiling at all on the administrator fetch** (`client.rs:469`,
  with the reason at `client.rs:498-504` and the ceiling test at `client.rs:517`), three
  attempts per request (`client.rs:29`), and a 25-second long poll (`client.rs:19`) inside a
  35-second request timeout (`client.rs:22`).
- **The core has no commerce vocabulary and no entry point that could grow one cheaply.**
  `InboundMessage` is a channel, a sender, an authority, an addressed flag, a reply target, a
  command, text, an origin and a timestamp (`message.rs:171-211`); `Observation` carries one of
  three facts (`message.rs:218-249`); `OutboundReply` is a channel, text, a kind and a reply
  target (`message.rs:373-390`; `:392` begins the module's own unit tests). There is no amount,
  no identifier of a thing being sold, and no deadline anywhere in the model.
- **Admission is fail-closed and per channel** (`authorization.rs:60-73`), and the privacy
  command family is five commands (`privacy.rs:95-104`) — the whole command vocabulary the core
  recognises today, before unit 15's catalogue.
- **Tools are admitted per conversation and checked at the call** (`tools/mod.rs:14-24`),
  decision 0040 puts tool behaviour in the core by rule, and the palette's registered set is
  already asserted exactly (`crates/core/tests/spine/tools.rs:653-705`).
- **Erasure nulls columns and removes direct conversations** (`erasure.rs:1-30`), one answer per
  row, with no notion of a row the law forbids erasing.
- **Unit 01 already deferred the receiving half to this unit**, in terms: paid media is not
  carried because "its files sit behind a purchase, and a bot fetching them is a payments
  question, not a media question" (`docs/units/telegram/01-receiving-media.md:266`), and its
  contract names paid media as an explicit skip (`:572`).
- **Unit 12 recorded the platform's own refusal to copy commerce messages**: `copyMessage`
  cannot copy "Service messages, paid media messages, giveaway messages, giveaway winners
  messages, and invoice messages" (`docs/units/telegram/12-forwarding-and-copying.md:55-56`).
- **The outbound refusal already has two mechanisms specified in this same batch, and this unit
  builds neither.** Unit 20 replaces the client's stringly-typed wire with a closed
  enumeration: `post` — today `async fn post(&self, method: &str, body: &serde_json::Value)`
  (`client.rs:532-545`) — "stops taking a `&str` and takes a closed enumeration of the calls the
  assistant makes, one variant per call ... An arbitrary method name stops being one typo away
  and becomes a type error", pinned by a `Method::ALL` exact-list assertion
  (`20-moderation-actions.md:256-282`). Beside it, units 19, 20, 09 and 26 share **one**
  committed file of method names the assistant must never call, scanned over the adapter crate
  with `file:line` reporting and a negative fixture proving the matcher can fail
  (`19-chat-administration.md:469-491`, `19-chat-administration.md:567-570`,
  `20-moderation-actions.md:284-291`, `26-checklists-and-suggested-posts.md:520-524`). Unit 20
  states why both are kept: "each catches what the other cannot".
- **The siblings disagree, in writing, about that shared file's name, its matching rule and the
  directories it reads.** Unit 19 calls it `docs/administrative-methods.txt`
  (`19-chat-administration.md:215-216, 470`) and specifies whole-word matching over runs of
  letters and digits, with the reason that "a substring rule would make `getChatMember` match
  `getChatMemberCount` and would make any future `getChat` entry match the two calls that must
  keep being made" (`:481-484`), scanning "the adapter crate's `src` and `tests`" (`:567`).
  Unit 20 says unit 19 specifies `docs/telegram-refused-methods.txt`
  (`20-moderation-actions.md:284-285`). Unit 26 calls it `docs/telegram-refused-methods.txt`
  and describes "units 19's and 22's single **substring** scan over both crates' `src`
  directories" (`26-checklists-and-suggested-posts.md:520-523`). Three readings, and this unit
  edits none of them. What it does instead is in the decisions: contribute only needles that
  behave identically under every one of those readings.
- **The adapter crate already owns a scanning test target with its own binary**:
  `tests/token_scan.rs` proves the token appears in no log line or error string, and documents
  why the capture subscriber owns a whole process — it is installed with `set_global_default`
  (`token_scan.rs:74-80`), and "a process-wide default can only be owned by a test that shares
  its process with nothing else ... whichever concurrently running test first executes a log
  statement decides its callsite's cached interest, so a shared process makes the capture
  racy" (`token_scan.rs:6-12`).
- **The scripted fixture records the poll itself, and exposes only a per-method accessor.**
  `BotApiServer::recorded(method: &str)` filters one method name (`tests/adapter/server.rs:248-258`)
  and `await_recorded` waits for a count of one method (`:262-272`); there is no accessor for
  every request made. The poll is itself recorded — the existing wire test reads the selection
  through `server.await_recorded("getUpdates", 1)` (`group_context.rs:29`) — and `fetch_identity`
  issues `getMe` before the first batch (`driver.rs:305`, `client.rs:304-307`). So no batch has
  zero recorded requests, and an assertion phrased that way could never pass.
- **The fixture already owns the settle point an absence assertion needs.**
  `await_state_file(path, next_offset)` blocks until the offset file holds exactly that value
  (`tests/adapter/support.rs:843-858`), used nine times in `offset.rs`. Reading the store
  directly goes through `Store::run`, which is generic over the closure's return
  (`agent-ledger/crates/agent-ledger/src/store/mod.rs:455-461`), so a row count is expressible;
  `offset.rs:148-157` is *not* a counting precedent — those lines are the `ALTER TABLE` that
  hides the identity table to force a transient failure.
- **Documentation pins have an established home**: `crates/assistant/tests/docs.rs` reads
  committed files and asserts named substrings, and "Each pin reads the committed file the way
  the repository ships it, so a drifted edit fails loudly here" (`docs.rs:32-34`). Units 20, 21
  and 23 all pin their decision records and contract sections there.
- **No privacy or compliance document mentions money.** The record of processing describes "A
  bot in the halogenOS community groups stores the groups' messages, and answers questions
  addressed to it" (§2) on three legitimate-interest purposes (§3); its review triggers include
  "a change to what is collected" and "a new path that sends message content off the machine"
  (§11). The impact assessment's trigger list (§10) is the same set plus the standing-touching
  clause. The public notice tells members "We take nothing about you from anywhere else"
  (`docs/privacy/bot-assistant-privacy-policy.md:50`).

## Decisions taken with this unit

- **No commerce capability ships: the assistant sends no invoice, answers no payment query,
  holds no Stars balance it manages, sells no media and sends no gift, 2026-08-27.** Three
  independent reasons, each sufficient. First the real-time contract: ten seconds is not a
  budget this inbound path can promise, and a payment that fails because a member's message was
  ahead of it in the batch is a defect visible to the person who tried to pay. Second the legal
  change: a sale needs a contract basis, a retention regime that Article 17(3)(b) and §147(3) AO
  put outside the erasure promise this project publishes, a terms route, a support route and a
  payment-support route the platform requires by name — none of which exists, and the first of
  them contradicts a published document on the day it merges, which is a defect and not a
  follow-up. Third the effect: gift and Star methods move value irreversibly from the bot's
  balance, at the word of a model reading text members write. *Rejected:* a Stars-only donation
  button, the smallest shape anyone will propose — it is still a payment in the platform's
  sense, carrying the same dispute, `/paysupport`, retention and refund duties, and the
  proceeds land in a balance the operator withdraws through a payout account; the smallness is
  in the code, not in what it makes the operator. *Rejected:* implementing only the refusing
  half, answering every pre-checkout query with `ok: false` so that a stray invoice cannot
  complete — it requires subscribing to the update, entering the ten-second contract we have
  just said we cannot meet, and decoding a person's order and address in order to say no,
  where doing nothing at all produces the same cancellation. *Rejected:* leaving the question
  open as a follow-up — follow-ups record accepted shortfalls in shipped work; this is a
  decision, and leaving it unrecorded means the next unit re-derives it or ships it.

- **The four commerce update types are never subscribed, and the refusal is asserted on the
  wire, 2026-08-27.** `CONSUMED_UPDATE_TYPES` gains nothing; a new adapter test asserts that
  the `allowed_updates` array sent on the wire contains none of `"shipping_query"`,
  `"pre_checkout_query"`, `"purchased_paid_media"` or `"subscription"`, with the reason in the
  assertion's own message. It is written as a containment check, like unit 08's, so it survives
  whatever sibling units add to the list. *Rejected:* relying on the fact that we name the list
  at all — the platform's default set contains all four, so a future change that drops the
  explicit selection would subscribe to them silently; the assertion is what makes that change
  fail a check. *Rejected:* an exact-list assertion — it would collide with units 05, 07 and 09,
  which unit 08 has already documented as colliding with each other.

- **The arriving-anyway case is pinned as a skip for all four types, and no decode path is added,
  2026-08-27.** The documented transition window means a token that had a wider selection can
  deliver a commerce update into this adapter's first polls. Today each decodes with every known
  field absent and is skipped anonymously; this unit pins it: a scripted update carrying an
  `update_id` and one such payload object is acknowledged, advances the offset, and provokes no
  request beyond the poll itself and no stored row. *Rejected:* giving them named `Skip` variants
  with a reason line, the way `edited_message` earns one (`client.rs:117-118`) — that field exists
  because edits arrive constantly on a subscribed type, whereas these decode an order, a price
  and possibly a postal address on a path that runs only in a misconfiguration; unit 06's
  reasoning against paths that never execute applies unchanged. *Rejected:* logging what
  arrived — a log line naming a pre-checkout query's payload or a shipping query's address puts
  personal data of a new category into a file, to describe an event we are declining to handle.

- **The absence of a request is asserted positively, as the exact set of method names the batch
  recorded, 2026-08-27.** The obvious phrasing — "acknowledged with no request of any kind" —
  is false against this fixture, which records the poll and the identity fetch before any
  update is processed (`group_context.rs:29`, `driver.rs:305`), and is not even expressible,
  because `BotApiServer` exposes only `recorded(method)` (`server.rs:248-258`). The next
  obvious phrasing — enumerating the methods that must be absent — is worse, because two of
  the names that would have to be enumerated are on the shared refusal list, so the test
  asserting the refusal would itself trip the scan enforcing it under unit 19's reading of the
  scanned tree (`19-chat-administration.md:567`). Both problems have one answer: the fixture
  gains a `recorded_methods()` accessor returning the distinct method names recorded so far,
  and each pin asserts that set **equals** `{"getMe", "getUpdates"}`. It names no refused
  literal, it needs no maintenance when a sibling unit adds a call the assistant does make —
  such a call arriving on a batch that should have made none is exactly the failure worth
  seeing — and it says something strictly stronger than any negative enumeration, because it
  also fails on a method nobody has thought of. *Rejected:* enumerating absent method names,
  the shape unit 08's AC4 uses (`08-inline-queries.md:575-576`). Unit 08 could do it safely
  only by leaving its own scan's literals out of the enumeration, which it did deliberately
  (`08-inline-queries.md:369-372`); an enumeration that has to be pruned to avoid the check
  guarding the same property is a weaker assertion bought with a footnote. *Rejected:* an
  exception in the scan for the test file — this unit's whole argument is that the scan is
  worth having, and a scan with exceptions is a scan whose next exception is easier.

- **Payment and gift service messages stay unrecorded, pinned by outcome and not by variant
  name, and the pin set is five of the nine, 2026-08-27.** `successful_payment`,
  `refunded_payment`, `gift`, `unique_gift` and `gift_upgrade_sent` ride the `message` type this
  adapter consumes, so unlike the four query types they are not kept out by the selection; they
  are kept out by carrying neither text nor caption (`translate.rs:165-166`). The pin asserts
  the outcome — no block, no conversation, no identity row, no request beyond the poll — and
  deliberately does not name the `Skip` variant, because unit 01 renames that variant and
  rewrites its condition. The remaining four of the nine are named in the grounding and pinned
  elsewhere or by somebody else: `invoice` is a message the assistant would have to have sent;
  the three `suggested_post_*` notices belong to unit 26, which grounds all five suggested-post
  service messages and pins that they record nothing
  (`26-checklists-and-suggested-posts.md:161-163, 401, 456`), and duplicating its pins here
  would be the same property asserted twice in two units' names. *Rejected:* a named skip per
  commerce service message — the same dead-path argument as above, and a named variant invites a
  later unit to "just record that a payment happened", which is the row erasure could not reach.
  *Rejected:* leaving it unpinned on the grounds that unit 01 owns the decode — unit 01 widens
  what is recorded, and a widening with no pin behind it is exactly how a receipt ends up on the
  ledger unnoticed. *Rejected:* pinning all nine here, so that one unit owns "commerce on the
  message type" — three of them are already unit 26's by its own contract, and two units pinning
  one behaviour means the second one to change it silently loosens the first.

- **The outbound refusal contributes names to the one shared list and builds no mechanism of its
  own, 2026-08-27.** An earlier draft of this unit specified its own scanner over the adapter
  crate and called a source scan "the only real check there is". Both halves were wrong. Unit 20
  makes an unlisted method call a compile error rather than a grep hit
  (`20-moderation-actions.md:256-282`), which is the stronger check and the one this unit relies
  on; and units 19, 20, 09 and 26 already share one committed list file with a stated merge rule,
  so a second list here would be the "one decision recorded twice" that unit 19's and unit 20's
  own rejected alternatives both condemn. This unit therefore contributes twenty-two names to
  that file, each with a comment naming this unit as the refusing one: `sendInvoice`,
  `createInvoiceLink`, `answerShippingQuery`, `answerPreCheckoutQuery`, `getMyStarBalance`,
  `getStarTransactions`, `refundStarPayment`, `editUserStarSubscription`, `sendPaidMedia`,
  `getAvailableGifts`, `sendGift`, `giftPremiumSubscription`, `getUserGifts`, `getChatGifts`,
  `getBusinessAccountGifts`, `getBusinessAccountStarBalance`, `transferBusinessAccountStars`,
  `convertGiftToStars`, `upgradeGift`, `transferGift`, `setBusinessAccountGiftSettings` and
  `InputInvoiceMessageContent`. `createChatSubscriptionInviteLink` and
  `editChatSubscriptionInviteLink` are unit 19's contribution and are not claimed twice;
  `approveSuggestedPost` and `declineSuggestedPost` are unit 26's. *Rejected:* keeping this
  unit's own scanner because commerce is a different class of refusal from moderation — it is
  not a different class, it is the same property (a method name the assistant must never emit)
  with a different reason, and the reason belongs in the file's comment, not in a second
  file-collection routine that drifts from the first. *Rejected:* relying on unit 20's
  enumeration alone and contributing no names — the enumeration prevents the call and says
  nothing about a helper, a comment, a serde struct or a half-written branch that names the
  method on the way to making it; unit 20 keeps both for that reason and this unit does not
  reopen it.

- **Every needle this unit contributes is chosen to behave identically under all three readings
  the siblings have written down, 2026-08-27.** The batch disagrees in writing about whether the
  shared scan matches whole words or substrings, and about whether it reads `src` alone or `src`
  and `tests` (receipts in the grounding). This unit does not arbitrate that; it makes itself
  independent of the outcome. Every one of the twenty-two names is a single run of letters and
  digits, so whole-word matching and substring matching find it in exactly the same places; none
  is a substring of any call the adapter must keep making — `getMe`, `getUpdates`, `getChat`,
  `leaveChat`, `sendMessage`, `sendChatAction`, `getChatAdministrators` (`19-chat-administration.md:210-212`)
  — so the substring reading raises no false positive against this unit's lines; and no test
  this unit writes contains any of the twenty-two, so the wider `src`-and-`tests` reading fails
  on nothing in this unit's diff. Case is not decided either: matching case-insensitively, as
  `crates/core/tests/vocabulary.rs` does by lowercasing both the list and the content
  (`vocabulary.rs:23`, `:76`), finds every one of these names, because a lowercased camelCase
  name is still one alphanumeric run. *(An earlier draft claimed the opposite — that a scanner
  copied from the vocabulary test would compare `sendInvoice` against a lowercased line and
  never match. That was a misreading of the two lines it cited: the list is lowercased too, so
  the comparison succeeds. The claim is withdrawn, and with it the case-sensitivity requirement
  it was the only argument for.)* *Rejected:* specifying the matching rule here so the tree has
  one answer. Three sibling specifications state a rule, two of them differently; a fourth
  statement makes the disagreement worse, and a unit whose own diff depends on which sibling's
  prose wins is a unit that breaks on merge order. The disagreement is named in the notes for
  whoever merges last to settle in one place.

- **Two commerce affordances are parameter-shaped and cannot go on a method list; they are
  stated as residuals instead of being covered badly, 2026-08-27.** `allow_paid_broadcast` (0.1
  Stars per message) and the serialized Pay button key are not method names. `allow_paid_broadcast`
  cannot be a needle under whole-word matching at all — `carries_word` splits on `_`
  (`vocabulary.rs:64-67`), so the entry could never fire — and its three runs are each ordinary
  words this workspace already uses (`broadcast` names a tokio channel kind). A bare `pay`
  needle matches ordinary English, and a quoted `"pay"` needle is inexpressible under whole-word
  matching and would be the only entry on a list of method names carrying punctuation. What
  covers them instead, stated rather than assumed: the Pay button "can only be used in invoice
  messages", and both methods that send one — `sendInvoice` and `createInvoiceLink` — are on the
  list and, after unit 20, are not callable at all; `allow_paid_broadcast` is a parameter on
  those same methods and on the sends unit 20's enumeration closes. The residual is that a
  parameter added to a permitted send is caught by diff review and by nothing mechanical, and
  it is written into the test comment and AC10 rather than papered over. *Rejected:* a needle
  that fires under one candidate matcher and silently never fires under the other — a check that
  passes for the wrong reason is worse than an acknowledged gap, because the gap is read and the
  false pass is not. *Rejected:* a second scanner with substring matching just for these two —
  the smearing this project refactors away from, for two entries.

- **The scan stays scoped to the adapter crate, and this unit says why, 2026-08-27.** The adapter
  is the only crate that holds a Bot API client, a base URL and the token
  (`client.rs:532-545`); `crates/assistant` reads the token from configuration and hands it to
  the adapter's constructor; `crates/core` never receives it and by decision 0013 speaks no
  platform API at all. A method name appearing in the core is caught by the platform-vocabulary
  scan's own rules or is inert. *Rejected:* widening the scan to the whole workspace. It would
  reach `crates/assistant/tests/docs.rs`, where this unit's own documentation pins live, and the
  pins would have to be written to avoid the very names the decision record exists to refuse —
  the same trap the request-set assertion above avoids, re-entered for no gain.

- **Value transfer is an effect, and the assistant may not cause one, 2026-08-27.** Decision
  0070 settles that the assistant assesses and a human decides, for moderation effects. This
  unit records the extension: no tool that moves money, Stars, gifts or a subscription is ever
  registered in the tool palette, and no future one ships without a mechanism where a human
  approves the concrete transfer before it takes effect — the same structure 0070 requires for
  an administrative action. The reason is stronger here than for moderation: a report can be
  judged and dropped by an administrator, a sent gift cannot be recalled by anybody, and
  `refundStarPayment` reverses only incoming payments. The refusal is not prose alone: the
  palette's registered set is already asserted exactly
  (`crates/core/tests/spine/tools.rs:653-705`), so a spending tool fails a test by being
  registered, and this unit cites that check rather than adding one — which also keeps the diff
  off `crates/core/`. *Rejected:* a spending tool behind an administrator's confirmation, which
  would satisfy the letter of 0070 — there is nothing to buy, so the mechanism would exist
  before its purpose, and a confirmation dialogue is exactly the affordance a mistaken model
  uses to make a member into an approver. *Rejected:* relying on the prompt to tell the model it
  has no money — a prompt is advice to a model, not a bound on the system, which 0070 already
  says in its own rejected list.

- **Nothing about a person is received or recorded, so no privacy or compliance document changes
  with this unit, 2026-08-27.** No new category of data, no new recipient, no new storage, no
  new path off the machine: none of the record's review triggers fires. Writing an amendment
  saying the project considered taking money and did not would put a non-event into a register
  of processing activities. What the unit does instead is the reversal list below, which names
  every document and clause a reopening would have to change first. *Rejected:* a note in the
  record for completeness — a record of processing describes processing, and padding it with
  refusals makes the real entries harder to audit.

- **Nothing streams, because nothing here moves a byte, 2026-08-27.** The unit adds one wire
  assertion, four scripted-update pins, five service-message pins, a log-capture pin, twenty-two
  lines on a committed list and two documents. Recorded because the streaming constraint binds
  every spec. If paid media were ever reopened, the sending side would ride unit 02's shape
  unchanged — `InputPaidMedia` takes a `file_id`, an HTTP URL or a multipart `attach://` upload,
  so the bytes would move chunk by chunk exactly as unit 02 specifies and would never be
  assembled in memory — and the receiving side stays refused by unit 01, whose reason (files
  behind a purchase) this unit confirms instead of overturning.

- **The two documents this unit ships are pinned, 2026-08-27.** The decision record and the
  operator-contract section are the unit's only durable artefacts; without a pin, a later edit
  that guts either passes every check. They go in `crates/assistant/tests/docs.rs`, the
  established home, where "Each pin reads the committed file the way the repository ships it, so
  a drifted edit fails loudly here" (`docs.rs:32-34`), matching units 20, 21 and 23. The pinned
  substrings are prose — the rule about value transfer, the statement that no provider may be
  connected, the statement that Stars need no connection — and carry no refused method name, so
  the pins stay clear of the scan under every reading of its scope. *Rejected:* leaving them
  unpinned because AC1 fences the diff off `crates/core/` — `crates/assistant/tests/docs.rs` is
  not in the core and not under `docs/privacy/`, so the omission was never forced. *Rejected:*
  pinning the list of refused method names in the decision record — that is the shared list
  file's job, the scan already reads it, and a second copy in prose is a second thing to keep
  in step.

- **The operator contract states the refusal in the operator's own terms, 2026-08-27.** A new
  section says that the assistant sells nothing, that no payment provider may be connected in
  BotFather for this token, that Stars need no such connection so the code is the only place
  the refusal lives, and that a request for donations or paid content is a product decision
  with the checklist below attached, not a configuration change. *Rejected:* leaving it
  undocumented because there is nothing for the operator to do — the one action that would
  silently break this unit's contract (connecting a provider in BotFather, or asking a
  contributor for "just a tip button") is the operator's, so it belongs in the operator's
  document.

## What this unit examined and deliberately leaves alone

**Paid-message groups.** A supergroup can charge members Stars per message: `Message` carries
`paid_star_count` ("The number of Telegram Stars that were paid by the sender of the message to
send it") and the service message `paid_message_price_changed` announces a change. Both ride
the `message` type this adapter consumes. The adapter decodes neither field, so a member's
message in such a group records exactly as it does anywhere else and nothing about their
spending is stored. That is the right outcome and this unit changes nothing, but it is named
here so that the next unit widening the message decode sees it already examined: how much a
member paid to speak is a fact about their finances, and recording it would be a new data
category in a record that has none.

**The three suggested-post money notices.** `suggested_post_paid`, `suggested_post_refunded`
and `suggested_post_approval_failed` are commerce facts on the `message` type and are named in
the grounding for that reason, but they occur only in a direct messages chat and unit 26 owns
all five suggested-post service messages, the two methods behind them and the pins that they
record nothing (`26-checklists-and-suggested-posts.md:161-163, 345-354, 401, 456`). Unit 26
cites this unit's money argument as one of its three reasons for refusing
`approveSuggestedPost`; the reference goes both ways and neither unit re-specifies the other.

**Whether a bot can receive a gift at all.** The documentation does not state that a bot which
is not connected to a Telegram Business account can be a gift recipient. What it does state
points the other way: `GiftInfo.owned_gift_id` is "only present for gifts received on behalf of
business accounts", and every method that manages a received gift requires a
`business_connection_id` and a business bot right. Marked as an unproven inference in the shape
unit 06's AC13 and unit 08's AC16 use, and nothing merged depends on it: the service-message pin
covers the case whether or not it can occur, and the adapter subscribes to no business update
type, so no connection can be established.

**`getUserGifts` and `getChatGifts`.** These are the only commerce methods that would tell the
assistant something new about a member without any money moving, and `getUserGifts` needs no
permission the documentation names — a numeric user id is the whole input. They are refused with
the rest and go on the shared list, because a lookup enumerating what a member owns is precisely
the enrichment the public notice denies: "We take nothing about you from anywhere else."

**Unit 07's Pay button.** `InlineKeyboardButton.pay` is a door from the offered-choices
keyboard straight into commerce, and it "can only be used in invoice messages" — so it is
harmless without an invoice, and both methods that send an invoice are refused twice over, by
unit 20's enumeration and by this unit's lines on the shared list. The residual, stated in AC10
and in the decision above, is that the button's own key is a parameter and not a method name,
so nothing mechanical catches it being added to a permitted send; the mitigation is that unit
07's keyboard is a closed shape in the adapter with its own tests, and the named later check is
that whichever unit introduces a typed keyboard struct asserts its serialized key set.

## What would have to be true before this is reopened

Refusing without naming what could work is refusing without examining. The narrowest shape
anybody will propose is a donation in Stars — one invoice, no shipping, no goods, one
`answerPreCheckoutQuery` that always says yes. Even that shape is blocked today, and the
checklist is the useful part:

1. **Somebody must be the merchant, on paper.** The platform assigns dispute handling, refunds
   and customer support to the bot's owner in terms, and requires a `/terms`, a `/support` and a
   `/paysupport` route. Those three commands would enter unit 15's `Command::ALL` catalogue and
   be published to the platform menu, which makes this checkable: if the catalogue has no
   payment-support command, the project is not selling anything.
2. **The ten-second contract needs a route of its own, and the core needs the vocabulary for
   it.** A deterministic answer, decided from facts stored before the query arrived, on a path
   that cannot queue behind a member's message, behind the chat lookup's minute-per-attempt
   ceiling, or behind the administrator fetch's unbounded wait. The neutral vocabulary is the
   easy half — a *purchase authorisation request* carrying an amount, a reference and a
   deadline, answered by the core with a yes or a no. The hard half is that the core would gain
   its first inbound kind with a real-time deadline, and every existing protection (admission,
   authority, budgets, suppression) would need an answer for it. If that route is invented for
   one caller, it is the bolted-on shape this project refactors away from.
3. **The ledger needs a retention hold and erasure needs a second answer.** A payment record
   cannot be nulled on request: Article 17(3)(b) GDPR withholds the right where a legal
   obligation applies, and §147(3) of the German Fiscal Code keeps accounting vouchers eight
   years. Erasure has one answer per row today (`erasure.rs:1-30`). Either the payment facts
   live outside the ledger entirely, in a store with its own retention rule, or the ledger
   learns what a held row is. The first is likely right and is a design in its own unit.
4. **Four documents change before the code merges, not after.** The record of processing gains a
   purpose with Article 6(1)(b) and its records with 6(1)(c), a category of data for order
   information including a postal address, a recipient for the payment collector, and a
   retention entry that is not "until somebody asks". The impact assessment gains an addendum:
   financial data, a new risk of loss, and the balance that can go negative. The
   legitimate-interests assessment is *not* amended — a sale does not run on legitimate
   interest — and its §4.1 must say so, since it currently describes the whole processing. And
   the public notice, the one document a member actually reads, changes in two places: the
   deletion promise ("ask, and it goes") acquires a second exception, and the recipients table
   acquires whoever holds the money. The public notice is named first, because it is the
   promise made to people, not to an auditor.
5. **A human decides before any value moves.** Per this unit's own decision, no spend without a
   mechanism in which a person approves the concrete transfer first, plus a hard cap on what can
   leave the balance in a period, so that a mistake is bounded even when the approval is
   automated later by somebody in a hurry.
6. **There has to be a product.** Nothing in the community's actual need requires any of this:
   telling a member where the project's donation page is, is text, and the wiki lookup already
   answers it today with no commerce code at all. A reopening that cannot name what is being
   sold is a mechanism looking for a purpose.

Nothing above is a decision deferred into legitimacy: the answer is no, today, for the reasons
in the previous section. The list exists so a future yes pays its price in the open.

## The unit's contract

After this unit the repository's answer to "can the assistant take, hold or spend money" is a
recorded no with its reasoning, and the no is checkable on both sides instead of assumed.
Inbound: the poll's `allowed_updates` is asserted to contain none of `shipping_query`,
`pre_checkout_query`, `purchased_paid_media` or `subscription`, with the reason written into the
assertion, and each of those four delivered inside the platform's documented transition window
is proven to be acknowledged, to advance the offset, to leave the store unchanged and to make
the batch record no method beyond the poll and the identity fetch; the five commerce service
messages that ride the subscribed message type — a successful payment, a refunded payment, a
regular gift, a unique gift and a gift upgrade — are proven to record nothing and to request
nothing, pinned by outcome so unit 01's rewrite of that skip cannot quietly change it, and no
captured log line carries an order, an address, a payload or a charge identifier from any of
them. Outbound: twenty-two commerce method names join the one committed refusal list units 19,
20, 09 and 26 share, each with a comment naming this unit, and the scan already specified there
finds none of them in the adapter crate — so the assistant is proven to emit no invoice, no
payment answer, no Star query, no refund, no Star-subscription edit, no paid media, no gift, no
gift enumeration and no business-account gift call. The two Stars-charging subscription invite
link methods are unit 19's contribution to the same list and the two suggested-post methods are
unit 26's, so the platform's Stars surface is closed across the three units without any name
appearing twice. This unit builds no scanner, adds no list file and does not restate unit 20's
closed `Method` enumeration, which is the stronger of the two outbound checks and the one that
makes an unlisted call a compile error. Two affordances stay uncovered by any mechanical check
and are named as such: the `allow_paid_broadcast` parameter and a Pay button key serialized
through a derived struct, both parameter-shaped, both on methods that are themselves refused.
The core is untouched: no new entry point, no new kind, no new table, no new tool, no vocabulary
for amounts, orders or deadlines, and `docs/platform-vocabulary.txt` is unchanged because the
core learned no platform word; the existing palette assertion
(`crates/core/tests/spine/tools.rs:653-705`) is cited as the check that a spending tool cannot be
registered quietly. Two documents are added and both are pinned in
`crates/assistant/tests/docs.rs`: a decision recording the refusal and the extension of decision
0070 to value transfer, and a section of the operator contract stating that no payment provider
is connected for this token, that Stars need no connection so the source is the only place the
refusal lives, and that donations or paid content are a product decision with a named checklist.
No privacy or compliance document changes, because nothing new is received, stored or sent
anywhere. Nothing streams, because nothing here carries a byte. No new dependency, no new
configuration entry, and no change to any behaviour a member can observe.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and
  `AnsweringMode::Addressed` (`assembly.rs:180-188`); clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan, the token scan and the shared refused-methods scan clean; no new
  dependency and no new configuration entry; the diff touches no file under `crates/core/` and
  no file under `docs/privacy/` or `docs/compliance/`.

- **AC2** The poll subscribes to no commerce update type: an adapter test asserts that the
  `allowed_updates` array sent on the wire contains none of `"shipping_query"`,
  `"pre_checkout_query"`, `"purchased_paid_media"` or `"subscription"`, with the assertion's
  message naming this unit's decision and stating that the platform's default set includes all
  four. Written as a containment check and placed beside
  `the_poll_names_the_update_types_it_consumes`
  (`crates/adapters/telegram/tests/adapter/group_context.rs:22-35`) without editing it.

- **AC3** The fixture can answer "what did this batch call": `BotApiServer` gains
  `recorded_methods()`, returning the distinct method names recorded so far in a deterministic
  order, built from the same `requests` vector `recorded` reads (`server.rs:75`, `:248-258`).
  It is the only production-adjacent change in the unit and it exists because the criteria below
  assert an exact request set, which the per-method accessor cannot express.

- **AC4** A pre-checkout query delivered anyway is acknowledged and ignored: with the scripted
  server pushing an update carrying an `update_id` and a `pre_checkout_query` object — an id, a
  sender, a currency, a total amount and an invoice payload — and nothing else, the adapter
  acknowledges it, `await_state_file` (`support.rs:843-858`) shows the next offset past it, and
  `recorded_methods()` equals exactly `["getMe", "getUpdates"]`. The test's own comment records
  why silence is the correct answer: an unanswered pre-checkout query cancels the transaction,
  so the member is not charged. The comment states the ten-second deadline in words and names no
  method, so it stays clear of the shared scan under every reading of its scope.

- **AC5** The same holds for the other three, each pinned separately so no one type stands in for
  the others: an update carrying an `update_id` and a `shipping_query` object with a shipping
  address, one carrying a `purchased_paid_media` object, and one carrying a `subscription`
  object. Each is acknowledged, each settles through `await_state_file`, and after each
  `recorded_methods()` equals exactly `["getMe", "getUpdates"]`.

- **AC6** Nothing from AC4 or AC5 reaches the store: after the four updates have settled — the
  settle point being `await_state_file` holding the offset past the last of them, so the
  assertion cannot pass by running before the adapter did anything — the store holds no new
  block, no new conversation, no new channel mapping and no new principal row for the sender
  named in them. Counted through `fixture.store.run`, which is generic over the closure's return
  (`agent-ledger/crates/agent-ledger/src/store/mod.rs:455-461`), reading counts before and after.

- **AC7** The five commerce service messages record nothing: a scripted `message` update whose
  message carries a chat, a sender, a date and a `successful_payment` object — with an
  `order_info` naming a person and a shipping address — and no text and no caption is
  acknowledged, creates no block, no conversation and no principal row, and leaves
  `recorded_methods()` equal to exactly `["getMe", "getUpdates"]`. The same is pinned for
  `refunded_payment`, for `gift`, for `unique_gift` and for `gift_upgrade_sent`, each settling
  through `await_state_file`. The assertions name the outcome and not the `Skip` variant, so unit
  01's rewrite of that condition does not invalidate them; each carries a comment saying so.

- **AC8** No log line carries the commerce payloads: in a test target of its own, with its own
  process-wide capture subscriber for the reason `token_scan.rs:6-12` documents, a pre-checkout
  query with an invoice payload and a charge-shaped identifier, a shipping query with a postal
  address, and a successful-payment service message with an `order_info` are re-scripted and
  driven through the adapter, and no captured line contains any of those scripted values. The
  duplication of three fixtures from AC4, AC5 and AC7 is deliberate and its reason is in the
  target's module comment: the subscriber is installed with `set_global_default`
  (`token_scan.rs:74-80`), so a capture assertion cannot read lines produced in the shared
  adapter suite's process.

- **AC9** The twenty-two names this unit refuses are on the single committed refusal list units
  19, 20, 09 and 26 share, each with a comment naming this unit and its reason, and the scan
  already specified over the adapter crate finds none of them. No second list file and no second
  scanner is created; the negative fixture proving the matcher can fail is unit 19's AC3
  (`19-chat-administration.md:567-570`), which pins it in both directions and is re-run
  unchanged with the longer list. `createChatSubscriptionInviteLink` and
  `editChatSubscriptionInviteLink` appear exactly once each, under unit 19's name;
  `approveSuggestedPost` and `declineSuggestedPost` exactly once each, under unit 26's. If none
  of those units has merged when this one does, this unit creates the file and the scan in the
  shape unit 19 specifies, and the later units add to it.

- **AC10** The two uncovered affordances are stated, not hidden: the list file's comment for this
  unit's block records that `allow_paid_broadcast` and the serialized Pay button key are
  parameters rather than method names, that neither can be a needle on a method list without
  either never firing or firing on ordinary prose, and that what covers them is the refusal of
  every method that carries them plus unit 20's closed enumeration. The same two sentences appear
  in the decision record of AC11.

- **AC11** The decision is recorded: a file in `docs/decisions/` carries this unit's refusal with
  its date and its rejected alternatives, including the extension of decision 0070 to value
  transfer, stated as the rule that no tool moving money, Stars, gifts or a subscription is
  registered and none ships without a human approving the concrete transfer beforehand. The
  number is taken at merge time, continuing the numbering after whatever is unclaimed then.

- **AC12** The operator is told: `docs/reference/group-operator-contract.md` gains a payments
  section stating that the assistant sells nothing and asks for nothing, that no payment provider
  may be connected for this token in BotFather, that payments in Telegram Stars require no such
  connection — so the refusal lives in the source and nowhere else — and that donations or paid
  content are a product decision, pointing at the reopening checklist in this document.

- **AC13** Both documents are pinned in `crates/assistant/tests/docs.rs`, in the established
  shape: the decision record's rule about value transfer, its date and the presence of its
  rejected alternatives; and the operator contract's three statements. The pinned substrings are
  prose and carry no refused method name, so the pins are safe under any scope the shared scan
  ends up with.

- **AC14** The reopening checklist in this document names the record of processing, the impact
  assessment, the legitimate-interests assessment and the public privacy notice explicitly, with
  the clauses each one would have to change — so a future unit that reopens the question cannot
  claim the documents were never considered. (The diff's emptiness over those documents is
  asserted by AC1's fence; it is not restated here.)

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The diff is small on purpose and touches no production code path: the deliverables
  are the tests, twenty-two lines on a committed list, the two documents and this specification.
- Adapter test sites, all under `crates/adapters/telegram/tests/`:
  - AC2 beside `the_poll_names_the_update_types_it_consumes` in `adapter/group_context.rs`, as a
    new test in the same file, without editing the existing exact-array assertion.
  - AC3 in `adapter/server.rs`, beside `recorded` (`server.rs:248-258`), reading the same
    `requests` vector (`server.rs:75`).
  - AC4, AC5, AC6 and AC7 in `adapter/offset.rs`, which already exercises the acknowledgement and
    offset contract through the scripted server, already uses `await_state_file` nine times and
    already reads the store directly. The scripted server selects updates by `update_id`
    (`adapter/server.rs:504-525`), so each scripted update carries an `update_id` plus its single
    payload object and nothing else. Note when writing them that `offset.rs:148-157` is the
    `ALTER TABLE` that hides the identity table, not a row-counting precedent — the counting
    shape comes from `Store::run` directly.
  - AC8 as its own test target beside `tests/token_scan.rs`, with its own capture subscriber and
    its own re-scripted fixtures. Both new targets are auto-discovered: the adapter manifest
    declares no `[[test]]` section, which is why `tests/adapter/main.rs` and `tests/token_scan.rs`
    need no entry today.
- Documentation sites: one decision file, number taken at merge; a section in
  `docs/reference/group-operator-contract.md` per AC12; pins in `crates/assistant/tests/docs.rs`
  per AC13. No entry in `docs/follow-ups.md`, per the decision — this is a decision, not an
  accepted shortfall.
- **The shared refusal list is specified three different ways across this batch, and this unit
  deliberately does not add a fourth.** Unit 19 names the file `docs/administrative-methods.txt`
  (`19-chat-administration.md:215-216, 470`), specifies whole-word matching over runs of letters
  and digits with a stated reason (`:481-484`), and scans `src` and `tests` (`:567`). Unit 20
  says unit 19 specifies `docs/telegram-refused-methods.txt` (`20-moderation-actions.md:284-285`).
  Unit 26 names `docs/telegram-refused-methods.txt` and describes a **substring** scan over both
  crates' `src` directories (`26-checklists-and-suggested-posts.md:520-523`). Whoever merges last
  settles the three in one place; this unit's twenty-two names match identically under every one
  of them, and none of its tests contains any of them, so it merges cleanly whichever way it goes
  and whatever the order. Two things the settler should know, offered as observations and not as
  edits to anybody's specification. First, whole-word matching over alphanumeric runs cannot
  express a needle spanning a separator: unit 08's `switch_inline_query`
  (`08-inline-queries.md:565, 575-576`) and this unit's `allow_paid_broadcast` both split into
  ordinary words and would never fire, so if those needles are wanted, the matcher needs to be a
  substring match bounded by non-alphanumeric characters on both sides — which keeps unit 19's
  stated property exactly (`getChatMember` still does not match `getChatMemberCount`, and its
  AC3 passes unchanged) while admitting multi-run needles. Second, unit 08 specifies a second
  list file beside its own test (`08-inline-queries.md:679-685`) for the same class of property;
  folding it into the shared file is the same "one decision recorded twice" argument units 19 and
  20 each make in their own rejected alternatives.
- Sibling collisions on the update selection, stated and not acted on. The existing wire test
  asserts the exact three-element array; units 05, 07 and 09 each add to or assert that list, and
  unit 08 has already documented that any two of them merging together breaks one. This unit's
  assertion is a containment check and collides with none. The exact-list wordings are for those
  units to relax, not this one.
- Two things to watch after merge. First, unit 01 rewrites the skip that keeps payment and gift
  service messages off the ledger; its implementer should read AC7 before widening what a
  file-bearing message records, and the pins are written by outcome so they will fail loudly if
  the widening reaches too far. Second, if the operator ever connects a payment provider in
  BotFather for an unrelated reason, nothing in this repository can see it — there is no
  capability flag on `getMe` for payments, unlike the inline flag unit 08 reads — which is why
  the operator contract carries the statement instead of the code.
