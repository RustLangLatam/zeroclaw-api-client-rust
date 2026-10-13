//! The gateway itself: health, status, metrics, diagnostics, tools,
//! integrations, logs, version upgrades and daemon administration.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/lib.rs` (`/health`,
//! `/metrics`, `/admin/*`), `api.rs`, `api_logs.rs`, `version.rs` and
//! `api_sop_author.rs` (tool parameter options).

use std::collections::BTreeMap;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, query};
use crate::error::Result;

/// Gateway-wide endpoints, from [`Client::system`].
#[derive(Debug, Clone, Copy)]
pub struct SystemApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Health, status, diagnostics, logs, versions and administration.
    pub fn system(&self) -> SystemApi<'_> {
        SystemApi { client: self }
    }
}

impl SystemApi<'_> {
    /// `GET /health`: liveness and component health. Public.
    pub async fn health(&self) -> Result<Health> {
        self.client.get(&["health"], &[]).await
    }

    /// `GET /api/health`: the runtime's component health.
    pub async fn runtime_health(&self) -> Result<HealthSnapshot> {
        #[derive(Deserialize)]
        struct Wrapped {
            health: HealthSnapshot,
        }
        let wrapped: Wrapped = self.client.get(&["api", "health"], &[]).await?;
        Ok(wrapped.health)
    }

    /// `GET /metrics`: Prometheus text exposition. Public.
    pub async fn metrics(&self) -> Result<String> {
        self.client
            .text(self.client.request(Method::GET, &["metrics"], &[])?)
            .await
    }

    /// `GET /api/status`: version, model, channels, nodes and process
    /// figures; with `agent`, that agent's model.
    pub async fn status(&self, agent: Option<&str>) -> Result<Status> {
        self.client
            .get(&["api", "status"], &query(&[("agent", agent)]))
            .await
    }

    /// `POST /api/doctor`: runs the configuration diagnostics.
    pub async fn doctor(&self) -> Result<DoctorReport> {
        let request = self.client.request(Method::POST, &["api", "doctor"], &[])?;
        self.client.json(request).await
    }

    /// `GET /api/tools`: the tools an agent (all of them without one) can call.
    pub async fn tools(&self, agent: Option<&str>) -> Result<Vec<ToolSpec>> {
        #[derive(Deserialize)]
        struct Tools {
            tools: Vec<ToolSpec>,
        }
        let tools: Tools = self
            .client
            .get(&["api", "tools"], &query(&[("agent", agent)]))
            .await?;
        Ok(tools.tools)
    }

    /// `POST /api/tools/param-options`: the values a tool parameter of
    /// `domain` accepts (`channel_refs`, `peer_targets`, `peer_groups`,
    /// `agent_aliases`, `tool_names`, `memory_categories`). `args` holds the
    /// sibling arguments the options depend on.
    pub async fn tool_param_options(
        &self,
        domain: &str,
        agent: Option<&str>,
        args: Option<Value>,
    ) -> Result<Vec<OptionEntry>> {
        #[derive(Deserialize)]
        struct Options {
            options: Vec<OptionEntry>,
        }
        let body = serde_json::json!({
            "domain": domain,
            "agent": agent,
            "args": args.unwrap_or(Value::Null),
        });
        let options: Options = self
            .client
            .send_json(Method::POST, &["api", "tools", "param-options"], &[], &body)
            .await?;
        Ok(options.options)
    }

    /// `GET /api/integrations`: the integrations this build offers.
    pub async fn integrations(&self) -> Result<Vec<Integration>> {
        #[derive(Deserialize)]
        struct Integrations {
            integrations: Vec<Integration>,
        }
        let integrations: Integrations = self.client.get(&["api", "integrations"], &[]).await?;
        Ok(integrations.integrations)
    }

    /// `GET /api/integrations/settings`: each integration's state, by name.
    pub async fn integration_settings(&self) -> Result<BTreeMap<String, IntegrationSetting>> {
        #[derive(Deserialize)]
        struct Settings {
            settings: BTreeMap<String, IntegrationSetting>,
        }
        let settings: Settings = self
            .client
            .get(&["api", "integrations", "settings"], &[])
            .await?;
        Ok(settings.settings)
    }

    /// `GET /api/cli-tools`: command-line tools found on the host's `PATH`
    /// (slow: it runs each with `--version`).
    pub async fn cli_tools(&self) -> Result<Vec<CliTool>> {
        #[derive(Deserialize)]
        struct Tools {
            cli_tools: Vec<CliTool>,
        }
        let tools: Tools = self.client.get(&["api", "cli-tools"], &[]).await?;
        Ok(tools.cli_tools)
    }

    /// `GET /api/tuis`: terminal clients connected to the daemon.
    pub async fn tuis(&self) -> Result<Vec<Tui>> {
        #[derive(Deserialize)]
        struct Tuis {
            tuis: Vec<Tui>,
        }
        let tuis: Tuis = self.client.get(&["api", "tuis"], &[]).await?;
        Ok(tuis.tuis)
    }

    /// `GET /api/logs`: a page of structured log events, newest first.
    /// Continue with [`LogQuery::until_line_offset`] set to the page's
    /// `next_cursor_line_offset`.
    pub async fn logs(&self, filter: &LogQuery) -> Result<LogPage> {
        let pairs = filter.pairs();
        let pairs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.client.get(&["api", "logs"], &pairs).await
    }

    /// `GET /api/version/check`: whether a newer release exists (`force`
    /// skips the gateway's one-hour cache; `version` checks one tag).
    pub async fn version_check(&self, force: bool, version: Option<&str>) -> Result<VersionCheck> {
        let force = if force { Some("true") } else { None };
        self.client
            .get(
                &["api", "version", "check"],
                &query(&[("force", force), ("version", version)]),
            )
            .await
    }

    /// `POST /api/version/upgrade`: starts a self-upgrade (latest release
    /// without `version`); returns the handoff id to follow with
    /// [`SystemApi::upgrade_status`]. Needs `gateway.allow_self_upgrade`.
    pub async fn upgrade(&self, version: Option<&str>, auto_restart: bool) -> Result<String> {
        #[derive(Deserialize)]
        struct Accepted {
            handoff_id: String,
        }
        let accepted: Accepted = self
            .client
            .send_json(
                Method::POST,
                &["api", "version", "upgrade"],
                &[],
                &serde_json::json!({ "version": version, "auto_restart": auto_restart }),
            )
            .await?;
        Ok(accepted.handoff_id)
    }

    /// `GET /api/version/upgrade/status`: progress of an upgrade (the latest
    /// without `handoff_id`).
    pub async fn upgrade_status(&self, handoff_id: Option<&str>) -> Result<UpgradeStatus> {
        self.client
            .get(
                &["api", "version", "upgrade", "status"],
                &query(&[("handoff_id", handoff_id)]),
            )
            .await
    }

    /// `POST /admin/reload`: restarts the daemon's subsystems from the
    /// configuration on disk. The connection drops shortly after; reconnect.
    /// Allowed from loopback, or remotely with `gateway.allow_remote_admin`.
    pub async fn reload(&self) -> Result<AdminResponse> {
        let request = self
            .client
            .request(Method::POST, &["admin", "reload"], &[])?;
        self.client.json(request).await
    }

    /// `POST /admin/shutdown`: stops the gateway. Loopback only.
    pub async fn shutdown(&self) -> Result<AdminResponse> {
        let request = self
            .client
            .request(Method::POST, &["admin", "shutdown"], &[])?;
        self.client.json(request).await
    }

    /// `GET /api/openapi.json`: the gateway's OpenAPI document (it covers
    /// the configuration, version, SOP and skill routes). Public.
    pub async fn openapi(&self) -> Result<Value> {
        self.client.get(&["api", "openapi.json"], &[]).await
    }
}

