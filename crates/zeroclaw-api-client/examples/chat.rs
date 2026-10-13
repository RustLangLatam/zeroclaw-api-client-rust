//! Asks an agent one question and streams its reply.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example chat -- helper "Hello"
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;
use zeroclaw_api_client::chat::{ChatFrame, ChatOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(agent), Some(question)) = (args.next(), args.next()) else {
        return Err("usage: chat <agent> <question>".into());
    };
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::new(url, std::env::var("ZEROCLAW_TOKEN")?)?;

    let mut chat = client.chat(&agent, ChatOptions::default()).await?;
    chat.send(&question).await?;
    let mut out = std::io::stdout().lock();
    while let Some(frame) = chat.next_frame().await? {
        match frame {
            ChatFrame::Chunk(text) => {
                write!(out, "{text}")?;
                out.flush()?;
            }
            ChatFrame::ToolCall { name, .. } => writeln!(out, "\n[tool: {name}]")?,
            ChatFrame::Done(result) => {
                writeln!(out, "\n— {} tokens", result.tokens_used.unwrap_or_default())?;
                break;
            }
            ChatFrame::Error { message, .. } => return Err(message.into()),
            _ => {}
        }
    }
    chat.close().await?;
    Ok(())
}
