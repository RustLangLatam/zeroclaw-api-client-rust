//! Configuration: per-property reads and writes, atomic JSON Patch batches,
//! map aliases (agents, providers, channels…), drift, catalogs and the
//! quickstart sections.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api_config.rs` and
//! `api_sections.rs`. Property paths are dotted
//! (`providers.models.anthropic.default.api_key`). Reads return values as the
//! gateway's display strings; writes take typed JSON, which the gateway
//! coerces to the field's kind. Secrets are never returned: their reads say
//! only whether they are populated.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, query};
use crate::error::Result;

/// Header that lets a [`ConfigApi::patch`] overwrite paths changed on disk
/// since the gateway loaded them.
const OVERRIDE_DRIFT: &str = "X-ZeroClaw-Override-Drift";

/// Configuration endpoints, from [`Client::config`].
#[derive(Debug, Clone, Copy)]
pub struct ConfigApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Configuration endpoints (`/api/config/...`).
    pub fn config(&self) -> ConfigApi<'_> {
        ConfigApi { client: self }
    }
}

impl ConfigApi<'_> {
    /// `GET /api/config`: the whole configuration with secrets masked as
    /// `***MASKED***`. A compatibility snapshot; prefer [`ConfigApi::list`]
    /// and [`ConfigApi::get`], which carry field metadata.
    pub async fn snapshot(&self) -> Result<Value> {
        self.client.get(&["api", "config"], &[]).await
    }

    /// `OPTIONS /api/config`: the JSON Schema of the whole configuration
    /// (static per gateway build). Public.
    pub async fn schema(&self) -> Result<Value> {
        let request = self
            .client
            .request(Method::OPTIONS, &["api", "config"], &[])?;
        self.client.json(request).await
    }

    /// `OPTIONS /api/config/prop`: the schema plus the field's metadata under
    /// `x-zeroclaw-prop`. Public.
    pub async fn prop_schema(&self, path: &str) -> Result<Value> {
        let request = self.client.request(
            Method::OPTIONS,
            &["api", "config", "prop"],
            &[("path", path)],
        )?;
        self.client.json(request).await
    }

    /// `GET /api/config/prop`: one field.
    pub async fn get(&self, path: &str) -> Result<PropValue> {
        self.client
            .get(&["api", "config", "prop"], &[("path", path)])
            .await
    }

    /// `PUT /api/config/prop`: writes one field. `value` is typed JSON
    /// (`null` resets it); `comment` is written above the key in the TOML.
    /// A map alias implied by the path is created.
    pub async fn set(
        &self,
        path: &str,
        value: impl Serialize,
        comment: Option<&str>,
    ) -> Result<PropValue> {
        let value = serde_json::to_value(value).map_err(|e| crate::Error::Decode(e.to_string()))?;
        let mut body = serde_json::json!({ "path": path, "value": value });
        if let Some(comment) = comment {
            body["comment"] = comment.into();
        }
        self.client
            .send_json(Method::PUT, &["api", "config", "prop"], &[], &body)
            .await
    }

    /// `DELETE /api/config/prop`: resets one field to its default.
    pub async fn reset(&self, path: &str) -> Result<PropValue> {
        let request = self.client.request(
            Method::DELETE,
            &["api", "config", "prop"],
            &[("path", path)],
        )?;
        self.client.json(request).await
    }

    /// `PATCH /api/config`: applies `ops` atomically (all or nothing). With
    /// `override_drift`, paths edited on disk since the gateway loaded them
    /// are overwritten instead of refused with 409.
    pub async fn patch(&self, ops: &[PatchOp], override_drift: bool) -> Result<PatchResponse> {
        let mut request = self
            .client
            .request(Method::PATCH, &["api", "config"], &[])?
            .json(ops);
        if override_drift {
            request = request.header(OVERRIDE_DRIFT, "true");
        }
        self.client.json(request).await
    }

    /// `GET /api/config/list`: every field reachable under `prefix` (all
    /// fields without one), with its kind, category and current value.
    pub async fn list(&self, prefix: Option<&str>) -> Result<ConfigList> {
        self.client
            .get(&["api", "config", "list"], &query(&[("prefix", prefix)]))
            .await
    }

    /// `GET /api/config/drift`: fields whose value on disk differs from the
    /// gateway's.
    pub async fn drift(&self) -> Result<Vec<DriftEntry>> {
        #[derive(Deserialize)]
        struct Drift {
            drifted: Vec<DriftEntry>,
        }
        let drift: Drift = self.client.get(&["api", "config", "drift"], &[]).await?;
        Ok(drift.drifted)
    }

    /// `GET /api/config/reload-status`: whether saved changes wait for a
    /// daemon reload ([`crate::system::SystemApi::reload`]).
    pub async fn pending_reload(&self) -> Result<bool> {
        #[derive(Deserialize)]
        struct Status {
            pending_reload: bool,
        }
        let status: Status = self
            .client
            .get(&["api", "config", "reload-status"], &[])
            .await?;
        Ok(status.pending_reload)
    }

    /// `POST /api/config/init`: fills unset nested sections under `section`
    /// (everywhere without one) with defaults; returns the sections created.
    pub async fn init(&self, section: Option<&str>) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Init {
            initialized: Vec<String>,
        }
        let request = self.client.request(
            Method::POST,
            &["api", "config", "init"],
            &query(&[("section", section)]),
        )?;
        let init: Init = self.client.json(request).await?;
        Ok(init.initialized)
    }

    /// `POST /api/config/migrate`: migrates `config.toml` to the current
    /// schema in place, keeping a backup.
    pub async fn migrate(&self) -> Result<MigrateResponse> {
        let request = self
            .client
            .request(Method::POST, &["api", "config", "migrate"], &[])?;
        self.client.json(request).await
    }

    /// `GET /api/config/templates`: the map and list sections and the type
    /// of their entries.
    pub async fn templates(&self) -> Result<Vec<TemplateEntry>> {
        #[derive(Deserialize)]
        struct Templates {
            templates: Vec<TemplateEntry>,
        }
        let templates: Templates = self
            .client
            .get(&["api", "config", "templates"], &[])
            .await?;
        Ok(templates.templates)
    }

    /// `GET /api/config/map-keys`: the aliases of a map section
    /// (e.g. `agents`, `channels.whatsapp`).
    pub async fn map_keys(&self, path: &str) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Keys {
            keys: Vec<String>,
        }
        let keys: Keys = self
            .client
            .get(&["api", "config", "map-keys"], &[("path", path)])
            .await?;
        Ok(keys.keys)
    }

    /// `POST /api/config/map-key`: creates alias `key` under map `path` with
    /// defaults (`created` is false when it already existed).
    pub async fn create_map_key(&self, path: &str, key: &str) -> Result<MapKeyResponse> {
        let request = self.client.request(
            Method::POST,
            &["api", "config", "map-key"],
            &[("path", path), ("key", key)],
        )?;
        self.client.json(request).await
    }

    /// `DELETE /api/config/map-key`: removes alias `key` and the references
    /// to it. Deleting an agent also archives its workspace and its memory,
    /// cron jobs and sessions; check [`ConfigApi::delete_plan`] first.
    pub async fn delete_map_key(&self, path: &str, key: &str) -> Result<MapKeyResponse> {
        let request = self.client.request(
            Method::DELETE,
            &["api", "config", "map-key"],
            &[("path", path), ("key", key)],
        )?;
        self.client.json(request).await
    }

    /// `GET /api/config/delete-plan`: what deleting alias `key` would block
    /// on or rewrite, without changing anything.
    pub async fn delete_plan(&self, path: &str, key: &str) -> Result<DeletePlan> {
        self.client
            .get(
                &["api", "config", "delete-plan"],
                &[("path", path), ("key", key)],
            )
            .await
    }

    /// `POST /api/config/rename-map-key`: renames alias `from` to `to` and
    /// every reference to it.
    pub async fn rename_map_key(&self, path: &str, from: &str, to: &str) -> Result<RenameResponse> {
        self.client
            .send_json(
                Method::POST,
                &["api", "config", "rename-map-key"],
                &[],
                &serde_json::json!({ "path": path, "from": from, "to": to }),
            )
            .await
    }

    /// `POST /api/config/model-providers/{type}/{alias}/refresh-context-window`:
    /// asks the provider for the model's context window and stores it.
    pub async fn refresh_context_window(&self, provider_type: &str, alias: &str) -> Result<u64> {
        #[derive(Deserialize)]
        struct Refreshed {
            context_window: u64,
        }
        let request = self.client.request(
            Method::POST,
            &[
                "api",
                "config",
                "model-providers",
                provider_type,
                alias,
                "refresh-context-window",
            ],
            &[],
        )?;
        let refreshed: Refreshed = self.client.json(request).await?;
        Ok(refreshed.context_window)
    }

    /// `GET /api/config/resolve-alias-source`: the aliases an alias-reference
    /// field may point to (providers and channels as `type.alias`).
    pub async fn alias_values(&self, source: AliasSource) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Values {
            values: Vec<String>,
        }
        let values: Values = self
            .client
            .get(
                &["api", "config", "resolve-alias-source"],
                &[("source", source.as_str())],
            )
            .await?;
        Ok(values.values)
    }

    /// `GET /api/config/catalog`: the model providers this build knows.
    pub async fn catalog(&self) -> Result<Vec<CatalogModelProvider>> {
        #[derive(Deserialize)]
        struct Catalog {
            model_providers: Vec<CatalogModelProvider>,
        }
        let catalog: Catalog = self.client.get(&["api", "config", "catalog"], &[]).await?;
        Ok(catalog.model_providers)
    }

    /// `GET /api/config/catalog/models`: the models of a provider (may call
    /// the provider with the configured credentials).
    pub async fn catalog_models(&self, model_provider: &str) -> Result<CatalogModels> {
        self.client
            .get(
                &["api", "config", "catalog", "models"],
                &[("model_provider", model_provider)],
            )
            .await
    }

    /// `GET /api/config/status`: whether the setup is complete.
    pub async fn status(&self) -> Result<ConfigStatus> {
        self.client.get(&["api", "config", "status"], &[]).await
    }

    /// `GET /api/config/agent-options`: the aliases an agent can reference.
    pub async fn agent_options(&self) -> Result<AgentOptions> {
        self.client
            .get(&["api", "config", "agent-options"], &[])
            .await
    }

    /// `GET /api/config/sections`: the setup sections, in wizard order.
    pub async fn sections(&self) -> Result<Vec<ConfigSection>> {
        #[derive(Deserialize)]
        struct Sections {
            sections: Vec<ConfigSection>,
        }
        let sections: Sections = self.client.get(&["api", "config", "sections"], &[]).await?;
        Ok(sections.sections)
    }

    /// `GET /api/config/sections/{section}`: the items a picker section
    /// offers (e.g. the provider families of `providers.models`).
    pub async fn section_items(&self, section: &str) -> Result<SectionItems> {
        self.client
            .get(&["api", "config", "sections", section], &[])
            .await
    }

    /// `POST /api/config/sections/{section}/items/{key}`: picks an item,
    /// creating what it needs (`alias` defaults to `default` where the
    /// section takes one). Returns where its fields live.
    pub async fn select_section_item(
        &self,
        section: &str,
        key: &str,
        alias: Option<&str>,
    ) -> Result<SelectedItem> {
        self.client
            .send_json(
                Method::POST,
                &["api", "config", "sections", section, "items", key],
                &[],
                &serde_json::json!({ "alias": alias }),
            )
            .await
    }
}

