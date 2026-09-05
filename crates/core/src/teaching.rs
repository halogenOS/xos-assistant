//! The prompt sections the assembly composes from its configuration
//! (unit 14, 2026-08-23): the name identity, the answering-mode teaching,
//! and — since unit 15, 2026-08-24 — the moderation teaching for the
//! deployments that can act on it.
//!
//! The embedder's prompt files stay prose an operator edits; what depends
//! on configuration — the resolved name, the answering mode, the
//! moderation capability — is behavior and composes here, in the core, so
//! the wording cannot drift from the mechanism that reads it. The
//! composition joins the configured prompt first and the composed sections
//! after it, and the assembly records the result as every new
//! conversation's system prompt; like any prompt edit, a changed name,
//! mode or moderation handle reaches new conversations only.
//!
//! Silence needs no vocabulary of its own (unit 22, 2026-08-24; re-keyed
//! by unit 55, 2026-09-02): the model stays silent by ending its turn
//! without SENDING anything. Writing is no longer the act — what the model
//! writes is private notes that reach nobody, and a message reaches the
//! group only through a sending tool — so every silence sentence here says
//! "without sending", and the contract that makes those words mean
//! something is composed ahead of them ([`SENDING_CONTRACT`]). When
//! addressed and unable to back an answer with a lookup, the model says it
//! doesn't know in its own words — ordinary prose, sent like any message,
//! with no machine routing.

use crate::assembly::AnsweringMode;

/// What this deployment can actually do, as the composition needs to know
/// it: one field per capability whose teaching is gated on its own
/// mechanism existing (unit 27, 2026-08-29). Named fields instead of a row
/// of positional booleans — two adjacent flags at a call site are one
/// silent swap away from teaching a tool the conversation does not have, which
/// is the exact defect this gating exists to prevent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// A moderation handle is configured — the report tool's half of its
    /// predicate; the answering mode is the other half.
    pub moderation_handle: bool,
    /// A web-search key is configured, which is the search tool's whole
    /// predicate: with no key the tool is not admitted and its teaching is
    /// not composed.
    pub web_search: bool,
}

/// Whether the moderation teaching composes — and, in the assembly, whether
/// the report tool registers: a moderation handle is configured AND the
/// answering mode is helpful, the two conditions autonomous assessment
/// needs (the report line goes nowhere without a handle; only helpful
/// answering shows the model every message it would judge). One predicate
/// for the teaching and the registration, so the prompt can never instruct
/// a tool the conversation does not have, and a registered tool is never left
/// untaught (unit 15, 2026-08-24).
#[must_use]
pub fn moderation_taught(handle_configured: bool, answering: AnsweringMode) -> bool {
    handle_configured && answering == AnsweringMode::Helpful
}

/// The moderation teaching, verbatim (unit 15, 2026-08-24): the model
/// judges each message against the pinned rules and reports a clear
/// violation through the report tool, naming the message by the msgid its
/// envelope shows — reasoned first, assessment only, the
/// administrators decide (decision 0070). The no-report cases are taught
/// beside the capability: borderline calls, rule-absent messages, and
/// every message while no rules are pinned. The pinned statement is named
/// the group's ONLY rules source, and its absence is taught as honestly as
/// its content: with no rules statement present the group has no rules
/// set — the model says so plainly, invents none, and reports nothing
/// (AC8's no-rules half, beside the base prose that carries no rules).
///
/// Since unit 36 (2026-08-29) the same teaching carries the join rule: a
/// join notice carries an envelope of its own, a shown name that is
/// itself unmistakably promotional bait — obvious at a glance to anyone —
/// is the violation before the account has spoken, and it is reported on
/// sight by that id; a name that is merely suspect is not bait, and doubt
/// means no report (unit 44's bar, both halves). The report is the whole action — no ban, no kick, no
/// reply to the joiner — so decision 0070 is untouched: the assistant
/// assesses and the group's administrators decide. It rides here, behind
/// [`moderation_taught`]'s two conditions, because the rule is worthless
/// where the report tool is not admitted.
pub const MODERATION_TEACHING: &str = "You also assess each group message against the group's pinned rules, \
     shown to you as the newest rules statement. That statement is the \
     group's only rules source: when no rules statement is present, the \
     group has no rules set — say so plainly if asked, invent none, and \
     report nothing. When a message clearly \
     violates those rules, think it through first, then file a report with \
     the report_spam tool, naming the violating message by the msgid its \
     envelope shows. Report only clear violations: do not report \
     borderline calls, messages no rule covers, or anything when no rules \
     are pinned. A report is an assessment for the group's administrators, \
     who decide; you never ban, mute or remove anyone. Reporting and \
     answering are independent: you may report and still answer, or report \
     and stay silent. The tool is the only way to report — never write the \
     report command into an answer yourself. \
     You also see join notices, each under an envelope of its own: a \
     line stating that someone joined the group, under the name the \
     platform showed. When a joiner's shown name is itself unmistakably \
     promotional bait — an advertisement, a solicitation or a come-on \
     carried in place of a name, obvious at a glance to anyone — that name \
     is the violation before the account has said anything, and you report \
     the join on sight, naming it by its msgid exactly as you would name a \
     violating message. A name that merely sounds promotional, or \
     that you suspect but cannot be certain of, is not bait: report only \
     what is beyond doubt, and when in doubt, do nothing. Filing \
     the report is the whole action: you never ban, kick, or reply to the \
     joiner, and a join you do not report needs no comment.";

/// The web search teaching, verbatim (unit 27, 2026-08-29), composed
/// exactly when the search tool is admitted — one predicate for both, so
/// the prompt never teaches a tool the conversation does not have. Four
/// things: the heads-up line ahead of the call (unit 40, 2026-08-30); a
/// snippet is a hint and an answer built on one says where it came from; a
/// snippet that does not contain the claim is a miss, exactly as the
/// sourcing rule already says of a lookup; and the carve-out that makes
/// registering a web tool safe at all — project facts still come only from
/// the project lookups. Without that last sentence, the sourcing rule's
/// "your lookup tools are the only source of substantive claims" would
/// silently authorise a random web page to back a claim about the project.
///
/// The heads-up line rides HERE, inside the gated teaching, because the
/// search is the slow work: it is the one tool that reaches the open web,
/// and the composing cue goes dark while a call runs, so the line is the
/// only sign of life in that window. Since unit 55 the line is SENT, with
/// the plain sending tool, because that is the only way anything the model
/// writes reaches a chat; nothing mechanizes it either way — the model
/// decides whether to make that call — so the behavior stays taught prose
/// and therefore probabilistic, never a guarantee the mechanism enforces.
/// Its wording carries its own bounds so it cannot decay into filler: one
/// line, what is being looked up, never a placeholder, never the member's
/// words read back. The last clause settles it against the silence
/// teaching, whose end-the-turn-without-sending rule governs a turn with
/// nothing to say — an announcing turn has a search to run.
pub const SEARCH_TEACHING: &str = "You can also search the web with the search_web tool, for questions about \
     the world and not about the project. Before you run a search, send one \
     short line with send_message saying what you are about to look up, then \
     run the search, then send the answer: one line and no more, stating the \
     thing you are going to look for — never a placeholder standing in for an \
     answer, and never a restatement of the words the member just wrote. \
     Ending a turn without sending is for a turn with nothing to say; a turn \
     with a search to run has something to say. A result's snippet is a hint, \
     not a source: when you answer from one, say where it came from and name \
     the page. A snippet that does not contain the claim is a miss, exactly \
     as an unanswering lookup is — say you don't know instead of filling the \
     gap from memory. Facts about the project itself still come only from the \
     project lookups: a web result is never the source for a claim about \
     halogenOS, its features, its procedures or its builds.";

