# 0197 — A quiet conversation expires after ninety days

Date: 2026-09-02, with unit 53. Refines decision 0003, which is not reversed and not
rewritten.

## Context

Decision 0003 kept message history with no scheduled expiry, and gave the reason: a window
over messages destroys the long memory the assistant exists for, and it does not remove the
need for erasure on request. That reasoning was about a schedule deleting individual
messages, and it still holds against one.

What it left behind is a store with no end date at all. Storage limitation under Article
5(1)(e) was answered with "until somebody asks", and everything a deletion request does not
reach — the assistant's own answers quoting a person, a stored lookup query, a rules text
naming somebody, the copy of one message identifier on a stranger's reply — was kept
forever with it. Replaced conversations accumulated on top: a compaction leaves its source
on record, and nothing ever removed one.

The decision on the rule is the operator's, verbatim: "Let's just say any, because the bot
still works even in fresh sessions. But the freshness is determined by the latest ledger
entry." And on activation: "It hasn't been 90 days yet so we are still in the clear any
way. Let's just implement it and activate it. Nothing should be deleted on the first boot
with the policy actice."

## Decision

**A conversation whose newest ledger entry is older than the span is deleted whole.** The
conversation, the blocks nothing else still holds, and the identity rows nothing anywhere
names any more. The span is ninety days by default, stated in the configuration file so a
deployment can be told apart from the code, and a span of zero switches the mechanism off.

**Freshness is the newest entry, and it belongs to the whole conversation.** One message
today keeps a year of that conversation's history; nothing inside one expires on its own.
Every conversation is measured by the same rule — serving, replaced, ancestor, direct — so
a compacted source, which stops growing at its cut, goes a span after that cut while the
thread standing on it is refreshed by every message. A conversation holding no entries at
all is never named: emptiness is a creation state, and expiring it would delete a
conversation mid-birth.

**The age question is asked of the store, in its own clock.** The application reads no wall
clock for it. The same database that stamped every entry answers whether one is old, so
there is no second clock to disagree with the first and no clock library enters the tree.

**Deletion on request never waits for the schedule.** The sweep and a request take the same
arbiter, so whichever holds it runs whole and the other follows; a request takes it on
demand, and the sweep's next pass simply finds less to do. A failure inside one
conversation's deletion fails that conversation alone, and the next pass retries it,
because the conversation is still expired.

**First activation is protected by the rule and by nothing else.** A boot is an ordinary
pass with no catch-up behaviour, and at this activation no stored conversation is near the
span. Stated plainly: switching this on over a store that already held conversations past
the span would delete them on the first pass, because the rule is the whole mechanism.

**The file-storage wording is held back.** The working draft of the privacy policy carried
paragraphs promising that pictures, documents and voice messages are stored, transcribed
and kept with their conversation. Those paragraphs are not published with this unit: the
media intake is a written specification with no implementation, so the promise would
describe software that does not exist, and a policy may describe only what the code does.
The wording returns with the unit that builds the intake. The draft's retention paragraph
is superseded by this decision, which is wider: it promised an expiry for replaced
conversations only.

## Rejected alternatives

- **Replaced conversations only.** The first draft's shape, and what the parked policy
  paragraph promised. Rejected by the operator: any conversation, because the assistant
  works in fresh sessions and freshness is the latest ledger entry.
- **A per-message expiry.** Rejected in decision 0003 for a reason that still holds — a
  schedule deleting individual messages guts a conversation the group is still using. This
  unit expires whole quiet conversations, which is the shape that objection does not reach.
- **A first-boot grace period.** Rejected: the rule already protects the first boot, and a
  mechanism whose only job is to protect a moment the rule protects is a second answer to
  one question, free to disagree with the first.
- **Sweeping inside the compaction driver's loop.** Rejected: that loop ticks in seconds of
  monotonic time serving context pressure, and retention is wall-clock days. Folding them
  couples two unrelated cadences and two kinds of clock in one place.
- **Deleting a group's authorization with its conversation.** Rejected: the authorization is
  the operator's admission of a channel, not a member's data, and a quiet group that speaks
  again is served without being admitted a second time.
- **Reading the age in the application.** Rejected: the application's one clock reading
  carries a local date with no instant, so comparing it against stored stamps would need a
  second clock and a second decision about time zones, both free to drift from the one that
  wrote the rows.
