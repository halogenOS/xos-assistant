//! The assistant's tools: the project lookups and everything else the model
//! can reach for.
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
//! The assembly takes a [`ToolSet`], registers every handler in it, and
//! records the framework's tool choice naming exactly those tools on every
//! conversation it creates. WHICH tools a conversation has is that recorded
//! choice, read by the framework at the dispatch that decides what a turn is
//! offered and at the runner that decides what a call resolves against.
//!
//! Required authority is enforced at the call, per decision 0043: every tool
//! answers the framework's admission hook with
//! [`admission::at_required_authority`], which reads the turn's provenance
//! through the call block's dispatch anchor — the `provenance` module holds
//! the reading — and declines when it falls below the bar the tool's own
//! module declares. Registration accepts any authority; the check at the
//! call is the whole enforcement.

use agent_ledger::{CoreEvent, ToolHandler, ToolRegistry};

pub mod admission;
pub mod changelog;
pub mod commit;
pub(crate) mod lookup;
pub mod mark;
pub(crate) mod provenance;
pub mod release;
pub mod report;
pub mod rights;
pub mod runtime;
pub mod search;
pub mod standing;
pub mod wiki;

use commit::CommitLookup;
use release::ReleaseLookup;
use wiki::WikiLookup;

/// The tools the assembly registers. The set is built by the embedder — the
/// binary admits the production lookups, a test may admit probes of its own
/// — and the assembly derives both the registry and the recorded tool choice
/// from it, so the two cannot name different tools.
///
/// A handler admitted here is registered at once, so a name is claimed where
/// it is admitted and the two derived views can never be built from
/// different sets.
pub struct ToolSet {
    registry: ToolRegistry<CoreEvent>,
    names: Vec<String>,
}

impl Default for ToolSet {
    fn default() -> Self {
        Self {
            registry: ToolRegistry::new(),
            names: Vec::new(),
        }
    }
}

/// What the production lookups are pointed at: one named base per host,
/// plus the mirror's optional credential. The bases are interchangeable
/// strings at the type level, so they travel under their names: a call
/// site reads what it sets, and two swapped hosts are visible where they
/// are written instead of compiling silently into production traffic at
/// the wrong address.
pub struct LookupEndpoints {
    /// The forge base URL the commit lookup queries.
    pub forge: String,
    /// The mirror base URL the release lookup queries.
    pub mirror: String,
    /// The mirror's optional bearer token, absent for anonymous reads.
    pub mirror_token: Option<String>,
    /// The raw wiki base URL the wiki lookup reads page content from.
    pub wiki: String,
    /// The forge base URL the wiki lookup's page enumeration reads the
    /// rendered wiki index from — a second host on purpose: the raw host
    /// serves pages but no index.
    pub wiki_index: String,
}

impl ToolSet {
    /// An empty set: nothing registered, every recorded choice written
    /// empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The production set — the three lookups at their required
    /// authorities and default timeouts, against the given endpoints. One
    /// answer to which lookups ship: the binary points it at the
    /// configured hosts, and the suites' default fixtures point the same
    /// set at a loopback address nothing listens on. The report tool is
    /// not a lookup and joins at the assembly, where its window and the
    /// erasure fence live.
    #[must_use]
    pub fn production_lookups(endpoints: LookupEndpoints) -> Self {
        let mut set = Self::new();
        set.admit(CommitLookup::new(endpoints.forge, commit::DEFAULT_TIMEOUT));
        set.admit(ReleaseLookup::new(
            endpoints.mirror,
            endpoints.mirror_token,
            release::DEFAULT_TIMEOUT,
        ));
        set.admit(WikiLookup::new(
            endpoints.wiki,
            endpoints.wiki_index,
            wiki::DEFAULT_TIMEOUT,
        ));
        set
    }

    /// Admit one tool. The registered name is the definition's own, and the
    /// authority a call of it requires is the tool's own answer to the
    /// framework's admission hook — stated in the module that owns the tool
    /// and nowhere else. Registration refuses nothing on authority; the
    /// check at the call is the whole enforcement (decision 0043).
    ///
    /// # Panics
    ///
    /// If two admitted tools share one name; the registry refuses the
    /// silent overwrite.
    pub fn admit(&mut self, handler: impl ToolHandler<CoreEvent> + 'static) {
        let name = handler.definition().name;
        self.names.push(name.clone());
        self.registry.register(name, handler);
    }