/// The react teaching, verbatim (unit 39, 2026-08-30), composed
/// unconditionally exactly as the react tool is admitted unconditionally —
/// one predicate for both, and here the predicate is "always", so the two
/// cannot drift. A reaction needs nothing but a chat, which is why it
/// carries none of the report's conditions.
///
/// What it teaches is the trigger and the two bounds. The trigger is the
/// TERMINAL MESSAGE, as decision 0197 defines it (2026-09-02, with unit
/// 54, superseding decision 0155's chatter fit): a response to
/// the assistant that needs no further response — the thanks that closes
/// an exchange already answered — may be stamped off with one reaction
/// where the silence default would otherwise end the turn empty. The
/// sentence stays permissive: it says a reaction CAN close such a message,
/// never that one should. The bounds are that words and a reaction never
/// land on one message — an answer already acknowledges it — and that one
/// message takes one reaction, ever.
///
/// It composes in the addressed mode too, and legitimately: there the
/// empty-turn clause covers an ADDRESSED message that leaves nothing to
/// say, which is exactly what a closing thanks is. The carve-out that
/// rides the helpful arm's silence sentence states the same rule where
/// that arm's silence default would otherwise contradict it.
///
/// No emoji appears here, deliberately: which emoji a platform can place
/// is a platform fact and lives in the adapter, and the core's own
/// cleanliness scans would fail on a glyph in this file. Taste — which
/// emoji suits which moment — is the deployed persona's, not the core's.
pub const REACT_TEACHING: &str = "You can also put one emoji reaction on a message, with the \
     react tool: name the message by the msgid its envelope shows and give the emoji \
     you choose. A response to you that needs no further response can be stamped off with \
     one reaction instead of an empty turn: someone asks you how something works, you \
     answer, they write back thanks — that thanks can take a reaction and nothing more. \
     Words and a reaction never land on one message: when you answer in words, the answer \
     is the whole of it. Most messages deserve no reaction at all, one message takes at \
     most one reaction ever, and silence stays the default.";

/// The closing prohibition (unit 54, 2026-09-02; halved by unit 55,
/// 2026-09-02), composed unconditionally: it binds every deployment,
/// because it names no capability a configuration can remove.
///
/// It PUSHES nothing, and that is unit 54's whole decision. The assistant
/// gained two tools that end a turn with nothing posted, and the teaching
/// gained no directive to call them: a rule that pushes an act gets the act
/// stamped on everything, and those tools are a protection against a model
/// that loops because it must do something, not a behaviour to encourage.
/// What each tool is for lives in its own model-facing description. Ending
/// a turn in silence stays the taught default, exactly as before.
///
/// THE ANNOUNCEMENT is what it forbids. Observed in production on
/// 2026-09-01: asked a question aimed at another member by name, the
/// assistant posted that the question was not for it and it was staying
/// out. That message is the silence written out, and it belongs to nobody.
///
/// Unit 54's second sentence — the narrated close, which warned that prose
/// written ahead of a tool call is posted as its own message — is GONE, and
/// its mechanism with it: from unit 55 nothing the model writes is posted
/// anywhere, so a narrated ending narrates to nobody and the warning would
/// describe a machine that no longer exists.
pub const CLOSING_PROHIBITIONS: &str = "Never send a message whose only content is that you \
     are not taking part: a line saying the question was for someone else, that you are \
     staying out of it, or that you have nothing to add is your silence written out, and \
     nobody needs to read it.";

/// The speaking contract, verbatim (unit 55, 2026-09-02), composed
/// unconditionally: it describes the machine every deployment runs, and
/// there is no configuration under which it is false.
///
/// It states four things and instructs almost nothing, because it is a
/// description of what happens rather than a rule about what to do:
///
/// 1. WRITTEN TEXT IS PRIVATE. This is the change, and it is stated first
///    and plainly. Everything the model writes is its own notes; no chat
///    ever reads a word of it.
/// 2. THE TWO DOORS. A message reaches the group through the sending tools
///    and through nothing else. Both are named, with the difference
///    between them — the reply names the message it answers — and where the
///    id for that comes from.
/// 3. WHAT ONE TURN MAY DO. Several messages, several people answered, or
///    nothing at all: the whole point of the change, in the operator's own
///    terms, is that the assistant is no longer forced into one answer per
///    turn or none.
/// 4. THE ENVELOPE. Every message the model reads carries one, and its
///    msgid is the token the reply tool, the report tool and the react tool
///    all aim by.
///
/// The two effects that are NOT sends stay named as what they are: a filed
/// report's line and an emoji reaction are the tools' own effects, and
/// neither is the model's text reaching anybody.
///
/// Silence keeps its place: it is the default here as everywhere else in
/// the prompt, and a turn that ends without a send posts nothing. That
/// sentence is what keeps the contract from reading as an instruction to
/// send.
pub const SENDING_CONTRACT: &str = "What you write is your own private notes. It is never \
     posted and nobody in the group reads it. A message reaches the group only when you send \
     it: with the send_message tool to post to the chat, or with the reply_message tool to \
     answer one message in particular, naming it by the msgid its envelope shows. One turn of \
     yours can send several messages, answer several people, or send nothing at all. Silence \
     stays the default: a turn that ends without sending posts nothing, which is exactly \
     right whenever there is nothing to say. Every message you read carries an envelope above \
     it naming who wrote it, when the chat says it was sent, and its msgid. Two things that \
     are not messages still reach the group as they always did, because they are a tool's own \
     effect and not your text: a report you file, and an emoji reaction you place.";

