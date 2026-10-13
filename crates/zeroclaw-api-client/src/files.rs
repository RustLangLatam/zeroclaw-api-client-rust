//! The shared directory (`<install>/shared/`) and image uploads.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api_browse.rs` and
//! `api_upload.rs`. Per-agent workspaces are in [`crate::agents`].

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// Shared-directory and upload endpoints, from [`Client::files`].
#[derive(Debug, Clone, Copy)]
pub struct FilesApi<'a> {
    client: &'a Client,
}

impl Client {
    /// The shared directory and uploads.
    pub fn files(&self) -> FilesApi<'_> {
        FilesApi { client: self }
    }
}

impl FilesApi<'_> {
    /// `GET /api/browse`: a directory of the shared area (its root without
    /// `path`).
    pub async fn list(&self, path: Option<&str>) -> Result<Listing> {
        let query: Vec<(&str, &str)> = path.map(|p| ("path", p)).into_iter().collect();
        self.client.get(&["api", "browse"], &query).await
    }

    /// `POST /api/browse/mkdir`: creates a directory.
    pub async fn make_dir(&self, path: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "browse", "mkdir"], &[])?
            .json(&serde_json::json!({ "path": path }));
        self.client.empty(request).await
    }

    /// `DELETE /api/browse/rmdir`: removes a directory and its contents.
    pub async fn remove_dir(&self, path: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "browse", "rmdir"], &[])?
            .json(&serde_json::json!({ "path": path }));
        self.client.empty(request).await
    }

    /// `POST /api/upload`: stores an image (PNG, JPEG, GIF or WebP, told by
    /// its bytes) in `agent`'s workspace and returns the marker that attaches
    /// it to a chat message.
    pub async fn upload_image(&self, agent: &str, bytes: Vec<u8>) -> Result<Upload> {
        let request = self
            .client
            .request(Method::POST, &["api", "upload"], &[("agent", agent)])?
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes);
        self.client.json(request).await
    }
}

/// A directory's entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    /// The directory.
    pub path: String,
    /// Directories first, then files, alphabetically.
    pub entries: Vec<Entry>,
}

/// A directory entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Name.
    pub name: String,
    /// `dir` or `file`.
    pub kind: String,
    /// Size of a file.
    #[serde(default)]
    pub size: Option<u64>,
    /// Whether it may not be changed.
    #[serde(default)]
    pub protected: bool,
}

/// A file's content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadFile {
    /// The file.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Whether it is text.
    pub is_text: bool,
    /// Content, in `encoding`.
    pub content: String,
    /// `utf8` or `base64`.
    pub encoding: String,
}

/// A stored upload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Upload {
    /// Where it was stored.
    pub path: String,
    /// `[IMAGE:<path>]`, to put in a message.
    pub marker: String,
}