/// A field as `GET`/`PUT`/`DELETE /api/config/prop` return it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropValue {
    /// A regular field: `value` is the gateway's display string (`null`
    /// after a reset).
    Value {
        /// Dotted path.
        path: String,
        /// Display value.
        value: Value,
        /// Validation problems of the configuration.
        #[serde(default)]
        warnings: Vec<ValidationWarning>,
    },
    /// A secret: only whether it is set.
    Secret {
        /// Dotted path.
        path: String,
        /// Whether the secret has a value.
        populated: bool,
    },
}

/// A validation problem the gateway reports without refusing the write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Stable code, e.g. `pre_existing_validation_error`.
    pub code: String,
    /// Explanation.
    pub message: String,
    /// The field concerned.
    pub path: String,
}

/// One operation of a [`ConfigApi::patch`]: RFC 6902 `add`, `replace`,
/// `remove` and `test`, plus ZeroClaw's `comment`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchOp {
    /// `add`, `replace`, `remove`, `test` or `comment`.
    pub op: String,
    /// Dotted path, or a JSON Pointer (`/a/b`).
    pub path: String,
    /// The value for `add`, `replace` and `test`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// TOML comment written above the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl PatchOp {
    /// Sets `path` to `value`.
    pub fn replace(path: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::with_value("replace", path, value)
    }

    /// Sets `path` to `value`, creating the map alias it implies.
    pub fn add(path: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::with_value("add", path, value)
    }

    /// Resets `path` to its default.
    pub fn remove(path: impl Into<String>) -> Self {
        Self {
            op: "remove".into(),
            path: path.into(),
            value: None,
            comment: None,
        }
    }

    /// Fails the whole patch unless `path` currently displays as `value`.
    pub fn test(path: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::with_value("test", path, value)
    }

    /// Writes `comment` above the key of `path`.
    pub fn comment(path: impl Into<String>, comment: impl Into<String>) -> Self {
        Self {
            op: "comment".into(),
            path: path.into(),
            value: None,
            comment: Some(comment.into()),
        }
    }

    fn with_value(op: &str, path: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            op: op.into(),
            path: path.into(),
            value: Some(value.into()),
            comment: None,
        }
    }
}

