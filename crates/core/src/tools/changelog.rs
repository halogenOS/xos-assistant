//! The harness-changelog tool (unit 47, 2026-08-30): what changed in this
//! assistant's own software since the previously deployed version,
//! answered from a text embedded in the build.
//!
//! Asked what changed or what is new in it, a model answers from its
//! training data and from whatever the chat said earlier — both wrong the
//! moment the deployment moves, and both invention about the very software
//! doing the answering. The text here comes from the build instead: the
//! whole since-last-version changelog, one result, verbatim. It exists for
//! the same reason the runtime facts do ([`crate::tools::runtime`], unit
//! 32): the question is answered from the record, never from memory.
//!
//! The changelog arrives as a compile-time environment value the build
//! passes in (`ASSISTANT_BUILD_CHANGELOG`), on the same rail the build
//! revision rides. The deployment builds from a source tree carrying no
//! version-control metadata: nothing at compile time and nothing at run
//! time could read a history there. The deployment repository generates
//! the text — per commit in the deployed range, the date and time, the
//! commit title and the full commit message — and no code in this
//! repository performs that generation. Browsing the whole git history is
//! a later, deliberately unbuilt tool; this one embeds (decision 0159).
//!
//! A build that passes no changelog answers a stated absence: the result
//! says plainly that this build carries no changelog and instructs the
//! model to say so instead of recalling or inventing one — the same
//! register the runtime facts decline in. An empty passed value is the
//! same absence: the revision can skip its empty check because a revision
//! is a fixed-width hash, while a generated changelog over a range with
//! nothing in it is an empty string a deployment can really pass, and an
//! empty result would hand the model nothing to answer from — exactly the
//! opening invention this tool exists to close.
//!
//! One result, the whole text: no pagination, no per-entry structure, no
//! filtering. The operator decided one result containing the entire
//! since-last-version text, and a present changelog is returned verbatim,
//! byte for byte as the build embedded it.
//!
//! This is the changelog of the ASSISTANT SOFTWARE — the harness the
//! model runs as — never of the halogenOS operating system. The name and
//! the description both carry that distinction, because a model handed a
//! plain "changelog" tool would spend it on the question the group asks
//! far more often: what changed in the ROM. Release questions stay with
//! the release lookup ([`crate::tools::release`]).
//!
//! No parameters, and no input is read: there is nothing to select in one
//! whole text. No git, no network, no filesystem read — the entire fact
//! is one value compiled into the build, exactly as old as the binary
//! that answers with it and refreshed the way the binary is: by the next
//! build that passes a new value.

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::json;

use crate::message::Authority;

/// The registered name the model calls the tool by. `harness`, not the
/// operating system's name: the word keeps this tool off the halogenOS
/// changelog, which is a different tool's business.
pub const NAME: &str = "harness_changelog";

/// The authority this tool requires — member: what changed in the
/// assistant is a fact any group member may ask about, the same reasoning
/// recorded on the runtime facts (unit 32).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The stated absence a build that passed no changelog answers with. The
/// tool did not fail: the honest fact about such a build is that it
/// carries no changelog, and the result says so and directs the model to
/// answer the same way — never a summary remembered from nowhere.
pub const ABSENT_RESULT: &str = "This build carries no changelog for the assistant. \
     Say so plainly, and state nothing about what changed in it — never recall or \
     invent a changelog.";

/// The changelog embedded in this build, resolved once at compile time:
/// the whole since-last-version text, or the stated absence.
pub const CHANGELOG: &str = resolve_changelog(option_env!("ASSISTANT_BUILD_CHANGELOG"));