/// The whole system prompt the assembly records: the embedder's prompt,
/// then the name identity, then the speaking contract, then the answering
/// teaching for the configured mode, then the react teaching and the
/// closing prohibition — all of them unconditional — and then, each exactly
/// when its own capability is there, the moderation teaching and the web
/// search teaching. Public because the suites assert recorded prompts
/// against exactly this composition instead of restating it.
///
/// The contract comes BEFORE the answering teaching, and the order is what
/// makes the rest readable: every silence sentence below it says "without
/// sending", which means what it means only once the reader knows that
/// sending is an act of its own.
#[must_use]
pub fn composed_system_prompt(
    base: &str,
    name: &str,
    answering: AnsweringMode,
    capabilities: Capabilities,
) -> String {
    let mut prompt = format!(
        "{base}\n\n{identity}\n\n{SENDING_CONTRACT}\n\n{teaching}\n\n{REACT_TEACHING}\
         \n\n{CLOSING_PROHIBITIONS}",
        identity = identity_section(name),
        teaching = answering_section(answering),
    );
    if moderation_taught(capabilities.moderation_handle, answering) {
        prompt.push_str("\n\n");
        prompt.push_str(MODERATION_TEACHING);
    }
    if capabilities.web_search {
        prompt.push_str("\n\n");
        prompt.push_str(SEARCH_TEACHING);
    }
    prompt
}

/// The name identity: what the assistant is called, that the
/// are-you-a-bot question about that name is a question about itself —
/// answered honestly, per decision 0080's teaching — and, since unit 32
/// (2026-08-28), where the answer to what-are-you-running-on comes from:
/// the runtime-facts tool, never the model's memory and never the chat,
/// both of which state a model the deployment may have moved off. Unit 37
/// (2026-08-30) routes the same way the questions about the host the
/// software runs on and the software it is built from, which memory
/// answers just as badly. Unit 47 (2026-08-30) routes the same way the
/// question of what changed in the assistant itself, to the
/// harness-changelog tool, with a sentence that keeps the assistant's own
/// changes apart from halogenOS's releases — the question the group asks
/// far more often, which stays with the release lookup. The sentences
/// compose in both modes and under every configuration, because the tools
/// they name register the same way.
fn identity_section(name: &str) -> String {
    format!(
        "You are called {name}. When someone asks whether {name} is an AI, \
         a bot, or a machine, that question is about you: answer it honestly, \
         as the AI system you are. When someone asks which model you run on, \
         which version you are, how long you have been running, which \
         operating system or architecture you run on, or what you are built \
         on, call the {tool} tool and answer from what it returns — never \
         from memory and never from what this conversation said earlier. \
         When someone asks what changed, what is new, or what was updated in \
         you — the assistant itself — call the {changelog} tool and answer \
         from what it returns, never from memory and never from what this \
         conversation said earlier. That tool carries this assistant \
         software's own changelog and nothing else: a question about a \
         halogenOS release or about changes in halogenOS belongs to the \
         release lookup, never to it.",
        tool = crate::tools::runtime::NAME,
        changelog = crate::tools::changelog::NAME
    )
}

/// The hardened silence sentence, as unit 39 amended it (2026-08-30). It
/// stood as "they get nothing from you, not an answer, not an
/// acknowledgment, not a comment" — a rule about acts, which the reaction
/// would have contradicted on a literal read. The amendment scopes it to
/// WORDS, which is what it always meant and what it can keep meaning
/// beside [`REACTION_CARVE_OUT`].
///
/// Decision 0148 refused to carve an exception into this sentence for the
/// heads-up line, resolving that case by wording alone — it could, because
/// the announce added no new ACT. This unit adds one, so the sentence
/// itself changes. Pinned byte for byte: the copy is this unit's, and its
/// two halves are constants so the composition cannot reword them.
pub(crate) const SILENCE_IN_WORDS: &str =
    "they get nothing from you in words: not an answer, not an acknowledgment, not a comment.";

/// The carve-out that joins the amended sentence, in the helpful arm
/// alone (unit 39, 2026-08-30; its trigger rewritten by decision 0197,
/// 2026-09-02, with unit 54). It rides HERE and not beside the rule as a
/// separate paragraph, because the composed prompt would otherwise contradict
/// itself on a literal read — the exact collision decision 0148 documents.
///
/// The trigger is the terminal message, the same one [`REACT_TEACHING`]
/// states: a response to the assistant that needs no further response.
/// The chatter wording this carried before — a share, a milestone, a joke
/// — is gone from both homes, because one teaching cannot name two
/// different triggers for one act; decision 0197 supersedes the chatter
/// fit decision 0155 gave it, and the bounds of 0155 live on here.
///
/// The addressed arm is deliberately left unamended: there this arm's
/// silence sentence does not exist, so there is nothing for a carve-out to
/// except. What the addressed mode gets is [`REACT_TEACHING`], whose
/// empty-turn clause covers the addressed message that leaves nothing to
/// say.
///
/// Asserted byte for byte, its two dashes spelled as codepoints: the copy
/// fixes an em dash there, and an em dash pasted through an editor is one
/// silent keystroke away from a hyphen or an en dash. The assertion below
/// spells the glyph instead, so the two sides cannot drift together.
pub(crate) const REACTION_CARVE_OUT: &str = "The one exception is the emoji reaction: a \
     response to you that needs no further response \u{2014} the thanks that closes an \
     exchange you already answered \u{2014} can be stamped off with one reaction instead of \
     the empty turn. A reaction never rides with words on the same message, most messages \
     deserve no reaction either, and silence stays the default.";

/// The answering teaching for one mode (rewritten by unit 16 and unit 22,
/// extended by unit 21, 2026-08-24; the silence sentence amended by unit
/// 39, 2026-08-30). Both modes teach the sourcing
/// discipline, the audience discipline and the end-empty silence — a turn
/// with nothing to say ends with no text wherever it runs — and the
/// helpful mode adds the silence-default judgment for messages that never
/// addressed the assistant, with the reaction carve-out that judgment now
/// carries, while the addressed mode adds the follow-up
/// reach: an unaddressed reply never opens a turn there, so a clarifying
/// question invites the member to reply to it.
fn answering_section(answering: AnsweringMode) -> String {
    let sourcing = sourcing_rules();
    let audience = audience_rules();
    let revisions = revision_rules();
    match answering {
        AnsweringMode::Helpful => format!(
            "Every message in a group conversation reaches you, including \
             messages that do not address you, and you decide whether to \
             speak. Silence is the strong default: most group messages are \
             members talking among themselves — often replying to each \
             other in threads you cannot see — and {SILENCE_IN_WORDS} \
             {REACTION_CARVE_OUT} \
             Speak only when a message addresses you — a mention, your \
             name, a reply to you — or asks a concrete question that \
             nobody else is answering and your lookups can settle. A \
             statement that asks nothing never warrants a reply, and if \
             someone else already answered, stay silent. In a busy \
             conversation, holding back is right even when you could add \
             something. When you do not speak, end your turn without \
             sending anything — no placeholder message. An answer is the \
             exception, and an answer that makes a substantive claim must \
             be one you can back with a lookup. {sourcing} {audience} \
             {revisions}"
        ),
        AnsweringMode::Addressed => format!(
            "You are brought in when a message addresses you: a mention, a \
             reply to one of your messages, your name, or a direct chat. \
             Answer what was asked of you; when even an addressed message \
             leaves you nothing useful to say, end your turn without \
             sending anything — no placeholder message. \
             {sourcing} {audience} {revisions} When you ask a clarifying \
             question, \
             invite the member to reply to your message: only a message \
             that addresses you reaches you, so a plain follow-up would \
             otherwise go unseen."
        ),
    }
}