/// Result of a [`ConfigApi::patch`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchResponse {
    /// Always true on success: the patch was saved.
    pub saved: bool,
    /// One result per operation, in order.
    pub results: Vec<PatchOpResult>,
    /// Validation problems outside the patched paths.
    #[serde(default)]
    pub warnings: Vec<ValidationWarning>,
}

/// What one patch operation left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchOpResult {
    /// The operation.
    pub op: String,
    /// Dotted path.
    pub path: String,
    /// New display value (absent for secrets and comments).
    #[serde(default)]
    pub value: Option<Value>,
    /// For secrets: whether it now has a value.
    #[serde(default)]
    pub populated: Option<bool>,
    /// For `comment`: the comment written.
    #[serde(default)]
    pub comment: Option<String>,
}

/// Result of [`ConfigApi::list`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigList {
    /// The fields.
    pub entries: Vec<ConfigEntry>,
    /// Fields whose value on disk differs from the gateway's.
    #[serde(default)]
    pub drifted: Vec<DriftEntry>,
}

/// One configuration field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// Dotted path.
    pub path: String,
    /// Category for grouping.
    pub category: String,
    /// `string`, `bool`, `integer`, `float`, `enum`, `alias-ref`,
    /// `string-array`, `object-array` or `object`.
    pub kind: String,
    /// The Rust type, e.g. `Option<String>`.
    pub type_hint: String,
    /// Display value; absent for secrets.
    #[serde(default)]
    pub value: Option<Value>,
    /// Whether it has a value.
    pub populated: bool,
    /// Whether it is a secret.
    pub is_secret: bool,
    /// Whether an environment variable overrides it.
    #[serde(default)]
    pub is_env_overridden: bool,
    /// Allowed values of an `enum`.
    #[serde(default)]
    pub enum_variants: Vec<String>,
    /// Setup section it belongs to.
    #[serde(default)]
    pub section: Option<String>,
    /// Dashboard tab.
    #[serde(default)]
    pub tab: String,
    /// Whether it is edited as multi-line text.
    #[serde(default)]
    pub multiline: bool,
}

