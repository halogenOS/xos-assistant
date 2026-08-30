# 0152 — For a bot sender, only an @mention addresses the assistant

Date: 2026-08-30, with unit 42.

## Context

A group message addresses the assistant in three forms today, translated where the
platform's forms are known: an @mention of its handle, a reply to one of its own messages,
and the configured wake name as a whole word. The three are a union, and a union is exactly
what a bot walks into by accident — a moderation bot quoting the assistant's message, or
announcing something that happens to contain its name, addresses it under two of the three.

## Decision

In the adapter, where the platform's addressing forms are translated, a bot sender's group
message is addressed if and only if it mentions the assistant's handle. A bot's reply to
the assistant does not address it, and a bot speaking the wake name does not address it.
Non-bot senders keep the three-form union untouched.

The mention is the one deliberate act a bot cannot perform by accident: writing the
assistant's handle into the text is a decision the other bot's operator made.

A direct channel stays addressed by definition, bot sender included. The narrowing is a
group rule, and the platform delivers no bot-to-bot private message anyway.

This decision lives in the adapter because which platform forms count as addressing is
translation, exactly where the three forms already live. The core receives the same neutral
addressed fact it always did.

## Rejected alternatives

- **Keeping the union for bots.** A moderation bot that replies to or quotes the
  assistant's message would reopen the exact hole this closes — and replying is precisely
  what a moderation bot does to a message it acts on.
- **Deciding the narrowing in the core.** The core would have to know what a reply and a
  wake name are, which is platform vocabulary; the addressed flag exists so it does not.
- **Dropping the wake name for everyone.** People use it and it works; the problem is
  automated senders, not the form.
