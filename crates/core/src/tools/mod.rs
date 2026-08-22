//! The assistant's tools: the project lookups, the palette that gates their
//! admission, and the wrapper that enforces it.
//!
//! Tools are product behavior and live here, in the core: the project's own
//! forge and releases are the product, and the no-platform-vocabulary
//! invariant bans chat-platform vocabulary, not the product's. Each tool is
//! one module — name, model-facing description, parameter schema, required
//! authority, and an execute performing one bounded HTTP GET against its
//! configured base URL. What the lookups share — the bounded-GET contract,
//! the decode helpers, the path-safety checks — lives in the `lookup`
//! module; this module holds the tree and the [`ToolSet`].
//!
//! The assembly takes a [`ToolSet`] — each handler admitted at its required
//! authority — wraps every one in the admission check (`AdmittedTool`), and
//! writes the [`palette::ToolPalette`] block naming exactly those tools at
//! every conversation's creation. A conversation without a palette admits
//! nothing.
//!
//! Required authority is enforced at registration, not at the call:
//! [`ToolSet::admit`] refuses any tool whose required authority is above
//! member — the structural floor of decision 0043's closure (2026-08-22),
//! standing in until the framework's dispatch anchor records the turn's
//! summoning frontier onto the tool call at insert. A refused tool never
//! reaches the registry or the palette, so nothing above the floor is
//! expressible at runtime.

use agent_ledger::{CoreEvent, ToolHandler, ToolRegistry};

use crate::message::Authority;

pub(crate) mod admission;
pub mod commit;
pub(crate) mod lookup;
pub mod palette;
pub mod release;

use admission::AdmittedTool;
use commit::CommitLookup;
use release::ReleaseLookup;

/// The refusal [`ToolSet::admit`] answers a tool whose required authority is
/// above member: the registration floor of decision 0043's closure. The
/// floor exists because no stored shape can currently answer whose authority
/// summoned a turn — the framework's dispatch anchor is the mechanism that
/// will, and until it ships, a tool needing more than member authority is
/// refused at the door instead of read from stored shape at the call.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the tool '{tool}' names {} as its required authority, above the member \
     floor: registration refuses every tool above member until the \
     framework's dispatch anchor ships (decision 0043, closed 2026-08-22)",
    required.as_str()
)]
pub struct RegistrationAboveFloor {
    /// The refused tool's registered name.
    pub tool: String,
    /// The authority the tool named, above the floor.
    pub required: Authority,
}

/// The tools the assembly registers, each admitted at the authority its
/// admission requires. The set is built by the embedder — the binary admits
/// the two production lookups, a test may admit probes of its own — and the
/// assembly derives both the registry and the palette list from it, so the
/// two cannot name different tools.
#[derive(Default)]
pub struct ToolSet {
    entries: Vec<AdmittedTool>,
}

impl ToolSet {
    /// An empty set: nothing registered, every palette written empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The production set — the two lookups at their required authorities
    /// and default timeouts, against the given base URLs. One answer to
    /// which tools ship: the binary points it at the configured hosts, and
    /// the suites' default fixtures point the same set at a loopback
    /// address nothing listens on.
    ///
    /// # Panics
    ///
    /// Never: both lookups sit at the member floor, and the pin on the
    /// production names covers this construction.
    #[must_use]
    pub fn production_lookups(
        forge_base_url: impl Into<String>,
        mirror_base_url: impl Into<String>,
        mirror_token: Option<String>,
    ) -> Self {
        let mut set = Self::new();
        set.admit(
            commit::REQUIRED_AUTHORITY,
            CommitLookup::new(forge_base_url, commit::DEFAULT_TIMEOUT),
        )
        .expect("the commit lookup sits at the member floor");
        set.admit(
            release::REQUIRED_AUTHORITY,
            ReleaseLookup::new(mirror_base_url, mirror_token, release::DEFAULT_TIMEOUT),
        )
        .expect("the release lookup sits at the member floor");
        set
    }

    /// Admit one tool at the given required authority. The registered name
    /// is the definition's own.
    ///
    /// # Errors
    ///
    /// [`RegistrationAboveFloor`] for any authority above member — the
    /// structural floor of decision 0043's closure. The refused tool is not
    /// held anywhere: it reaches neither the registry nor the palette.
    pub fn admit(
        &mut self,
        required: Authority,
        handler: impl ToolHandler<CoreEvent> + 'static,
    ) -> Result<(), RegistrationAboveFloor> {
        if required > Authority::Member {
            return Err(RegistrationAboveFloor {
                tool: handler.definition().name,
                required,
            });
        }
        self.entries.push(AdmittedTool::new(handler));
        Ok(())
    }