/// A field whose value on disk differs from the gateway's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftEntry {
    /// Dotted path.
    pub path: String,
    /// Whether it is a secret (no values shown).
    #[serde(default)]
    pub secret: bool,
    /// Always true.
    pub drifted: bool,
    /// The gateway's value.
    #[serde(default)]
    pub in_memory_value: Option<Value>,
    /// The value in `config.toml`.
    #[serde(default)]
    pub on_disk_value: Option<Value>,
}

/// Result of [`ConfigApi::migrate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrateResponse {
    /// Whether the file changed.
    pub migrated: bool,
    /// Where the previous file was kept.
    #[serde(default)]
    pub backup_path: Option<String>,
    /// Schema version now on disk.
    pub schema_version: u32,
}

/// A map or list section and the type of its entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateEntry {
    /// Dotted path.
    pub path: String,
    /// `map` or `list`.
    pub kind: String,
    /// Rust type of an entry.
    pub value_type: String,
    /// What the section holds.
    pub description: String,
}

/// Result of creating or deleting a map alias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapKeyResponse {
    /// The map section.
    pub path: String,
    /// The alias.
    pub key: String,
    /// Whether a new alias was created.
    pub created: bool,
    /// Cleanup steps that failed after the change was saved.
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
}

/// What deleting an alias would do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletePlan {
    /// The map section.
    pub path: String,
    /// The alias.
    pub key: String,
    /// Whether the delete would go through.
    pub allowed: bool,
    /// References that block it.
    pub blockers: Vec<ReferenceSite>,
    /// References it would clear.
    pub scrubs: Vec<ReferenceSite>,
    /// For agents: live ACP sessions (they block the delete).
    #[serde(default)]
    pub live_acp_sessions: Option<usize>,
    /// Whether the agent's memory, cron jobs and sessions go too.
    pub cascades_owned_state: bool,
}

