//! Scheduled jobs: shell commands or agent prompts on a cron schedule, with
//! their results delivered to a channel.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api.rs` (`/api/cron*`);
//! types in `crates/zeroclaw-runtime/src/cron/types.rs`.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// How long a manual run may take: the gateway allows 600 s.
const LONG_RUN: std::time::Duration = std::time::Duration::from_secs(660);

/// Cron endpoints, from [`Client::cron`].
#[derive(Debug, Clone, Copy)]
pub struct CronApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Scheduled jobs (`/api/cron`).
    pub fn cron(&self) -> CronApi<'_> {
        CronApi { client: self }
    }
}

impl CronApi<'_> {
    /// `GET /api/cron`: every job.
    pub async fn list(&self) -> Result<Vec<CronJob>> {
        #[derive(Deserialize)]
        struct Jobs {
            jobs: Vec<CronJob>,
        }
        let jobs: Jobs = self.client.get(&["api", "cron"], &[]).await?;
        Ok(jobs.jobs)
    }

    /// `POST /api/cron`: creates a job.
    pub async fn create(&self, job: &NewCronJob) -> Result<CronJob> {
        let created: JobEnvelope = self
            .client
            .send_json(Method::POST, &["api", "cron"], &[], job)
            .await?;
        Ok(created.job)
    }

    /// `PATCH /api/cron/{id}`: changes a job; unset fields stay as they are.
    pub async fn update(&self, id: &str, patch: &CronJobPatch) -> Result<CronJob> {
        let updated: JobEnvelope = self
            .client
            .send_json(Method::PATCH, &["api", "cron", id], &[], patch)
            .await?;
        Ok(updated.job)
    }

    /// `DELETE /api/cron/{id}`: deletes a job.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "cron", id], &[])?;
        self.client.empty(request).await
    }

    /// `POST /api/cron/{id}/run`: runs a job now and waits for it (the
    /// gateway allows up to its long-running timeout, 600 s by default). A
    /// failed job still returns `Ok`, with `success: false`.
    pub async fn run(&self, id: &str) -> Result<CronRunResult> {
        let request = self
            .client
            .request(Method::POST, &["api", "cron", id, "run"], &[])?
            .timeout(LONG_RUN);
        self.client.json(request).await
    }

    /// `GET /api/cron/{id}/runs`: the job's latest runs (`limit` 1 to 100,
    /// default 20).
    pub async fn runs(&self, id: &str, limit: Option<u32>) -> Result<Vec<CronRun>> {
        #[derive(Deserialize)]
        struct Runs {
            runs: Vec<CronRun>,
        }
        let limit = limit.map(|l| l.to_string());
        let query: Vec<(&str, &str)> = limit.iter().map(|l| ("limit", l.as_str())).collect();
        let runs: Runs = self
            .client
            .get(&["api", "cron", id, "runs"], &query)
            .await?;
        Ok(runs.runs)
    }

    /// `GET /api/cron/settings`: the scheduler settings.
    pub async fn settings(&self) -> Result<CronSettings> {
        self.client.get(&["api", "cron", "settings"], &[]).await
    }

    /// `PATCH /api/cron/settings`: changes the scheduler settings that are
    /// set in `settings`; saved to the configuration.
    pub async fn update_settings(&self, settings: &CronSettingsPatch) -> Result<CronSettings> {
        self.client
            .send_json(Method::PATCH, &["api", "cron", "settings"], &[], settings)
            .await
    }
}

#[derive(Deserialize)]
struct JobEnvelope {
    job: CronJob,
}

