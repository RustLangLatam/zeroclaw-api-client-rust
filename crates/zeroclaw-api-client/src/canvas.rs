//! Canvases: HTML, SVG, Markdown or text frames an agent (or a client)
//! publishes for display.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/canvas.rs`. Limits: 100
//! canvases, 50 frames of history each, 256 KiB per frame (the gateway's
//! 64 KiB request limit applies first).

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// Canvas endpoints, from [`Client::canvas`].
#[derive(Debug, Clone, Copy)]
pub struct CanvasApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Canvases (`/api/canvas`).
    pub fn canvas(&self) -> CanvasApi<'_> {
        CanvasApi { client: self }
    }
}

impl CanvasApi<'_> {
    /// `GET /api/canvas`: the canvas ids.
    pub async fn list(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Canvases {
            canvases: Vec<String>,
        }
        let canvases: Canvases = self.client.get(&["api", "canvas"], &[]).await?;
        Ok(canvases.canvases)
    }

    /// `GET /api/canvas/{id}`: the current frame.
    pub async fn current(&self, id: &str) -> Result<CanvasFrame> {
        let framed: Framed = self.client.get(&["api", "canvas", id], &[]).await?;
        Ok(framed.frame)
    }

    /// `POST /api/canvas/{id}`: publishes a frame (`content_type`: `html`,
    /// `svg`, `markdown` or `text`), creating the canvas.
    pub async fn publish(
        &self,
        id: &str,
        content_type: &str,
        content: &str,
    ) -> Result<CanvasFrame> {
        let framed: Framed = self
            .client
            .send_json(
                Method::POST,
                &["api", "canvas", id],
                &[],
                &serde_json::json!({ "content_type": content_type, "content": content }),
            )
            .await?;
        Ok(framed.frame)
    }

    /// `GET /api/canvas/{id}/history`: past frames, oldest first.
    pub async fn history(&self, id: &str) -> Result<Vec<CanvasFrame>> {
        #[derive(Deserialize)]
        struct History {
            frames: Vec<CanvasFrame>,
        }
        let history: History = self
            .client
            .get(&["api", "canvas", id, "history"], &[])
            .await?;
        Ok(history.frames)
    }

    /// `GET /ws/canvas/{id}`: follows a canvas live. Frames are
    /// `{"type": "connected" | "frame" | "lagged" | "error", …}`; a `frame`
    /// carries the published `frame`.
    pub async fn watch(&self, id: &str) -> Result<crate::chat::JsonFeed> {
        let socket = crate::chat::connect(self.client, &["ws", "canvas", id], &[]).await?;
        Ok(crate::chat::json_frames(socket))
    }

    /// `DELETE /api/canvas/{id}`: clears a canvas.
    pub async fn clear(&self, id: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "canvas", id], &[])?;
        self.client.empty(request).await
    }
}

#[derive(Deserialize)]
struct Framed {
    frame: CanvasFrame,
}

/// A canvas frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasFrame {
    /// Frame id.
    pub frame_id: String,
    /// `html`, `svg`, `markdown` or `text`.
    pub content_type: String,
    /// Content.
    pub content: String,
    /// When it was published.
    pub timestamp: String,
}