/// The sourcing discipline, shared by both modes so the operator's rule has
/// one spelling: the tools are the only source of substantive claims, the
/// lookup comes before the answer, an unanswering lookup is a miss, no
/// guesses and no hedged trained knowledge — and the honest outcomes in
/// the model's own voice: a plain "I don't know" when addressed, no text
/// at all otherwise. The addressed-versus-silent choice is the model's
/// own judgment.
fn sourcing_rules() -> String {
    "Your lookup tools are the only source of substantive claims: any \
     claim about the project — a feature, a procedure, a project fact — \
     must come from a lookup you made in this turn, never from your \
     trained knowledge, so look it up before you answer. A lookup \
     answers a question only when its result actually contains the \
     answer: a result that is empty, off-topic, or missing the specific \
     claim is a miss, not a licence to fill the gap from memory. Never \
     guess and never answer from hedged memory — no \"as far as I \
     know\", no \"probably\" — and in a compound answer, every \
     project-specific claim is either confirmed by a lookup or dropped. \
     When you were addressed and a lookup cannot back the answer, say \
     you don't know, plainly and in your own words — never guess from \
     memory or offer a hedged recollection. When you were not addressed \
     and have nothing to add, end the turn without sending."
        .to_owned()
}

/// The revision discipline, shared by both modes so the rule has one
/// spelling (unit T3, 2026-08-31): a person may say the same thing again,
/// differently, and the conversation then holds both wordings under one
/// id. Three sentences, and no fourth — what a person meant by their own
/// edit is theirs to mean, so the last clause teaches the judgment instead
/// of mechanising it.
///
/// No platform vocabulary: the marker and the id are what the projection
/// shows, and nothing here names an event type, a field or a platform.
fn revision_rules() -> &'static str {
    "A message may appear again marked as edited under the same id: the \
     edited version is what the person now means, so answer that one. When \
     the earlier wording was already answered and the edit does not change \
     what was asked, end the turn without sending."
}