/// A scheduled job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CronJob {
    /// Job id.
    pub id: String,
    /// Schedule expression as written.
    pub expression: String,
    /// Parsed schedule.
    pub schedule: Schedule,
    /// Shell command (shell jobs).
    #[serde(default)]
    pub command: String,
    /// Prompt (agent jobs).
    #[serde(default)]
    pub prompt: Option<String>,
    /// Name.
    #[serde(default)]
    pub name: Option<String>,
    /// `shell` or `agent`.
    pub job_type: String,
    /// `isolated` or `main`.
    pub session_target: String,
    /// Model override (agent jobs).
    #[serde(default)]
    pub model: Option<String>,
    /// Agent that owns it.
    #[serde(default)]
    pub agent_alias: String,
    /// Whether it runs.
    pub enabled: bool,
    /// Where its output goes.
    pub delivery: Delivery,
    /// Whether it is deleted after one run.
    pub delete_after_run: bool,
    /// Tools an agent job may use.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    /// Whether an agent job uses memory.
    #[serde(default)]
    pub uses_memory: bool,
    /// `imperative` (API, CLI) or `declarative` (config file).
    #[serde(default)]
    pub source: String,
    /// `wrapped` or `raw` (shell jobs).
    #[serde(default)]
    pub shell_output_format: Option<String>,
    /// Creation time (RFC 3339).
    pub created_at: String,
    /// Next run (RFC 3339).
    pub next_run: String,
    /// Last run (RFC 3339).
    #[serde(default)]
    pub last_run: Option<String>,
    /// Last status.
    #[serde(default)]
    pub last_status: Option<String>,
    /// Last output.
    #[serde(default)]
    pub last_output: Option<String>,
}

/// When a job runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Schedule {
    /// A cron expression, in `tz` (IANA) or UTC.
    Cron {
        /// Expression.
        expr: String,
        /// Time zone.
        #[serde(default)]
        tz: Option<String>,
    },
    /// Once, at a time.
    At {
        /// RFC 3339 time.
        at: String,
    },
    /// Every `every_ms` milliseconds.
    Every {
        /// Interval.
        every_ms: u64,
    },
}

/// Where a job's output goes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Delivery {
    /// `none` or `announce` (post it on a channel).
    #[serde(default)]
    pub mode: String,
    /// Channel, as `type.alias`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Recipient on the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Thread to post in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Whether a delivery failure leaves the run successful.
    #[serde(default = "best_effort")]
    pub best_effort: bool,
}

fn best_effort() -> bool {
    true
}

impl Delivery {
    /// Posts the output to `to` on `channel` (`type.alias`).
    pub fn announce(channel: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            mode: "announce".into(),
            channel: Some(channel.into()),
            to: Some(to.into()),
            thread_id: None,
            best_effort: true,
        }
    }
}

/// A job to create. Shell jobs need `command`; agent jobs `prompt` (and
/// `job_type: "agent"`, implied by a prompt).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewCronJob {
    /// Owning agent (must be configured).
    pub agent: String,
    /// Cron expression.
    pub schedule: String,
    /// Name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// IANA time zone of the schedule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
    /// Shell command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// `shell` or `agent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_type: Option<String>,
    /// Agent prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Where the output goes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Delivery>,
    /// `isolated` (default) or `main`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_target: Option<String>,
    /// Model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Tools an agent job may use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Deletes the job after its first run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_after_run: Option<bool>,
    /// Whether an agent job uses memory (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses_memory: Option<bool>,
    /// `wrapped` (default) or `raw`: how a shell job's output is announced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_output_format: Option<String>,
}

/// Changes to a job; `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CronJobPatch {
    /// Owning agent; required when changing a shell job's command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Cron expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// IANA time zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
    /// Removes the time zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_tz: Option<bool>,
    /// Shell command (or an agent job's prompt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Agent prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Pauses (`false`) or resumes the job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Whether an agent job uses memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses_memory: Option<bool>,
    /// `wrapped` or `raw`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_output_format: Option<String>,
}

/// Result of [`CronApi::run`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CronRunResult {
    /// Run status.
    pub status: String,
    /// Job id.
    pub job_id: String,
    /// Whether the job succeeded.
    pub success: bool,
    /// Its output.
    pub output: String,
    /// Duration.
    pub duration_ms: i64,
    /// Start (RFC 3339).
    pub started_at: String,
    /// End (RFC 3339).
    pub finished_at: String,
}

