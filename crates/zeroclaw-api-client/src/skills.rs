//! Skill bundles and the skills in them.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/api_skills.rs`. Bundles
//! themselves are configuration (`skill_bundles.<alias>`); this module edits
//! the skills inside them. A skill is a `SKILL.md`: front matter plus a
//! Markdown body.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

/// Skill endpoints, from [`Client::skills`].
#[derive(Debug, Clone, Copy)]
pub struct SkillsApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Skill bundles and skills (`/api/skills`).
    pub fn skills(&self) -> SkillsApi<'_> {
        SkillsApi { client: self }
    }
}

impl SkillsApi<'_> {
    /// `GET /api/skills/bundles`: the skill bundles.
    pub async fn bundles(&self) -> Result<Vec<SkillBundle>> {
        #[derive(Deserialize)]
        struct Bundles {
            bundles: Vec<SkillBundle>,
        }
        let bundles: Bundles = self.client.get(&["api", "skills", "bundles"], &[]).await?;
        Ok(bundles.bundles)
    }

    /// `GET /api/skills/bundles/{bundle}/skills`: the skills of a bundle.
    pub async fn list(&self, bundle: &str) -> Result<Vec<SkillSummary>> {
        #[derive(Deserialize)]
        struct Skills {
            skills: Vec<SkillSummary>,
        }
        let skills: Skills = self
            .client
            .get(&["api", "skills", "bundles", bundle, "skills"], &[])
            .await?;
        Ok(skills.skills)
    }

    /// `POST /api/skills/bundles/{bundle}/skills`: creates a skill (with
    /// `scripts/`, `references/` and `assets/` unless `no_scaffold`).
    pub async fn create(
        &self,
        bundle: &str,
        name: &str,
        frontmatter: &Value,
        body: &str,
        no_scaffold: bool,
    ) -> Result<()> {
        let request = self
            .client
            .request(
                Method::POST,
                &["api", "skills", "bundles", bundle, "skills"],
                &[],
            )?
            .json(&serde_json::json!({
                "name": name,
                "frontmatter": frontmatter,
                "body": body,
                "no_scaffold": no_scaffold,
            }));
        self.client.empty(request).await
    }

    /// `GET /api/skills/bundles/{bundle}/skills/{name}`: a skill.
    pub async fn read(&self, bundle: &str, name: &str) -> Result<Skill> {
        self.client
            .get(&["api", "skills", "bundles", bundle, "skills", name], &[])
            .await
    }

    /// `PUT /api/skills/bundles/{bundle}/skills/{name}`: replaces a skill's
    /// front matter and body.
    pub async fn write(
        &self,
        bundle: &str,
        name: &str,
        frontmatter: &Value,
        body: &str,
    ) -> Result<()> {
        let request = self
            .client
            .request(
                Method::PUT,
                &["api", "skills", "bundles", bundle, "skills", name],
                &[],
            )?
            .json(&serde_json::json!({ "frontmatter": frontmatter, "body": body }));
        self.client.empty(request).await
    }

    /// `DELETE /api/skills/bundles/{bundle}/skills/{name}`: archives a skill,
    /// or deletes it for good with `purge`.
    pub async fn delete(&self, bundle: &str, name: &str, purge: bool) -> Result<()> {
        let purge = if purge { "true" } else { "false" };
        let request = self.client.request(
            Method::DELETE,
            &["api", "skills", "bundles", bundle, "skills", name],
            &[("purge", purge)],
        )?;
        self.client.empty(request).await
    }

    /// `GET /api/skills/slash-option-kinds`: the option types a skill's
    /// slash command may declare.
    pub async fn slash_option_kinds(&self) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct Kinds {
            kinds: Vec<Value>,
        }
        let kinds: Kinds = self
            .client
            .get(&["api", "skills", "slash-option-kinds"], &[])
            .await?;
        Ok(kinds.kinds)
    }
}

/// A skill bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillBundle {
    /// Alias.
    pub alias: String,
    /// Directory.
    pub directory: String,
    /// Skills it includes.
    #[serde(default)]
    pub include: Vec<String>,
    /// Skills it leaves out.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A skill in a listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillSummary {
    /// Bundle.
    pub bundle: String,
    /// Name.
    pub name: String,
    /// Directory.
    pub directory: String,
    /// Front matter (`name`, `description`, `tags`, `slash_options`, …).
    pub frontmatter: Value,
}

/// A skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// Bundle.
    pub bundle: String,
    /// Name.
    pub name: String,
    /// Front matter.
    pub frontmatter: Value,
    /// Markdown body.
    pub body: String,
}