/// Answer of `GET /health`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    /// Always `ok` while the gateway answers.
    pub status: String,
    /// Whether at least one client is paired.
    pub paired: bool,
    /// Whether requests need a bearer token.
    pub require_pairing: bool,
    /// Component health (raw when the gateway could not serialize it).
    pub runtime: Value,
}

/// The runtime's component health (`HealthSnapshot`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Daemon process id.
    pub pid: u32,
    /// When it was computed (RFC 3339).
    pub updated_at: String,
    /// Seconds since the daemon started.
    pub uptime_seconds: u64,
    /// Health per component (`gateway`, `channel:whatsapp.x`, …).
    pub components: BTreeMap<String, ComponentHealth>,
}

/// One component's health.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// `ok`, `error`, …
    pub status: String,
    /// Last update (RFC 3339).
    pub updated_at: String,
    /// Last time it was healthy.
    #[serde(default)]
    pub last_ok: Option<String>,
    /// Last error.
    #[serde(default)]
    pub last_error: Option<String>,
    /// How many times it was restarted.
    pub restart_count: u64,
}

/// Answer of `GET /api/status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// ZeroClaw version.
    pub version: String,
    /// Model provider in use, as `type.alias`.
    #[serde(default)]
    pub model_provider: Option<String>,
    /// Model in use.
    pub model: String,
    /// Sampling temperature.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Seconds since the daemon started.
    pub uptime_seconds: u64,
    /// When the daemon started (RFC 3339).
    pub daemon_started_at: String,
    /// Gateway port.
    pub gateway_port: u16,
    /// UI locale.
    pub locale: String,
    /// Memory backend.
    pub memory_backend: String,
    /// Whether a client is paired.
    pub paired: bool,
    /// Configured channels, as `type.alias`.
    pub channels: BTreeMap<String, bool>,
    /// Connected nodes and peers found on the network.
    #[serde(default)]
    pub nodes: Value,
    /// Component health.
    pub health: HealthSnapshot,
    /// The agent asked about, if any.
    #[serde(default)]
    pub agent_alias: Option<String>,
    /// Process figures.
    #[serde(default)]
    pub process: Option<ProcessStats>,
    /// Whether the daemon checks for updates.
    #[serde(default)]
    pub check_updates: bool,
    /// Whether self-upgrade is allowed.
    #[serde(default)]
    pub allow_self_upgrade: bool,
    /// `supervised`, `self_respawn` or `manual`.
    #[serde(default)]
    pub restart_mode: Option<String>,
    /// How to restart the daemon.
    #[serde(default)]
    pub restart_hint: Option<String>,
    /// Configuration a running channel picks up without a restart.
    #[serde(default)]
    pub live_channel_config: Vec<String>,
}

