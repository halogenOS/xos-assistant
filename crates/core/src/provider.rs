//! The in-memory provider registration: the framework's `OpenRouter` module
//! with its configuration resident in process memory instead of the store.
//!
//! The framework's `OpenRouter` module persists provider configuration — the API
//! key included — into the store. The assistant's secrets rule forbids that:
//! the store file is long-lived, backed up, and outlives any key rotation.
//! This wrapper changes only where the configuration lives. The wire, the
//! request shapes and the streaming contract all stay the framework's: every
//! model-facing call delegates to the wrapped module, and the persistence
//! hooks are inert, so nothing ever writes the key. Surfaced to the
//! framework's improvements list: an in-memory provider-configuration seam,
//! so consumers stop needing wrappers like this one.

use agent_ledger::providers::openrouter::OpenRouterModule;
use agent_ledger::providers::{BoxFuture, mask_secret};
use agent_ledger::{
    LlmError, ProviderModule, Store, StoreError,
    providers::{ModelInfo, ProviderRx, ProviderTx},
};
use serde_json::{Value, json};

/// The `OpenRouter` module with its configuration held in memory. Registered
/// in place of the wrapped module, under the same type id.
///
/// No `Debug` implementation on purpose: the held configuration carries the
/// API key, and a derived representation would print it.
pub struct MemoryConfiguredProvider {
    inner: OpenRouterModule,
    config: Value,
}

impl MemoryConfiguredProvider {
    /// Wrap the framework's `OpenRouter` module around an in-memory
    /// configuration: the key, and the base URL where a test points it at a
    /// loopback server (`None` keeps the module's real host).
    ///
    /// The wrapped module creates its configuration table on construction;
    /// the table stays empty for the process's whole life, because the save
    /// hook below never writes.
    ///
    /// # Panics
    ///
    /// If the wrapped module's configuration table cannot be created — a
    /// broken store, not a runtime condition.
    pub async fn new(store: &Store, api_key: String, base_url: Option<String>) -> Self {
        let inner = OpenRouterModule::new(store.tx()).await;
        let mut config = json!({ "api_key": api_key });
        if let Some(base_url) = base_url {
            config["base_url"] = json!(base_url);
        }
        Self { inner, config }
    }
}

impl ProviderModule for MemoryConfiguredProvider {
    fn type_id(&self) -> &'static str {
        self.inner.type_id()
    }

    fn display_name(&self) -> &'static str {
        self.inner.display_name()
    }

    fn description(&self) -> &'static str {
        self.inner.description()
    }

    /// Every instance reads the one in-memory configuration. The assembly
    /// registers a single instance of this type, so there is no second
    /// instance a broader answer could mislead.
    fn get_config(&self, _provider_id: String) -> BoxFuture<'_, Result<Option<Value>, StoreError>> {
        Box::pin(async move { Ok(Some(self.config.clone())) })
    }

    /// Inert: the configuration's residence is process memory, and a write
    /// here is precisely what this wrapper exists to prevent. Succeeds so a
    /// caller running a save-back cycle — the registry's startup pass does —
    /// stays on its happy path while nothing reaches the store.
    fn save_config(
        &self,
        _provider_id: String,
        _config: Value,
    ) -> BoxFuture<'_, Result<(), StoreError>> {
        Box::pin(async { Ok(()) })
    }

    /// Inert, like the save: there is nothing stored to delete.
    fn delete_config(&self, _provider_id: String) -> BoxFuture<'_, Result<(), StoreError>> {
        Box::pin(async { Ok(()) })
    }

    fn summary(&self, _provider_id: String) -> BoxFuture<'_, Result<Option<String>, StoreError>> {
        Box::pin(async move { Ok(self.config["api_key"].as_str().map(mask_secret)) })
    }

    /// The wire itself: delegated whole. The stored-config argument the
    /// runtime read through [`Self::get_config`] is already the in-memory
    /// one, and it is passed on untouched.
    fn bind(
        &self,
        conversation_id: i64,
        provider_id: String,
        config: Value,
    ) -> (ProviderTx, ProviderRx) {
        self.inner.bind(conversation_id, provider_id, config)
    }

    fn list_models(&self, config: Value) -> BoxFuture<'_, Result<Vec<ModelInfo>, LlmError>> {
        self.inner.list_models(config)
    }
}