/// The audience discipline, shared by both modes so it applies wherever a
/// question is answered (unit 21, 2026-08-24): the same question reads one
/// way to an end user and another to a builder, the audience is read from
/// the message and the conversation — never from a profile of the person —
/// genuine ambiguity draws exactly one brief clarifying question, a clear
/// question is answered directly, and clarifying questions never chain.
/// The lookup-backing rule is reconciled here, narrowly: a question back
/// to the asker makes no substantive claim and needs no lookup, while the
/// real answer it unlocks needs its lookup exactly as before.
fn audience_rules() -> &'static str {
    "The same question often reads one way from an end user who wants to \
     use a feature on their device and another from a developer who wants \
     to build it into a ROM or integrate it, and the right answer differs \
     sharply between the two. Read the audience from what the message and \
     the conversation show — the words, the level, the prior turns — never \
     from assumptions about who the person is. When the intent is clear, \
     answer that reading directly. When a question is genuinely ambiguous \
     between using and building, ask one brief clarifying question — \"are \
     you asking how to use it on your device, or how to build it into a \
     ROM?\" — instead of committing to an assumption. A clarifying question \
     back to the asker is a warranted reply, not silence and not a \
     don't-know: it makes no substantive claim, so it needs no lookup, \
     while the real answer that follows the member's reply needs its \
     lookup like any substantive claim. Never chain clarifying questions: \
     when the reply still leaves the intent unclear, answer the likeliest \
     reading as well as your lookups allow instead of asking again."
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capabilities of a deployment with a moderation handle and no
    /// search key.
    fn moderating() -> Capabilities {
        Capabilities {
            moderation_handle: true,
            web_search: false,
        }
    }

    /// The capabilities of a deployment with a search key and no moderation
    /// handle.
    fn searching() -> Capabilities {
        Capabilities {
            moderation_handle: false,
            web_search: true,
        }
    }

    /// The composition order and the three facts the sections must carry:
    /// the base leads, the name reaches the identity, and each mode's
    /// teaching states the end-empty silence mechanism.
    #[test]
    fn the_prompt_composes_base_identity_and_mode_teaching() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt =
                composed_system_prompt("The base prose.", "Probe", mode, Capabilities::default());
            assert!(
                prompt.starts_with("The base prose.\n\n"),
                "the embedder's prompt leads"
            );
            assert!(
                prompt.contains("You are called Probe."),
                "the identity names the assistant"
            );
            assert!(
                prompt.contains("end your turn without sending anything — no placeholder message"),
                "the teaching states silence as the empty turn"
            );
        }
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains("including messages that do not address you"),
            "helpful mode teaches the undirected reach"
        );
        let addressed =
            composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default());
        assert!(
            addressed.contains("when a message addresses you"),
            "addressed mode teaches the summons shape"
        );
    }

    /// AC4 (unit 47): both modes' composed prompt routes the
    /// what-changed-in-you question to the harness-changelog tool, under
    /// every configuration, and the sentence keeps the two changelogs
    /// apart — the assistant's own changes go to this tool, halogenOS
    /// release questions stay with the release lookup. The tool registers
    /// on nothing, so the sentence composes on nothing.
    #[test]
    fn both_modes_route_change_questions_to_the_changelog_tool() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for moderation_handle in [false, true] {
                for web_search in [false, true] {
                    let capabilities = Capabilities {
                        moderation_handle,
                        web_search,
                    };
                    let prompt = composed_system_prompt("b", "n", mode, capabilities);
                    assert!(
                        prompt.contains(
                            "When someone asks what changed, what is new, or what was \
                             updated in you — the assistant itself — call the \
                             harness_changelog tool and answer from what it returns, \
                             never from memory and never from what this conversation \
                             said earlier. That tool carries this assistant software's \
                             own changelog and nothing else: a question about a \
                             halogenOS release or about changes in halogenOS belongs \
                             to the release lookup, never to it."
                        ),
                        "the {mode:?} teaching routes the question to the tool"
                    );
                }
            }
        }
    }

    /// AC6 (unit 32) and AC5 (unit 37): both modes' composed prompt routes
    /// the what-are-you-running-on question to the runtime-facts tool —
    /// the model, the version and the uptime, and the operating system,
    /// the architecture and what the software is built on — under every
    /// configuration, because the sentence composes on nothing and the
    /// tool it names registers on nothing.
    #[test]
    fn both_modes_route_identity_questions_to_the_runtime_tool() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for moderation_handle in [false, true] {
                let capabilities = Capabilities {
                    moderation_handle,
                    ..Capabilities::default()
                };
                let prompt = composed_system_prompt("b", "n", mode, capabilities);
                assert!(
                    prompt.contains(
                        "When someone asks which model you run on, which version you \
                         are, how long you have been running, which operating system \
                         or architecture you run on, or what you are built on, call \
                         the runtime_facts tool and answer from what it returns — \
                         never from memory and never from what this conversation \
                         said earlier."
                    ),
                    "the {mode:?} teaching routes the question to the tool"
                );
            }
        }
    }

    /// AC6 (unit 22, continuing unit 16's AC7): both modes' teaching
    /// carries the sourcing discipline verbatim — the tools as the only
    /// source of substantive claims, the lookup before the answer, the
    /// sufficiency rule that an unanswering lookup is a miss, the
    /// no-guessing and no-hedged-knowledge prohibition, and the honest
    /// outcomes in the model's own voice: the plain "I don't know" when
    /// addressed, the empty turn otherwise — the choice between them being
    /// the model's own reading of the message. The addressed mode's gain
    /// is exactly this discipline; the silence-default framing is pinned
    /// in the test below.
    #[test]
    fn both_modes_teach_the_sourcing_discipline() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for fact in [
                "Your lookup tools are the only source of substantive claims",
                "never from your trained knowledge, so look it up before you answer",
                "a result that is empty, off-topic, or missing the specific \
                 claim is a miss, not a licence to fill the gap from memory",
                "Never guess and never answer from hedged memory",
                "every project-specific claim is either confirmed by a lookup or dropped",
                "When you were addressed and a lookup cannot back the answer, \
                 say you don't know, plainly and in your own words — never \
                 guess from memory or offer a hedged recollection",
                "When you were not addressed and have nothing to add, end \
                 the turn without sending",
            ] {
                assert!(
                    prompt.contains(fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
    }

    /// AC9 (unit T3, 2026-08-31): both modes' teaching carries the
    /// revision rules and the whole sourcing paragraph beside them —
    /// compared against [`sourcing_rules`] itself, never against a copied
    /// string, so a reworded paragraph cannot pass here by having been
    /// reworded in two places. The revision teaching names no platform:
    /// what it points at is the marker and the id the projection shows.
    #[test]
    fn both_modes_teach_the_revision_rules_beside_the_whole_sourcing_paragraph() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            assert!(
                prompt.contains(&sourcing_rules()),
                "the {mode:?} teaching carries the sourcing paragraph whole"
            );
            assert!(
                prompt.contains(revision_rules()),
                "the {mode:?} teaching carries the revision rules whole"
            );
            for fact in [
                "A message may appear again marked as edited under the same id",
                "the edited version is what the person now means, so answer that one",
                "When the earlier wording was already answered and the edit does \
                 not change what was asked, end the turn without sending",
            ] {
                assert!(
                    prompt.contains(fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
    }

    /// AC2 (unit 21): both modes' teaching carries the audience discipline
    /// verbatim — the use-versus-build distinction, the read-the-message
    /// never-profile-the-person rule, the answer-directly-when-clear rule,
    /// the ask-one-brief-clarifying-question-on-genuine-ambiguity rule, the
    /// do-not-chain rule, and the reconciled lookup statement: a clarifying
    /// question is a warranted reply needing no lookup, while a substantive
    /// claim still needs one. The helpful mode's silence framing binds the
    /// lookup duty to the substantive claim, and the addressed mode names
    /// its follow-up reach.
    #[test]
    fn both_modes_teach_the_audience_discipline() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for fact in [
                "one way from an end user who wants to use a feature on \
                 their device and another from a developer who wants to \
                 build it into a ROM or integrate it",
                "Read the audience from what the message and the \
                 conversation show — the words, the level, the prior turns \
                 — never from assumptions about who the person is",
                "When the intent is clear, answer that reading directly.",
                "When a question is genuinely ambiguous between using and \
                 building, ask one brief clarifying question",
                "A clarifying question back to the asker is a warranted \
                 reply, not silence and not a don't-know: it makes no \
                 substantive claim, so it needs no lookup",
                "the real answer that follows the member's reply needs its \
                 lookup like any substantive claim",
                "Never chain clarifying questions: when the reply still \
                 leaves the intent unclear, answer the likeliest reading as \
                 well as your lookups allow instead of asking again",
            ] {
                assert!(
                    prompt.contains(fact),
                    "the {mode:?} teaching carries: {fact}"
                );
            }
        }
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains(
                "an answer that makes a substantive claim must be one you \
                 can back with a lookup"
            ),
            "the reconciled helpful sentence binds the lookup to the claim"
        );
        let addressed =
            composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default());
        assert!(
            addressed.contains(
                "When you ask a clarifying question, invite the member to \
                 reply to your message"
            ),
            "the addressed teaching names its follow-up reach"
        );
    }

    /// AC6's silence half (unit 22): helpful mode leads with silence as
    /// the default and keeps the unit-16 lookup-backed sentence verbatim —
    /// the new end-empty rule completes it, it does not contradict it —
    /// and no sentinel vocabulary survives anywhere in either mode's
    /// prompt.
    #[test]
    fn silence_is_the_default_and_no_sentinel_vocabulary_survives() {
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains("Silence is the strong default"),
            "helpful mode leads with silence"
        );
        assert!(
            helpful.contains(
                "an answer that makes a substantive claim must be one you \
                 can back with a lookup"
            ),
            "the unit-16 lookup-backed sentence stands verbatim"
        );
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let prompt = composed_system_prompt("b", "n", mode, Capabilities::default());
            for token in ["[[", "]]", "abstain", "abstention", "sentinel"] {
                assert!(
                    !prompt.to_lowercase().contains(token),
                    "the {mode:?} teaching carries no sentinel vocabulary: {token}"
                );
            }
            assert!(
                prompt.contains("end your turn without sending anything — no placeholder message"),
                "the {mode:?} teaching states the empty turn as the silence mechanism"
            );
        }
    }

    /// AC7's prompt half at the composition itself: the moderation teaching
    /// rides the prompt exactly when a handle is configured AND the mode is
    /// helpful — absent either condition, no moderation instruction exists
    /// for a tool that is not there.
    #[test]
    fn the_moderation_teaching_composes_only_with_a_handle_and_helpful_mode() {
        let taught = composed_system_prompt("b", "n", AnsweringMode::Helpful, moderating());
        assert!(
            taught.ends_with(MODERATION_TEACHING),
            "handle plus helpful composes the moderation teaching last"
        );
        for (name, prompt) in [
            (
                "helpful without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default()),
            ),
            (
                "addressed with a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, moderating()),
            ),
            (
                "addressed without a handle",
                composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default()),
            ),
        ] {
            assert!(
                !prompt.contains("report_spam"),
                "{name} teaches no moderation"
            );
            assert!(
                !prompt.contains("join notices"),
                "{name} teaches no join rule either: the rule is worthless \
                 where the report tool is not admitted"
            );
        }
        assert!(moderation_taught(true, AnsweringMode::Helpful));
        assert!(!moderation_taught(false, AnsweringMode::Helpful));
        assert!(!moderation_taught(true, AnsweringMode::Addressed));
    }

    /// AC5's prompt half and AC9's carve-out (unit 27): the search teaching
    /// rides the prompt exactly when a search key is configured, in either
    /// answering mode — and with no key, no sentence of it exists for a
    /// tool that is not there.
    #[test]
    fn the_search_teaching_composes_only_with_a_configured_key() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            let taught = composed_system_prompt("b", "n", mode, searching());
            assert!(
                taught.ends_with(SEARCH_TEACHING),
                "a configured key composes the search teaching last in {mode:?} mode"
            );
            let untaught = composed_system_prompt("b", "n", mode, Capabilities::default());
            assert!(
                !untaught.contains("search_web"),
                "no key teaches no search in {mode:?} mode"
            );
        }
        // Both capabilities at once compose both teachings, each whole.
        let both = composed_system_prompt(
            "b",
            "n",
            AnsweringMode::Helpful,
            Capabilities {
                moderation_handle: true,
                web_search: true,
            },
        );
        assert!(both.contains(MODERATION_TEACHING) && both.contains(SEARCH_TEACHING));
    }

    /// The search teaching's copy, pinned on the three things it must
    /// carry — above all the carve-out, without which registering a web
    /// tool would silently authorise a web page to back a project claim.
    #[test]
    fn the_search_teaching_carries_the_snippet_rules_and_the_project_carve_out() {
        for fact in [
            "the search_web tool",
            "for questions about the world and not about the project",
            "A result's snippet is a hint, not a source",
            "say where it came from and name the page",
            "A snippet that does not contain the claim is a miss",
            "Facts about the project itself still come only from the project lookups",
            "a web result is never the source for a claim about halogenOS",
        ] {
            assert!(
                SEARCH_TEACHING.contains(fact),
                "the search teaching carries: {fact}"
            );
        }
    }

    /// AC2 (unit 40): the heads-up line before slow work rides the search
    /// teaching and nothing else — it composes exactly when the search
    /// capability is admitted, in either answering mode, and no
    /// configuration without a key carries a word of it. Its wording is
    /// pinned on the four facts the wording decision fixed, because each
    /// one is what keeps the line from decaying into the filler the
    /// conduct prose and the silence teaching forbid: one line, what is
    /// being looked up, no placeholder, no restating — plus the clause
    /// that settles it against the end-your-turn-with-no-text rule.
    #[test]
    fn the_announce_line_rides_the_search_teaching_and_only_there() {
        for fact in [
            "Before you run a search, send one short line with send_message \
             saying what you are about to look up, then run the search, then \
             send the answer",
            "one line and no more",
            "stating the thing you are going to look for",
            "never a placeholder standing in for an answer",
            "never a restatement of the words the member just wrote",
            "a turn with a search to run has something to say",
        ] {
            assert!(
                SEARCH_TEACHING.contains(fact),
                "the search teaching carries: {fact}"
            );
            for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
                assert!(
                    composed_system_prompt("b", "n", mode, searching()).contains(fact),
                    "a configured key composes it in {mode:?} mode: {fact}"
                );
                for capabilities in [Capabilities::default(), moderating()] {
                    assert!(
                        !composed_system_prompt("b", "n", mode, capabilities).contains(fact),
                        "no key teaches no announce in {mode:?} mode under \
                         {capabilities:?}: {fact}"
                    );
                }
            }
        }
    }

    /// AC-A (unit 39): the amended silence sentence and the carve-out are
    /// pinned BYTE FOR BYTE, both as constants and as they land in the
    /// composed helpful prompt — the copy is the unit's, and a
    /// re-wrapping that changed a space would fail here. The old
    /// act-scoped sentence is asserted gone: leaving it beside the
    /// carve-out is the literal self-contradiction decision 0148
    /// documents.
    #[test]
    fn the_silence_sentence_is_amended_and_carries_the_carve_out() {
        assert_eq!(
            SILENCE_IN_WORDS,
            "they get nothing from you in words: not an answer, not an acknowledgment, \
             not a comment."
        );
        assert_eq!(
            REACTION_CARVE_OUT,
            "The one exception is the emoji reaction: a response to you that needs no \
             further response — the thanks that closes an exchange you already answered \
             — can be stamped off with one reaction instead of the empty turn. A \
             reaction never rides with words on the same message, most messages deserve \
             no reaction either, and silence stays the default."
        );
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains(SILENCE_IN_WORDS),
            "the helpful arm carries the amended sentence verbatim"
        );
        assert!(
            helpful.contains(REACTION_CARVE_OUT),
            "the carve-out joins it verbatim, in the same sentence's place"
        );
        assert!(
            helpful.contains(&format!("{SILENCE_IN_WORDS} {REACTION_CARVE_OUT}")),
            "the carve-out follows the sentence it excepts, with one space between"
        );
        assert!(
            !helpful.contains("they get nothing from you, not an answer"),
            "the act-scoped sentence is amended, not left standing beside its exception"
        );
        // The addressed arm is unamended on purpose: unaddressed chatter
        // opens no turn there, so the chatter carve-out never meets it.
        let addressed =
            composed_system_prompt("b", "n", AnsweringMode::Addressed, Capabilities::default());
        assert!(
            !addressed.contains(SILENCE_IN_WORDS) && !addressed.contains(REACTION_CARVE_OUT),
            "the addressed arm keeps its own silence rule untouched"
        );
    }

    /// AC-A's tool half (unit 39), with the trigger unit 54 rewrote: the
    /// react teaching composes unconditionally — every mode, every
    /// configuration, exactly as the tool registers — and states the
    /// terminal-message trigger with its example, permissively, beside the
    /// bounds that survive untouched. The chatter wording it carried is
    /// asserted GONE from both homes: one act cannot have two triggers.
    /// It carries no emoji of its own, which the core's cleanliness scans
    /// would fail on anyway; taste is the deployed persona's.
    #[test]
    fn the_react_teaching_composes_everywhere_and_states_its_bounds() {
        for replaced in ["a share, a milestone", "Chatter that lands"] {
            assert!(
                !REACT_TEACHING.contains(replaced) && !REACTION_CARVE_OUT.contains(replaced),
                "the chatter wording is replaced, not left beside its replacement: {replaced}"
            );
        }
        for fact in [
            "with the react tool",
            "name the message by the msgid its envelope shows",
            "A response to you that needs no further response can be stamped off with \
             one reaction instead of an empty turn",
            "someone asks you how something works, you answer, they write back thanks \
             — that thanks can take a reaction and nothing more",
            "Words and a reaction never land on one message",
            "one message takes at most one reaction ever",
            "Most messages deserve no reaction at all",
            "silence stays the default",
        ] {
            assert!(
                REACT_TEACHING.contains(fact),
                "the react teaching carries: {fact}"
            );
        }
        assert!(
            !REACT_TEACHING.chars().any(|c| {
                let point = c as u32;
                (0x1F000..=0x1FAFF).contains(&point)
                    || (0x2600..=0x27BF).contains(&point)
                    || point == 0x200D
                    || point == 0xFE0F
            }),
            "the react teaching names no emoji: which emoji a platform places is the \
             adapter's fact, and the taste is the persona's"
        );
        // The mood is the TRIGGER SENTENCE's, so the read is that
        // sentence's alone: the constant around it is free to say
        // "should" about anything else, and a later sentence carrying
        // the letters in another word decides nothing here.
        for teaching in [REACT_TEACHING, REACTION_CARVE_OUT] {
            let trigger = teaching
                .split_inclusive('.')
                .find(|sentence| sentence.contains("can be stamped off with one reaction"))
                .expect("the trigger sentence stands in the teaching");
            assert!(
                !trigger.contains("should"),
                "the trigger sentence stays permissive: it says a reaction CAN close \
                 such a message, never that one should: {trigger}"
            );
        }
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for capabilities in every_capabilities() {
                assert!(
                    composed_system_prompt("b", "n", mode, capabilities).contains(REACT_TEACHING),
                    "the react teaching composes in {mode:?} mode under {capabilities:?}"
                );
            }
        }
    }

    /// The moderation teaching's copy, pinned on the facts it must carry:
    /// the tool, the bracketed-id naming, the reasoned judgment, the
    /// only-rules-source rule with its no-rules honesty (AC8's no-rules
    /// half), the no-report cases (AC4's judgment half), the decision
    /// boundary of 0070, the report-answer independence, and the
    /// never-type-the-command rule.
    #[test]
    fn the_moderation_teaching_carries_its_facts() {
        for fact in [
            "the report_spam tool",
            "naming the violating message by the msgid its envelope shows",
            "think it through first",
            "That statement is the group's only rules source",
            "when no rules statement is present, the group has no rules \
             set — say so plainly if asked, invent none, and report nothing",
            "do not report borderline calls, messages no rule covers, or \
             anything when no rules are pinned",
            "an assessment for the group's administrators, who decide",
            "you never ban, mute or remove anyone",
            "Reporting and answering are independent",
            "never write the report command into an answer yourself",
        ] {
            assert!(
                MODERATION_TEACHING.contains(fact),
                "the moderation teaching carries: {fact}"
            );
        }
    }

    /// AC6 (unit 36): the join rule rides the same teaching, and since
    /// unit 44 (2026-08-30) it is pinned WHOLE — the entire join block,
    /// byte for byte, from the join-notice line to the closing no-comment
    /// sentence. One assertion, so no word of the bar — the certainty
    /// trigger, the aside, the suspicion carve-out, the doubt rule, the
    /// whole-action close — can drift silently.
    #[test]
    fn the_moderation_teaching_carries_the_join_rule_as_the_whole_action() {
        assert!(
            MODERATION_TEACHING.ends_with(
                "You also see join notices, each under an envelope of its own: a \
                 line stating that someone joined the group, under the name the \
                 platform showed. When a joiner's shown name is itself unmistakably \
                 promotional bait — an advertisement, a solicitation or a come-on \
                 carried in place of a name, obvious at a glance to anyone — that name \
                 is the violation before the account has said anything, and you report \
                 the join on sight, naming it by its msgid exactly as you would name a \
                 violating message. A name that merely sounds promotional, or \
                 that you suspect but cannot be certain of, is not bait: report only \
                 what is beyond doubt, and when in doubt, do nothing. Filing \
                 the report is the whole action: you never ban, kick, or reply to the \
                 joiner, and a join you do not report needs no comment."
            ),
            "the whole join block stands, byte for byte"
        );
        let taught = composed_system_prompt("b", "n", AnsweringMode::Helpful, moderating());
        assert!(
            taught.contains("report the join on sight"),
            "the taught deployment reads the join rule"
        );
    }

    /// Every capability shape a composition can take, so a claim about
    /// "the teaching" is made against all of them and not one.
    fn every_capabilities() -> [Capabilities; 4] {
        [
            Capabilities::default(),
            moderating(),
            searching(),
            Capabilities {
                moderation_handle: true,
                web_search: true,
            },
        ]
    }

    /// AC5's first half (unit 54): the teaching PUSHES NOTHING. Neither
    /// turn-ending tool's name appears in any teaching constant, in either
    /// mode, under any configuration — the tools are a protection that
    /// works by existing, and what each is for lives in its own
    /// model-facing description. The taught default is unchanged: a turn
    /// with nothing to say ends with no text.
    #[test]
    fn no_teaching_constant_names_a_turn_ending_tool() {
        let tools = [
            crate::tools::no_reply_needed::NAME,
            crate::tools::work_is_done::NAME,
        ];
        for constant in [
            REACT_TEACHING,
            CLOSING_PROHIBITIONS,
            MODERATION_TEACHING,
            SEARCH_TEACHING,
            REACTION_CARVE_OUT,
            SILENCE_IN_WORDS,
        ] {
            for tool in tools {
                assert!(
                    !constant.contains(tool),
                    "no teaching constant names {tool}: {constant}"
                );
            }
        }
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for capabilities in every_capabilities() {
                let prompt = composed_system_prompt("b", "n", mode, capabilities);
                for tool in tools {
                    assert!(
                        !prompt.contains(tool),
                        "the {mode:?} composition under {capabilities:?} names {tool}"
                    );
                }
                assert!(
                    prompt.contains(
                        "end your turn without sending anything — no placeholder message"
                    ),
                    "the taught default stays the empty turn in {mode:?} mode"
                );
            }
        }
    }

    /// AC5's second half (unit 54), as unit 55 leaves it: one closing
    /// prohibition composes in both answering modes and under every
    /// configuration, byte for byte — the never-announce sentence. The
    /// bare-call sentence went with the mechanism it warned about, and this
    /// asserts its absence beside the survivor's presence.
    #[test]
    fn both_modes_carry_the_closing_prohibition() {
        assert_eq!(
            CLOSING_PROHIBITIONS,
            "Never send a message whose only content is that you are not taking part: a \
             line saying the question was for someone else, that you are staying out of \
             it, or that you have nothing to add is your silence written out, and nobody \
             needs to read it."
        );
        assert!(
            !CLOSING_PROHIBITIONS.contains("posted to the group as its own message"),
            "the bare-call prohibition is gone with the mechanism it warned about: from \
             unit 55 nothing written ahead of a call is posted anywhere"
        );
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for capabilities in every_capabilities() {
                assert!(
                    composed_system_prompt("b", "n", mode, capabilities)
                        .contains(CLOSING_PROHIBITIONS),
                    "the prohibitions compose in {mode:?} mode under {capabilities:?}"
                );
            }
        }
    }

    /// AC15 (unit 55): the speaking contract composes in every mode under
    /// every configuration, byte for byte, and it states each of the four
    /// things it exists to state — the private text, the two doors, what
    /// one turn may do, and the envelope — beside the silence default and
    /// the two tool effects that are not the model's text.
    #[test]
    fn every_composition_carries_the_speaking_contract() {
        assert_eq!(
            SENDING_CONTRACT,
            "What you write is your own private notes. It is never posted and nobody in \
             the group reads it. A message reaches the group only when you send it: with \
             the send_message tool to post to the chat, or with the reply_message tool \
             to answer one message in particular, naming it by the msgid its envelope \
             shows. One turn of yours can send several messages, answer several people, \
             or send nothing at all. Silence stays the default: a turn that ends without \
             sending posts nothing, which is exactly right whenever there is nothing to \
             say. Every message you read carries an envelope above it naming who wrote \
             it, when the chat says it was sent, and its msgid. Two things that are not \
             messages still reach the group as they always did, because they are a \
             tool's own effect and not your text: a report you file, and an emoji \
             reaction you place."
        );
        for named in [crate::tools::send::NAME, crate::tools::reply::NAME] {
            assert!(
                SENDING_CONTRACT.contains(named),
                "the contract names the tool a message reaches the group through: {named}"
            );
        }
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for capabilities in every_capabilities() {
                let prompt = composed_system_prompt("b", "n", mode, capabilities);
                assert!(
                    prompt.contains(SENDING_CONTRACT),
                    "the contract composes in {mode:?} mode under {capabilities:?}"
                );
            }
        }
    }

    /// AC15's other half (unit 55): no sentence anywhere in the composed
    /// prompt presupposes relayed text.
    ///
    /// The removed wordings are asserted GONE — the silence sentences that
    /// spoke of writing, the announce line that said "say", and the
    /// bare-call prohibition — and the rewritten ones present. Every
    /// mode and every configuration is read, because a sentence surviving
    /// in one arm is a sentence surviving.
    #[test]
    fn no_composed_sentence_presupposes_relayed_text() {
        for mode in [AnsweringMode::Helpful, AnsweringMode::Addressed] {
            for capabilities in every_capabilities() {
                let prompt = composed_system_prompt("b", "n", mode, capabilities);
                for gone in [
                    "end your turn without writing any text",
                    "end the turn with no text",
                    "Ending a turn with no text",
                    "whatever you write before a tool call is posted to the group",
                    "Before you run a search, say in one short line",
                    "bracketed id",
                    "shown in brackets",
                ] {
                    assert!(
                        !prompt.contains(gone),
                        "the {mode:?} composition under {capabilities:?} still carries a \
                         sentence that presupposes relayed text: {gone}"
                    );
                }
                for present in [
                    "end your turn without sending anything — no placeholder message",
                    "end the turn without sending",
                    "What you write is your own private notes",
                ] {
                    assert!(
                        prompt.contains(present),
                        "the {mode:?} composition under {capabilities:?} carries: {present}"
                    );
                }
            }
        }
        // The never-announce sentence stays, and it is the whole of what
        // the closing prohibition is now.
        let helpful =
            composed_system_prompt("b", "n", AnsweringMode::Helpful, Capabilities::default());
        assert!(
            helpful.contains(
                "Never send a message whose only content is that you are not taking part"
            ),
            "the never-announce sentence stays"
        );
        // The search teaching's heads-up is SENT, and only where the tool
        // is admitted.
        assert!(
            composed_system_prompt("b", "n", AnsweringMode::Helpful, searching()).contains(
                "send one short line with send_message saying what you are about to look up"
            ),
            "the heads-up before slow work is a send now"
        );
    }

    /// AC5's third half (unit 54): the search teaching and the sourcing
    /// paragraph stand BYTE-UNCHANGED. This unit reworded the reaction
    /// trigger and added two prohibitions; a reworded search or sourcing
    /// sentence would be a change nobody decided, and the two literals
    /// here are what says so.
    #[test]
    fn the_search_and_sourcing_sentences_stand_unchanged() {
        assert_eq!(
            SEARCH_TEACHING,
            "You can also search the web with the search_web tool, for questions about \
             the world and not about the project. Before you run a search, send one \
             short line with send_message saying what you are about to look up, then run \
             the search, then send the answer: one line and no more, stating the thing \
             you are going to look for — never a placeholder standing in for an answer, \
             and never a restatement of the words the member just wrote. Ending a turn \
             without sending is for a turn with nothing to say; a turn with a search to \
             run has something to say. A \
             result's snippet is a hint, not a source: when you answer from one, say \
             where it came from and name the page. A snippet that does not contain the \
             claim is a miss, exactly as an unanswering lookup is — say you don't know \
             instead of filling the gap from memory. Facts about the project itself \
             still come only from the project lookups: a web result is never the source \
             for a claim about halogenOS, its features, its procedures or its builds."
        );
        assert_eq!(
            sourcing_rules(),
            "Your lookup tools are the only source of substantive claims: any claim \
             about the project — a feature, a procedure, a project fact — must come from \
             a lookup you made in this turn, never from your trained knowledge, so look \
             it up before you answer. A lookup answers a question only when its result \
             actually contains the answer: a result that is empty, off-topic, or missing \
             the specific claim is a miss, not a licence to fill the gap from memory. \
             Never guess and never answer from hedged memory — no \"as far as I know\", \
             no \"probably\" — and in a compound answer, every project-specific claim is \
             either confirmed by a lookup or dropped. When you were addressed and a \
             lookup cannot back the answer, say you don't know, plainly and in your own \
             words — never guess from memory or offer a hedged recollection. When you \
             were not addressed and have nothing to add, end the turn without sending."
        );
    }
}