/// The daemon's resource use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessStats {
    /// Resident memory.
    pub rss_bytes: u64,
    /// Host memory.
    pub system_ram_total_bytes: u64,
    /// CPU use.
    #[serde(default)]
    pub cpu_percent: Option<f32>,
    /// Host CPUs.
    pub num_cpus: u32,
}

/// Result of [`SystemApi::doctor`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    /// Each check.
    pub results: Vec<Diagnostic>,
    /// Counts by severity.
    pub summary: DoctorSummary,
}

/// One diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// `ok`, `warn` or `error`.
    pub severity: String,
    /// Area checked.
    pub category: String,
    /// Finding.
    pub message: String,
}

/// Counts of a doctor run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorSummary {
    /// Checks passed.
    pub ok: usize,
    /// Warnings.
    pub warnings: usize,
    /// Errors.
    pub errors: usize,
}

/// A tool an agent can call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Tool name.
    pub name: String,
    /// What it does.
    pub description: String,
    /// JSON Schema of its arguments.
    pub parameters: Value,
    /// JSON Schema of its output, when declared.
    #[serde(default)]
    pub output: Option<Value>,
    /// Option domain of the parameters that take known values.
    #[serde(default)]
    pub param_domains: BTreeMap<String, String>,
}

/// A value a tool parameter accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionEntry {
    /// The value.
    pub value: String,
    /// Label to show.
    #[serde(default)]
    pub label: Option<String>,
    /// Extra hint.
    #[serde(default)]
    pub hint: Option<String>,
}

/// An integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    /// Name.
    pub name: String,
    /// What it does.
    pub description: String,
    /// `Chat`, `AiModel`, `ToolsAutomation` or `Platform`.
    pub category: String,
    /// The category in words.
    pub category_label: String,
    /// `Available` or `Active`.
    pub status: String,
}

/// An integration's state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegrationSetting {
    /// Whether it is active.
    pub enabled: bool,
    /// Category.
    pub category: String,
    /// Status.
    pub status: String,
}

/// A command-line tool on the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliTool {
    /// Command name.
    pub name: String,
    /// Full path.
    pub path: String,
    /// Reported version.
    #[serde(default)]
    pub version: Option<String>,
    /// Kind of tool (`VersionControl`, `Language`, …).
    pub category: String,
}

/// A terminal client connected to the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tui {
    /// Client id.
    pub tui_id: String,
    /// When it connected (RFC 3339).
    pub connected_at: String,
    /// Who it is.
    pub peer_label: String,
    /// `unix` or `wss`.
    pub transport: String,
}