/// A field that references an alias.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceSite {
    /// Dotted path of the referencing field.
    pub path: String,
    /// Its raw value.
    pub raw_value: String,
}

/// Result of [`ConfigApi::rename_map_key`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenameResponse {
    /// The map section.
    pub path: String,
    /// Old alias.
    pub from: String,
    /// New alias.
    pub to: String,
    /// Whether it was renamed.
    pub renamed: bool,
    /// Follow-up steps that failed (re-sending the rename finishes them).
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// The alias kinds an alias-reference field can point to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AliasSource {
    /// `providers.models.*`
    ModelProviders,
    /// TTS providers.
    TtsProviders,
    /// Transcription providers.
    TranscriptionProviders,
    /// `channels.*`
    Channels,
    /// Risk profiles.
    RiskProfiles,
    /// Runtime profiles.
    RuntimeProfiles,
    /// `agents.*`
    Agents,
    /// Skill bundles.
    SkillBundles,
    /// Knowledge bundles.
    KnowledgeBundles,
    /// MCP bundles.
    McpBundles,
}

impl AliasSource {
    /// The gateway's name for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelProviders => "model_providers",
            Self::TtsProviders => "tts_providers",
            Self::TranscriptionProviders => "transcription_providers",
            Self::Channels => "channels",
            Self::RiskProfiles => "risk_profiles",
            Self::RuntimeProfiles => "runtime_profiles",
            Self::Agents => "agents",
            Self::SkillBundles => "skill_bundles",
            Self::KnowledgeBundles => "knowledge_bundles",
            Self::McpBundles => "mcp_bundles",
        }
    }
}

/// A model provider the build knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogModelProvider {
    /// Provider family name.
    pub name: String,
    /// Display name.
    pub display_name: String,
    /// Whether it runs locally.
    pub local: bool,
}

/// The models of a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogModels {
    /// Provider family name.
    pub model_provider: String,
    /// Model ids.
    pub models: Vec<String>,
    /// Prices per model, when the provider publishes them.
    #[serde(default)]
    pub pricing: Option<std::collections::HashMap<String, ModelPricing>>,
    /// Whether it runs locally.
    pub local: bool,
    /// Whether the list came live from the provider.
    pub live: bool,
}

/// A model's prices as the provider states them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Input price.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Output price.
    #[serde(default)]
    pub completion: Option<String>,
    /// Cached input read price.
    #[serde(default)]
    pub input_cache_read: Option<String>,
    /// Cached input write price.
    #[serde(default)]
    pub input_cache_write: Option<String>,
}

/// Whether the setup is complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigStatus {
    /// Whether the quickstart still has to run.
    pub needs_quickstart: bool,
    /// `has_dispatchable_agent`, `incomplete_agent` or `fresh_install`.
    pub reason: String,
    /// Whether part of the setup is done.
    pub has_partial_state: bool,
    /// Steps left, in words.
    pub missing: Vec<String>,
}

/// The aliases an agent can reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentOptions {
    /// Channels, as `type.alias`.
    pub channels: Vec<String>,
    /// Channel types.
    pub channel_types: Vec<String>,
    /// Model providers, as `type.alias`.
    pub model_providers: Vec<String>,
    /// Risk profiles.
    pub risk_profiles: Vec<String>,
    /// Runtime profiles.
    pub runtime_profiles: Vec<String>,
    /// Skill bundles.
    pub skill_bundles: Vec<String>,
    /// Knowledge bundles.
    pub knowledge_bundles: Vec<String>,
    /// MCP bundles.
    pub mcp_bundles: Vec<String>,
    /// Agents.
    pub agents: Vec<String>,
}