    /// The registry the runtime resolves calls against, and the palette list
    /// every created conversation records — one source, two readers.
    ///
    /// # Panics
    ///
    /// If two admitted tools share one name; the registry refuses the
    /// silent overwrite.
    pub(crate) fn into_registry(self) -> (ToolRegistry<CoreEvent>, Vec<String>) {
        let mut registry = ToolRegistry::new();
        let mut names: Vec<String> = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            names.push(entry.name().to_owned());
            registry.register(names.last().expect("just pushed").clone(), entry);
        }
        // The palette records the names in sorted order — the same order the
        // registry iterates, deterministic across registration orders.
        names.sort_unstable();
        (registry, names)
    }
}

#[cfg(test)]
mod tests {
    use agent_ledger::providers::{BoxFuture, ToolDefinition};
    use agent_ledger::{ToolContext, ToolOutcome};

    use super::*;

    /// A named no-op handler for the registration pins.
    struct Named(&'static str);

    impl agent_ledger::ToolHandler<CoreEvent> for Named {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.0.into(),
                description: "a probe".into(),
                parameters: serde_json::json!({ "type": "object" }),
            }
        }
        fn execute<'a>(
            &'a self,
            _input: &'a str,
            _ctx: ToolContext<'a, CoreEvent>,
        ) -> BoxFuture<'a, ToolOutcome> {
            Box::pin(async { ToolOutcome::Done("ok".into()) })
        }
    }

    #[test]
    fn the_default_base_urls_name_the_real_hosts() {
        // The values the binary falls back to when the configuration names
        // no override — pinned so an accidental edit fails loudly instead
        // of silently pointing production at the wrong host.
        assert_eq!(commit::DEFAULT_BASE_URL, "https://git.halogenos.org");
        assert_eq!(release::DEFAULT_BASE_URL, "https://api.github.com");
    }

    #[test]
    fn the_production_set_yields_the_two_lookup_names() {
        // The one shared answer to which tools ship, pinned where it is
        // defined: the fixtures and the binary both build this set.
        let set = ToolSet::production_lookups("http://127.0.0.1:1", "http://127.0.0.1:1", None);
        let (_, names) = set.into_registry();
        assert_eq!(
            names,
            vec![commit::NAME.to_owned(), release::NAME.to_owned()]
        );
    }

    #[test]
    fn the_tool_set_yields_sorted_palette_names() {
        // The names come back sorted no matter the admission order — the
        // registry's own iteration order, so the palette and the schema
        // list agree.
        let mut set = ToolSet::new();
        set.admit(Authority::Member, Named("zulu"))
            .expect("a member-level probe registers");
        set.admit(Authority::Member, Named("alpha"))
            .expect("a member-level probe registers");
        let (registry, names) = set.into_registry();
        assert_eq!(names, vec!["alpha".to_owned(), "zulu".to_owned()]);
        assert_eq!(
            registry.names().collect::<Vec<_>>(),
            vec!["alpha", "zulu"],
            "the registry iterates the same order the palette records"
        );
    }

    #[test]
    fn a_registration_above_the_member_floor_is_refused_and_never_reaches_the_registry() {
        // The structural floor of decision 0043's closure (AC5): an
        // admin-level registration is refused with the typed error, the
        // refusal names the floor and the closure, and the refused tool is
        // provably absent from the registry and the palette list alike.
        let mut set = ToolSet::new();
        let refusal = set
            .admit(Authority::Admin, Named("above_the_floor"))
            .expect_err("an admin-level registration is refused");
        assert_eq!(
            refusal,
            RegistrationAboveFloor {
                tool: "above_the_floor".to_owned(),
                required: Authority::Admin,
            }
        );
        let worded = refusal.to_string();
        assert!(
            worded.contains("above the member floor"),
            "the refusal names the floor: {worded}"
        );
        assert!(
            worded.contains("decision 0043, closed 2026-08-22"),
            "the refusal names the closure it stands on: {worded}"
        );

        set.admit(Authority::Moderator, Named("also_above"))
            .expect_err("a moderator-level registration is refused too");
        set.admit(Authority::Member, Named("at_the_floor"))
            .expect("a member-level tool registers");

        let (registry, names) = set.into_registry();
        assert_eq!(
            names,
            vec!["at_the_floor".to_owned()],
            "the palette list carries only the admitted tool"
        );
        assert_eq!(
            registry.names().collect::<Vec<_>>(),
            vec!["at_the_floor"],
            "the refused registrations never reached the registry"
        );
    }
}