/// Filters of [`SystemApi::logs`]. Unset fields are not sent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LogQuery {
    /// Events at or after (RFC 3339).
    pub since_ts: Option<String>,
    /// Events before (RFC 3339).
    pub until_ts: Option<String>,
    /// Paging cursor: the previous page's `next_cursor_line_offset`.
    pub until_line_offset: Option<u64>,
    /// Event action.
    pub action: Option<String>,
    /// Event category.
    pub category: Option<String>,
    /// Event outcome.
    pub outcome: Option<String>,
    /// Minimum severity number.
    pub severity_min: Option<u8>,
    /// Trace id.
    pub trace_id: Option<String>,
    /// Free text.
    pub q: Option<String>,
    /// Leaves out the daemon's internal events.
    pub hide_internal: bool,
    /// Page size (gateway default 200).
    pub limit: Option<usize>,
    /// Exact matches on attribution fields (`agent_alias`, `channel`,
    /// `session_key`, `tool`, …).
    pub attributes: BTreeMap<String, String>,
}

impl LogQuery {
    /// The query string pairs of the filters that are set.
    fn pairs(&self) -> Vec<(String, String)> {
        let text = [
            ("since_ts", &self.since_ts),
            ("until_ts", &self.until_ts),
            ("action", &self.action),
            ("category", &self.category),
            ("outcome", &self.outcome),
            ("trace_id", &self.trace_id),
            ("q", &self.q),
        ];
        let numbers = [
            ("until_line_offset", self.until_line_offset),
            ("severity_min", self.severity_min.map(u64::from)),
            ("limit", self.limit.map(|l| l as u64)),
        ];
        text.into_iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| (k.to_string(), v.clone())))
            .chain(
                numbers
                    .into_iter()
                    .filter_map(|(k, v)| v.map(|v| (k.to_string(), v.to_string()))),
            )
            .chain(
                self.hide_internal
                    .then(|| ("hide_internal".into(), "true".into())),
            )
            .chain(self.attributes.iter().map(|(k, v)| (k.clone(), v.clone())))
            .collect()
    }
}

/// A page of [`SystemApi::logs`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogPage {
    /// Log events (OpenTelemetry-like JSON, see ZeroClaw's `LogEvent`).
    pub events: Vec<Value>,
    /// Cursor for the next (older) page.
    #[serde(default)]
    pub next_cursor_line_offset: Option<u64>,
    /// Whether this is the oldest page.
    pub at_end: bool,
    /// When the daemon started.
    pub daemon_started_at: String,
    /// Attribution fields that can be filtered on.
    #[serde(default)]
    pub attribution_keys: Vec<String>,
}

/// Answer of [`SystemApi::version_check`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionCheck {
    /// Running version.
    pub current_version: String,
    /// Latest release (`None` when the check failed).
    #[serde(default)]
    pub latest_version: Option<String>,
    /// Whether the latest is newer.
    pub is_newer: bool,
    /// Release page.
    #[serde(default)]
    pub release_url: Option<String>,
    /// Release notes (Markdown).
    #[serde(default)]
    pub release_notes: Option<String>,
    /// Release date.
    #[serde(default)]
    pub published_at: Option<String>,
    /// Why the check failed.
    #[serde(default)]
    pub error: Option<String>,
}

/// Progress of a self-upgrade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpgradeStatus {
    /// `idle`, `running`, `done`, `restarting` or `failed`.
    pub state: String,
    /// Upgrade id.
    #[serde(default)]
    pub handoff_id: Option<String>,
    /// Step 0 to 6.
    #[serde(default)]
    pub phase: Option<u8>,
    /// Last log lines.
    #[serde(default)]
    pub log_tail: Vec<String>,
    /// Version before.
    #[serde(default)]
    pub previous_version: Option<String>,
    /// Version being installed.
    #[serde(default)]
    pub target_version: Option<String>,
    /// How the daemon restarts.
    #[serde(default)]
    pub restart_mode: Option<String>,
    /// How to restart it by hand.
    #[serde(default)]
    pub restart_hint: Option<String>,
    /// Why it failed.
    #[serde(default)]
    pub error: Option<String>,
}

/// Answer of the `/admin` actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminResponse {
    /// Whether it was accepted.
    pub success: bool,
    /// What happens next.
    pub message: String,
}