/// A setup section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigSection {
    /// Section key, e.g. `providers.models`.
    pub key: String,
    /// Label.
    pub label: String,
    /// Help text.
    pub help: String,
    /// Whether it offers items to pick ([`ConfigApi::section_items`]).
    pub has_picker: bool,
    /// Whether it is completed.
    pub completed: bool,
    /// Whether it is ready to use.
    #[serde(default)]
    pub ready: bool,
    /// Group it is shown under.
    #[serde(default)]
    pub group: String,
    /// Whether the quickstart includes it.
    #[serde(default)]
    pub is_quickstart: bool,
    /// `direct_form`, `one_tier_alias_map`, `typed_family_map` or
    /// `backend_picker`.
    #[serde(default)]
    pub shape: Option<String>,
    /// Cost category.
    #[serde(default)]
    pub cost_category: String,
}

/// The items of a picker section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionItems {
    /// Section key.
    pub section: String,
    /// The items.
    pub items: Vec<SectionItem>,
    /// Help text.
    pub help: String,
}

/// An item of a picker section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionItem {
    /// Item key.
    pub key: String,
    /// Label.
    pub label: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// `configured`, `needs setup`, `active` or `created`.
    #[serde(default)]
    pub badge: Option<String>,
}

/// Result of [`ConfigApi::select_section_item`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedItem {
    /// Prefix of the fields to fill ([`ConfigApi::list`]).
    pub fields_prefix: String,
    /// Whether something was created.
    pub created: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_op_vectors() {
        let vectors = [
            (
                PatchOp::replace("a.b", 1),
                serde_json::json!({ "op": "replace", "path": "a.b", "value": 1 }),
            ),
            (
                PatchOp::add("agents.x.model", "m"),
                serde_json::json!({ "op": "add", "path": "agents.x.model", "value": "m" }),
            ),
            (
                PatchOp::remove("/a/b"),
                serde_json::json!({ "op": "remove", "path": "/a/b" }),
            ),
            (
                PatchOp::test("a", true),
                serde_json::json!({ "op": "test", "path": "a", "value": true }),
            ),
            (
                PatchOp::comment("a", "why"),
                serde_json::json!({ "op": "comment", "path": "a", "comment": "why" }),
            ),
            (
                PatchOp::replace("a", serde_json::json!(["x", "y"])),
                serde_json::json!({ "op": "replace", "path": "a", "value": ["x", "y"] }),
            ),
        ];
        for (op, expected) in vectors {
            assert_eq!(serde_json::to_value(&op).unwrap(), expected);
            assert_eq!(serde_json::from_value::<PatchOp>(expected).unwrap(), op);
        }
    }

    #[test]
    fn alias_source_vectors() {
        let vectors = [
            (AliasSource::ModelProviders, "model_providers"),
            (AliasSource::TtsProviders, "tts_providers"),
            (
                AliasSource::TranscriptionProviders,
                "transcription_providers",
            ),
            (AliasSource::Channels, "channels"),
            (AliasSource::RiskProfiles, "risk_profiles"),
            (AliasSource::RuntimeProfiles, "runtime_profiles"),
            (AliasSource::Agents, "agents"),
            (AliasSource::SkillBundles, "skill_bundles"),
            (AliasSource::KnowledgeBundles, "knowledge_bundles"),
            (AliasSource::McpBundles, "mcp_bundles"),
        ];
        for (source, name) in vectors {
            assert_eq!(source.as_str(), name);
        }
    }

    #[test]
    fn props_decode_as_values_or_secrets() {
        let value: PropValue =
            serde_json::from_str(r#"{"path":"gateway.port","value":"42617"}"#).unwrap();
        assert!(matches!(value, PropValue::Value { ref value, .. } if value == "42617"));
        let secret: PropValue = serde_json::from_str(
            r#"{"path":"providers.models.x.default.api_key","populated":true}"#,
        )
        .unwrap();
        assert!(matches!(
            secret,
            PropValue::Secret {
                populated: true,
                ..
            }
        ));
    }

    #[test]
    fn patch_ops_serialize_as_the_gateway_reads_them() {
        let ops = [
            PatchOp::replace("gateway.port", 42617),
            PatchOp::remove("agents.x.model"),
            PatchOp::comment("gateway.port", "set by the SDK"),
        ];
        assert_eq!(
            serde_json::to_value(ops).unwrap(),
            serde_json::json!([
                { "op": "replace", "path": "gateway.port", "value": 42617 },
                { "op": "remove", "path": "agents.x.model" },
                { "op": "comment", "path": "gateway.port", "comment": "set by the SDK" }
            ])
        );
    }
}
