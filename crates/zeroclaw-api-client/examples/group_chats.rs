//! Lists every channel, the group chats it takes part in and their members.
//! Read-only.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example group_chats
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::new(url, std::env::var("ZEROCLAW_TOKEN")?)?;
    let channels = client.channels();
    let mut out = std::io::stdout().lock();

    for channel in channels.list().await? {
        writeln!(
            out,
            "{} ({}, {})",
            channel.name, channel.status, channel.health
        )?;
        let rooms = match channels.rooms(&channel.name).await {
            Ok(rooms) => rooms,
            // Not running, or a build without the route: say why and go on.
            Err(e) => {
                writeln!(out, "  rooms unavailable: {e}")?;
                continue;
            }
        };
        for room in rooms {
            writeln!(
                out,
                "  {} — {} ({} members)",
                room.name, room.id, room.participants
            )?;
            for member in channels.room_members(&channel.name, &room.id).await? {
                let role = if member.admin { "admin" } else { "member" };
                writeln!(out, "    {:<20} {role}", member.id)?;
            }
        }
    }
    Ok(())
}
