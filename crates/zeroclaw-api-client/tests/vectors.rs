//! Response vectors: answers captured from a real gateway
//! (`scripts/capture-vectors.py`, anonymized) must decode into the SDK's
//! types, and re-encode to an equivalent value.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use zeroclaw_api_client::{
    agents, channels, config, cost, cron, events, pairing, sessions, skills, system,
};

/// Decodes `value` as `T`, then checks that encoding it back and decoding
/// again gives the same `T`: no field is lost on the way.
fn decodes<T>(value: Value) -> Result<(), String>
where
    T: DeserializeOwned + Serialize + PartialEq + std::fmt::Debug,
{
    let decoded: T = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let again: T =
        serde_json::from_value(serde_json::to_value(&decoded).map_err(|e| e.to_string())?)
            .map_err(|e| format!("re-decoding failed: {e}"))?;
    if decoded != again {
        return Err("re-encoding changed the value".into());
    }
    Ok(())
}

type Check = fn(Value) -> Result<(), String>;

/// Every vector name, the envelope field the SDK unwraps (if any), and the
/// type that field decodes into.
fn table() -> BTreeMap<&'static str, (Option<&'static str>, Check)> {
    BTreeMap::from([
        ("health", (None, decodes::<system::Health> as Check)),
        (
            "runtime_health",
            (Some("health"), decodes::<system::HealthSnapshot>),
        ),
        ("status", (None, decodes::<system::Status>)),
        (
            "channels",
            (Some("channels"), decodes::<Vec<channels::Channel>>),
        ),
        ("config_list", (None, decodes::<config::ConfigList>)),
        (
            "config_drift",
            (Some("drifted"), decodes::<Vec<config::DriftEntry>>),
        ),
        (
            "config_reload_status",
            (Some("pending_reload"), decodes::<bool>),
        ),
        (
            "config_templates",
            (Some("templates"), decodes::<Vec<config::TemplateEntry>>),
        ),
        ("config_map_keys", (Some("keys"), decodes::<Vec<String>>)),
        (
            "config_catalog",
            (
                Some("model_providers"),
                decodes::<Vec<config::CatalogModelProvider>>,
            ),
        ),
        ("config_status", (None, decodes::<config::ConfigStatus>)),
        (
            "config_agent_options",
            (None, decodes::<config::AgentOptions>),
        ),
        (
            "config_sections",
            (Some("sections"), decodes::<Vec<config::ConfigSection>>),
        ),
        ("cron_list", (Some("jobs"), decodes::<Vec<cron::CronJob>>)),
        ("cron_settings", (None, decodes::<cron::CronSettings>)),
        ("cost", (Some("cost"), decodes::<cost::CostSummary>)),
        (
            "sessions",
            (Some("sessions"), decodes::<Vec<sessions::Session>>),
        ),
        (
            "events_history",
            (Some("events"), decodes::<Vec<events::Event>>),
        ),
        ("tools", (Some("tools"), decodes::<Vec<system::ToolSpec>>)),
        (
            "integrations",
            (Some("integrations"), decodes::<Vec<system::Integration>>),
        ),
        (
            "integration_settings",
            (
                Some("settings"),
                decodes::<BTreeMap<String, system::IntegrationSetting>>,
            ),
        ),
        (
            "skill_bundles",
            (Some("bundles"), decodes::<Vec<skills::SkillBundle>>),
        ),
        ("slash_option_kinds", (Some("kinds"), decodes::<Vec<Value>>)),
        ("quickstart_state", (None, decodes::<Value>)),
        ("tuis", (Some("tuis"), decodes::<Vec<system::Tui>>)),
        (
            "devices",
            (Some("devices"), decodes::<Vec<pairing::Device>>),
        ),
        (
            "personality_files",
            (None, decodes::<agents::PersonalityFiles>),
        ),
        ("agent_skills", (None, decodes::<agents::AgentSkills>)),
    ])
}

#[test]
fn every_captured_answer_decodes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let table = table();
    let mut checked = 0;
    let mut failures = vec![];
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let vector: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let Some((field, check)) = table.get(name.as_str()) else {
            failures.push(format!(
                "{name}: no type for this vector; add it to the table"
            ));
            continue;
        };
        let response = vector["response"].clone();
        let value = match field {
            Some(field) => response[field].clone(),
            None => response,
        };
        if let Err(e) = check(value) {
            failures.push(format!("{name} ({}): {e}", vector["route"]));
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        checked >= 20,
        "only {checked} vectors found in {}",
        dir.display()
    );
}

/// Every string in `value`.
fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.iter().for_each(|(k, v)| {
            out.push(k.clone());
            strings(v, out);
        }),
        _ => {}
    }
}

#[test]
fn anonymization_leaves_the_wire_format_intact() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let vector: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut all = vec![];
        strings(&vector, &mut all);
        for text in all {
            // A placeholder glued into a timestamp or a UUID means the
            // anonymizer rewrote something that was not a phone number.
            for artifact in [".+34", "-+34", ":+34", "T+34"] {
                assert!(!text.contains(artifact), "{path:?}: {text}");
            }
        }
    }
}

#[test]
fn vectors_record_their_route_and_build() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let vector: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            vector["route"].as_str().is_some_and(|r| r.starts_with('/')),
            "{path:?}"
        );
        assert!(vector["build"].is_string(), "{path:?}");
    }
}