/// The one place an absent build changelog becomes the stated absence.
const fn resolve_changelog(passed: Option<&'static str>) -> &'static str {
    match passed {
        Some(changelog) if !changelog.is_empty() => changelog,
        _ => ABSENT_RESULT,
    }
}

/// The harness-changelog tool. Constructed by the assembly, which admits
/// it unconditionally beside the runtime facts — the reasoning is
/// recorded once, on the assembly's admit site and in decision 0159. It
/// holds nothing: the whole fact is one value the process holds.
pub(crate) struct HarnessChangelog;

impl HarnessChangelog {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ToolHandler<CoreEvent> for HarnessChangelog {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "State what changed in this assistant's own software since \
                 the previously deployed version: the whole changelog of the assistant \
                 itself, one text, as the build embedded it. This is the changelog of \
                 the assistant software, not of the halogenOS operating system: \
                 questions about halogenOS releases, builds or version changes belong \
                 to the release lookup, not here. Call it whenever someone asks what \
                 changed, what is new, or what was updated in you or in the assistant — \
                 the answer comes from the changelog embedded in this build, never \
                 from what you remember and never from what the conversation said \
                 earlier. It takes no arguments."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move { ToolOutcome::Done(CHANGELOG.to_owned()) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;
    use crate::schema::store_config;

    /// The changelog resolution, over the injected shapes a build can
    /// pass: a present changelog stands verbatim, and an absent or empty
    /// one is the stated absence. The compiled-in constant is the same
    /// resolution over the real build value, so it is never empty.
    #[test]
    fn a_present_changelog_stands_and_an_absent_one_states_its_absence() {
        let changelog = "2026-08-30 09:41 +02:00 — core: the changelog tool\n\n\
             The assistant answers what-changed questions from the build now.\n\n\
             2026-08-30 08:12 +02:00 — core: the runtime facts\n\n\
             The model, version and uptime come from the process.";
        assert_eq!(resolve_changelog(Some(changelog)), changelog);
        assert_eq!(resolve_changelog(None), ABSENT_RESULT);
        assert_eq!(
            resolve_changelog(Some("")),
            ABSENT_RESULT,
            "an empty passed changelog is no changelog: the stated absence, never an \
             empty result the model would fill from memory"
        );
        assert!(
            !CHANGELOG.is_empty(),
            "the compiled changelog is the text or the stated absence, both non-empty"
        );
    }

    /// The stated absence, byte-pinned where it is defined. The one
    /// assertion carries the whole claim: the plain fact that this build
    /// has no changelog, and the instruction to answer honestly instead
    /// of recalling or inventing one.
    #[test]
    fn the_stated_absence_tells_the_model_to_say_so() {
        assert_eq!(
            ABSENT_RESULT,
            "This build carries no changelog for the assistant. Say so plainly, and \
             state nothing about what changed in it — never recall or invent a \
             changelog."
        );
    }

    /// The description keeps the operating system's changelog out: it
    /// names the assistant software as the subject, says the halogenOS
    /// changelog is not this tool's, and points release questions at the
    /// release lookup. A model that reads it cannot spend the tool on the
    /// ROM's changes.
    #[test]
    fn the_description_keeps_the_os_changelog_out() {
        let definition = HarnessChangelog::new().definition();
        assert_eq!(definition.name, NAME);
        for phrase in [
            "the whole changelog of the assistant itself",
            "not of the halogenOS operating system",
            "belong to the release lookup",
            "never from what you remember",
            "takes no arguments",
        ] {
            assert!(
                definition.description.contains(phrase),
                "the description carries: {phrase}"
            );
        }
        assert_eq!(definition.parameters["properties"], json!({}));
    }

    /// The authority is member, pinned where it is defined: what changed
    /// in the assistant is a fact of the group, asked by ordinary members.
    #[test]
    fn the_required_authority_is_member() {
        assert_eq!(REQUIRED_AUTHORITY, Authority::Member);
    }

    /// One call of the tool, with a block id naming nothing: the execute
    /// path reads nothing, so the context exists only to carry the call.
    async fn call(
        tool: &HarnessChangelog,
        agency: &AgencyCtx<CoreEvent>,
        input: &str,
    ) -> ToolOutcome {
        tool.execute(
            input,
            ToolContext {
                agency,
                tool_call_id: "call-0",
                block_id: 12345,
            },
        )
        .await
    }

    /// Every input — an empty one, an object with strange fields,
    /// unparsable text — answers the same one result: the compiled value,
    /// whole and verbatim. In this suite's builds no changelog is passed,
    /// so that result is the stated absence byte for byte; a build that
    /// passes one answers it just as wholly.
    #[tokio::test]
    async fn every_input_answers_the_embedded_value_whole() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let agency: AgencyCtx<CoreEvent> = AgencyCtx {
            conversation_id: 404,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = HarnessChangelog::new();
        for input in ["", "{}", r#"{"since":"yesterday"}"#, "not json"] {
            match call(&tool, &agency, input).await {
                ToolOutcome::Done(result) => assert_eq!(
                    result, CHANGELOG,
                    "the input {input:?} answers the embedded value and nothing else"
                ),
                ToolOutcome::Error(error) => {
                    panic!("the input {input:?} answers, it does not decline: {error}")
                }
                ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                    panic!("the tool resolves its own call")
                }
            }
        }
        if option_env!("ASSISTANT_BUILD_CHANGELOG").is_none() {
            assert_eq!(
                CHANGELOG, ABSENT_RESULT,
                "a build that passed no changelog answers the stated absence"
            );
        }
    }
}