    /// The registry the runtime resolves calls against, and the tool names
    /// every created conversation records as its choice — one source, two
    /// readers.
    pub(crate) fn into_registry(mut self) -> (ToolRegistry<CoreEvent>, Vec<String>) {
        // The choice records the names in sorted order — the same order the
        // registry iterates, deterministic across registration orders.
        self.names.sort_unstable();
        (self.registry, self.names)
    }
}

#[cfg(test)]
mod tests {
    use agent_ledger::providers::{BoxFuture, ToolDefinition};
    use agent_ledger::{ToolContext, ToolOutcome};

    use super::*;
    use crate::message::Authority;

    /// A named no-op handler for the registration pins, answering the
    /// admission hook at the authority it was built with.
    struct Named(&'static str, Authority);

    impl agent_ledger::ToolHandler<CoreEvent> for Named {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.0.into(),
                description: "a probe".into(),
                parameters: serde_json::json!({ "type": "object" }),
            }
        }
        fn admit<'a>(
            &'a self,
            ctx: &'a ToolContext<'a, CoreEvent>,
            ledger: &'a [agent_ledger::Block],
        ) -> BoxFuture<'a, agent_ledger::Admission> {
            admission::at_required_authority(self.0, self.1, ctx, ledger)
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
        assert_eq!(wiki::DEFAULT_BASE_URL, "https://raw.githubusercontent.com");
        assert_eq!(wiki::DEFAULT_INDEX_BASE_URL, "https://github.com");
    }

    #[test]
    fn the_production_set_yields_the_three_lookup_names() {
        // The one shared answer to which lookups ship, pinned where it is
        // defined: the fixtures and the binary both build this set.
        let set = ToolSet::production_lookups(LookupEndpoints {
            forge: "http://127.0.0.1:1".into(),
            mirror: "http://127.0.0.1:1".into(),
            mirror_token: None,
            wiki: "http://127.0.0.1:1".into(),
            wiki_index: "http://127.0.0.1:1".into(),
        });
        let (_, names) = set.into_registry();
        assert_eq!(
            names,
            vec![
                commit::NAME.to_owned(),
                release::NAME.to_owned(),
                wiki::NAME.to_owned()
            ]
        );
    }

    #[test]
    fn the_tool_set_yields_sorted_choice_names() {
        // The names come back sorted no matter the admission order — the
        // registry's own iteration order, so the recorded choice and the
        // schema list agree.
        let mut set = ToolSet::new();
        set.admit(Named("zulu", Authority::Member));
        set.admit(Named("alpha", Authority::Member));
        let (registry, names) = set.into_registry();
        assert_eq!(names, vec!["alpha".to_owned(), "zulu".to_owned()]);
        assert_eq!(
            registry.names().collect::<Vec<_>>(),
            vec!["alpha", "zulu"],
            "the registry iterates the same order the choice records"
        );
    }

    #[test]
    fn a_registration_above_member_reaches_the_registry_and_the_choice() {
        // Registration accepts every authority (decision 0043): an
        // admin-level registration reaches both derived views, because
        // enforcement is each tool's own answer to the admission hook,
        // which every call passes through.
        let mut set = ToolSet::new();
        set.admit(Named("admin_probe", Authority::Admin));
        set.admit(Named("moderator_probe", Authority::Moderator));
        set.admit(Named("member_probe", Authority::Member));
        let (registry, names) = set.into_registry();
        assert_eq!(
            names,
            vec![
                "admin_probe".to_owned(),
                "member_probe".to_owned(),
                "moderator_probe".to_owned()
            ],
            "every authority registers and the choice names all three"
        );
        assert_eq!(
            registry.names().collect::<Vec<_>>(),
            vec!["admin_probe", "member_probe", "moderator_probe"],
            "the registry holds all three registrations"
        );
    }
}
