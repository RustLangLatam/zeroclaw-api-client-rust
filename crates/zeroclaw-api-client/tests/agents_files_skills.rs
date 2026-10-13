//! Agent workspaces, personality, shared files, uploads and skills at the
//! HTTP boundary.

mod common;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::ResponseTemplate;
use wiremock::matchers::{body_bytes, body_json, query_param};

#[tokio::test]
async fn edits_an_agents_workspace() {
    let (server, client) = gateway().await;
    expect("GET", "/api/agents/helper/workspace/list")
        .and(query_param("path", "docs"))
        .respond_with(json(json!({ "path": "docs", "entries": [
            { "name": "a.pdf", "kind": "file", "size": 10 }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/agents/helper/workspace/read")
        .and(query_param("path", "docs/a.pdf"))
        .respond_with(json(json!({
            "path": "docs/a.pdf", "size": 10, "is_text": false, "content": "JVBERg==", "encoding": "base64"
        })))
        .mount(&server)
        .await;
    expect("POST", "/api/agents/helper/workspace/mkdir")
        .and(body_json(json!({ "path": "docs" })))
        .respond_with(json(json!({ "created": "docs" })))
        .mount(&server)
        .await;
    expect("POST", "/api/agents/helper/workspace/move")
        .and(body_json(json!({ "from": "a", "to": "b" })))
        .respond_with(json(json!({ "from": "a", "to": "b" })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/agents/helper/workspace/path")
        .and(body_json(json!({ "path": "b" })))
        .respond_with(json(json!({ "removed": "b" })))
        .mount(&server)
        .await;
    expect("GET", "/api/agents/helper/skills")
        .respond_with(json(
            json!({ "agent": "helper", "skills": [{ "name": "s", "origin": "bundle" }] }),
        ))
        .mount(&server)
        .await;

    let agent = client.agent("helper");
    assert_eq!(
        agent.list_files(Some("docs")).await.unwrap().entries[0].size,
        Some(10)
    );
    assert_eq!(
        agent.read_file("docs/a.pdf").await.unwrap().encoding,
        "base64"
    );
    agent.make_dir("docs").await.unwrap();
    agent.move_path("a", "b").await.unwrap();
    agent.remove_path("b").await.unwrap();
    assert_eq!(agent.skills().await.unwrap().skills.len(), 1);
}

#[tokio::test]
async fn reads_and_writes_personality_files() {
    let (server, client) = gateway().await;
    expect("GET", "/api/personality")
        .and(query_param("agent", "helper"))
        .respond_with(json(json!({ "max_chars": 20000, "files": [
            { "filename": "SOUL.md", "exists": true, "size": 3, "mtime_ms": 1 }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/personality/SOUL.md")
        .and(query_param("agent", "helper"))
        .respond_with(json(json!({
            "filename": "SOUL.md", "content": "Hi", "exists": true, "truncated": false, "mtime_ms": 1
        })))
        .mount(&server)
        .await;
    expect("PUT", "/api/personality/SOUL.md")
        .and(query_param("agent", "helper"))
        .and(body_json(
            json!({ "content": "New", "expected_mtime_ms": 1 }),
        ))
        .respond_with(json(json!({ "bytes_written": 3, "mtime_ms": 2 })))
        .mount(&server)
        .await;
    expect("PUT", "/api/personality/USER.md")
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": "personality_disk_drift", "filename": "USER.md",
            "current_content": "x", "current_mtime_ms": 5
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/personality/templates")
        .and(query_param("agent", "helper"))
        .and(query_param("user_name", "Ana"))
        .respond_with(json(
            json!({ "preset": "default", "files": [{ "filename": "USER.md", "content": "Ana" }] }),
        ))
        .mount(&server)
        .await;

    let agent = client.agent("helper");
    assert_eq!(agent.personality_files().await.unwrap().max_chars, 20_000);
    assert_eq!(
        agent
            .personality("SOUL.md")
            .await
            .unwrap()
            .content
            .as_deref(),
        Some("Hi")
    );
    assert_eq!(
        agent
            .write_personality("SOUL.md", "New", Some(1))
            .await
            .unwrap()
            .mtime_ms,
        Some(2)
    );
    let drift = agent
        .write_personality("USER.md", "y", Some(1))
        .await
        .unwrap_err();
    assert_eq!(drift.status(), Some(409));
    assert_eq!(
        agent
            .personality_templates(&[("user_name", "Ana")])
            .await
            .unwrap()
            .preset,
        "default"
    );
}

#[tokio::test]
async fn browses_shared_files_and_uploads_images() {
    let (server, client) = gateway().await;
    expect("GET", "/api/browse")
        .respond_with(json(
            json!({ "path": "", "entries": [{ "name": "x", "kind": "dir" }] }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/api/browse/mkdir")
        .and(body_json(json!({ "path": "x" })))
        .respond_with(json(json!({ "created": "x" })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/browse/rmdir")
        .and(body_json(json!({ "path": "x" })))
        .respond_with(json(json!({ "removed": "x" })))
        .mount(&server)
        .await;
    expect("POST", "/api/upload")
        .and(query_param("agent", "helper"))
        .and(body_bytes(b"\x89PNG".to_vec()))
        .respond_with(json(
            json!({ "path": "/w/webui_1.png", "marker": "[IMAGE:/w/webui_1.png]" }),
        ))
        .mount(&server)
        .await;

    let files = client.files();
    assert_eq!(files.list(None).await.unwrap().entries[0].kind, "dir");
    files.make_dir("x").await.unwrap();
    files.remove_dir("x").await.unwrap();
    let upload = files
        .upload_image("helper", b"\x89PNG".to_vec())
        .await
        .unwrap();
    assert_eq!(upload.marker, "[IMAGE:/w/webui_1.png]");
}

#[tokio::test]
async fn manages_skills_in_bundles() {
    let (server, client) = gateway().await;
    let frontmatter = json!({ "name": "greet", "description": "Greets" });
    expect("GET", "/api/skills/bundles")
        .respond_with(json(json!({ "bundles": [
            { "alias": "team", "directory": "/s/team", "include": [], "exclude": [] }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/skills/bundles/team/skills")
        .respond_with(json(json!({ "skills": [
            { "bundle": "team", "name": "greet", "directory": "/s/team/greet", "frontmatter": frontmatter }
        ] })))
        .mount(&server)
        .await;
    expect("POST", "/api/skills/bundles/team/skills")
        .and(body_json(
            json!({ "name": "greet", "frontmatter": frontmatter, "body": "", "no_scaffold": true }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(
            json!({ "bundle": "team", "name": "greet", "directory": "/s/team/greet" }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/api/skills/bundles/team/skills/greet")
        .respond_with(json(json!({ "bundle": "team", "name": "greet", "frontmatter": frontmatter, "body": "# Greet" })))
        .mount(&server)
        .await;
    expect("PUT", "/api/skills/bundles/team/skills/greet")
        .and(body_json(
            json!({ "frontmatter": frontmatter, "body": "# Hi" }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    expect("DELETE", "/api/skills/bundles/team/skills/greet")
        .and(query_param("purge", "true"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    expect("GET", "/api/skills/slash-option-kinds")
        .respond_with(json(json!({ "kinds": [{ "manifest_name": "string" }] })))
        .mount(&server)
        .await;

    let skills = client.skills();
    assert_eq!(skills.bundles().await.unwrap()[0].alias, "team");
    assert_eq!(skills.list("team").await.unwrap()[0].name, "greet");
    skills
        .create("team", "greet", &frontmatter, "", true)
        .await
        .unwrap();
    assert_eq!(skills.read("team", "greet").await.unwrap().body, "# Greet");
    skills
        .write("team", "greet", &frontmatter, "# Hi")
        .await
        .unwrap();
    skills.delete("team", "greet", true).await.unwrap();
    assert_eq!(skills.slash_option_kinds().await.unwrap().len(), 1);
}