/// A recorded run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CronRun {
    /// Run id.
    pub id: i64,
    /// Job id.
    pub job_id: String,
    /// Start (RFC 3339).
    pub started_at: String,
    /// End (RFC 3339).
    pub finished_at: String,
    /// Status.
    pub status: String,
    /// Output.
    #[serde(default)]
    pub output: Option<String>,
    /// Duration.
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

/// The scheduler settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CronSettings {
    /// Whether the scheduler runs jobs.
    pub enabled: bool,
    /// Whether missed runs run at startup.
    pub catch_up_on_startup: bool,
    /// Runs kept per job.
    pub max_run_history: u32,
}

/// Changes to the scheduler settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CronSettingsPatch {
    /// Whether the scheduler runs jobs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Whether missed runs run at startup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catch_up_on_startup: Option<bool>,
    /// Runs kept per job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_run_history: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_vectors() {
        let vectors = [
            (
                serde_json::json!({ "kind": "cron", "expr": "0 9 * * *", "tz": null }),
                Schedule::Cron {
                    expr: "0 9 * * *".into(),
                    tz: None,
                },
            ),
            (
                serde_json::json!({ "kind": "cron", "expr": "*/5 * * * *", "tz": "America/Bogota" }),
                Schedule::Cron {
                    expr: "*/5 * * * *".into(),
                    tz: Some("America/Bogota".into()),
                },
            ),
            (
                serde_json::json!({ "kind": "at", "at": "2026-10-14T08:00:00Z" }),
                Schedule::At {
                    at: "2026-10-14T08:00:00Z".into(),
                },
            ),
            (
                serde_json::json!({ "kind": "every", "every_ms": 60000 }),
                Schedule::Every { every_ms: 60_000 },
            ),
        ];
        for (json, expected) in vectors {
            assert_eq!(serde_json::from_value::<Schedule>(json).unwrap(), expected);
        }
        assert!(
            serde_json::from_value::<Schedule>(serde_json::json!({ "kind": "weekly" })).is_err()
        );
    }

    #[test]
    fn delivery_vectors() {
        assert_eq!(
            serde_json::to_value(Delivery::announce("whatsapp.main", "+34600000000")).unwrap(),
            serde_json::json!({ "mode": "announce", "channel": "whatsapp.main", "to": "+34600000000", "best_effort": true })
        );
        let lean: Delivery = serde_json::from_value(serde_json::json!({ "mode": "none" })).unwrap();
        assert!(lean.best_effort && lean.channel.is_none());
    }

    #[test]
    fn jobs_decode_from_the_gateways_json() {
        let job: CronJob = serde_json::from_value(serde_json::json!({
            "id": "j1", "expression": "0 9 * * 1-5",
            "schedule": { "kind": "cron", "expr": "0 9 * * 1-5", "tz": "Europe/Madrid" },
            "command": "", "prompt": "Daily summary", "name": "summary",
            "job_type": "agent", "session_target": "isolated", "model": null,
            "agent_alias": "helper", "enabled": true,
            "delivery": { "mode": "announce", "channel": "whatsapp.main", "to": "+34600000000", "best_effort": true },
            "delete_after_run": false, "uses_memory": true, "source": "imperative",
            "shell_output_format": "wrapped",
            "created_at": "2026-10-13T09:00:00Z", "next_run": "2026-10-14T07:00:00Z",
            "last_run": null, "last_status": null, "last_output": null
        }))
        .unwrap();
        assert_eq!(
            job.schedule,
            Schedule::Cron {
                expr: "0 9 * * 1-5".into(),
                tz: Some("Europe/Madrid".into())
            }
        );
        assert_eq!(job.delivery.channel.as_deref(), Some("whatsapp.main"));
        serde_json::to_value(&job).unwrap();
    }
}
