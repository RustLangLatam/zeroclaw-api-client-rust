#!/usr/bin/env python3
"""Captures response test vectors from a running gateway.

Each read-only route below is called and its answer saved, anonymized, to
crates/zeroclaw-api-client/tests/vectors/<name>.json as
{"route": ..., "build": ..., "response": ...}. tests/vectors.rs then checks
that every vector decodes into its type.

    ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… \\
      [ZEROCLAW_AGENT=<alias>] [ZEROCLAW_REDACT=word,word] \
      python3 scripts/capture-vectors.py

ZEROCLAW_REDACT lists words of your own deployment (provider aliases,
business names) to replace with "example". MCP tools (`<server>__<tool>`)
are left out: they belong to whoever serves them.

Routes whose answers hold people's words (logs, memory, transcripts) are not
captured. Review the files before committing them.
"""
import json
import os
import re
import urllib.request

URL = os.environ["ZEROCLAW_URL"].rstrip("/")
TOKEN = os.environ["ZEROCLAW_TOKEN"]
AGENT = os.environ.get("ZEROCLAW_AGENT")
REDACT = [w for w in os.environ.get("ZEROCLAW_REDACT", "").split(",") if w]
OUT = "crates/zeroclaw-api-client/tests/vectors"

ROUTES = {
    "health": "/health",
    "runtime_health": "/api/health",
    "status": "/api/status",
    "channels": "/api/channels",
    "config_list": "/api/config/list?prefix=gateway",
    "config_drift": "/api/config/drift",
    "config_reload_status": "/api/config/reload-status",
    "config_templates": "/api/config/templates",
    "config_map_keys": "/api/config/map-keys?path=agents",
    "config_catalog": "/api/config/catalog",
    "config_status": "/api/config/status",
    "config_agent_options": "/api/config/agent-options",
    "config_sections": "/api/config/sections",
    "cron_list": "/api/cron",
    "cron_settings": "/api/cron/settings",
    "cost": "/api/cost",
    "sessions": "/api/sessions",
    "events_history": "/api/events/history",
    "tools": "/api/tools",
    "integrations": "/api/integrations",
    "integration_settings": "/api/integrations/settings",
    "skill_bundles": "/api/skills/bundles",
    "slash_option_kinds": "/api/skills/slash-option-kinds",
    "quickstart_state": "/api/quickstart/state",
    "tuis": "/api/tuis",
    "devices": "/api/devices",
}
if AGENT:
    ROUTES["personality_files"] = f"/api/personality?agent={AGENT}"
    ROUTES["agent_skills"] = f"/api/agents/{AGENT}/skills"

# Most items kept per list: vectors check shapes, not volume.
MAX_ITEMS = 5

# Applied in order. Ids first, so the phone rule never sees their digits;
# the phone rule skips digits right after ".", ":" or "-" (fractions of a
# second, UUID tails) but not after "_" (session keys carry the number).
SECRETS = [(re.compile(re.escape(word), re.IGNORECASE), "example") for word in REDACT] + [
    (re.compile(r"zc_[0-9a-f]{64}"), "zc_" + "0" * 64),
    (re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"),
     "00000000-0000-0000-0000-000000000000"),
    (re.compile(r"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])"), "0" * 32),
    (re.compile(r"\b\d{1,3}(\.\d{1,3}){3}\b"), "192.0.2.1"),
    (re.compile(r"[\w.+-]+@[\w-]+\.[\w.]+"), "someone@example.com"),
    (re.compile(r"(?<![\d.:-])\+?\d{9,15}(?![\d:])"), "+34600000000"),
]


def anonymize(value):
    if isinstance(value, str):
        for pattern, replacement in SECRETS:
            value = pattern.sub(replacement, value)
        return value
    if isinstance(value, list):
        return [anonymize(v) for v in value[:MAX_ITEMS]]
    if isinstance(value, dict):
        items = list(value.items())[: MAX_ITEMS * 4]
        return {anonymize(k): anonymize(v) for k, v in items}
    return value


def get(route):
    request = urllib.request.Request(URL + route, headers={"Authorization": f"Bearer {TOKEN}"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


build = get("/api/status").get("version", "unknown")
os.makedirs(OUT, exist_ok=True)
for name, route in ROUTES.items():
    try:
        response = get(route)
        if name == "tools":
            response["tools"] = [t for t in response["tools"] if "__" not in t["name"]]
        response = anonymize(response)
    except Exception as error:  # a route this build or setup does not offer
        print(f"skip  {name}: {error}")
        continue
    route = anonymize(route)
    with open(f"{OUT}/{name}.json", "w", encoding="utf-8") as file:
        json.dump({"route": route, "build": build, "response": response}, file, indent=1, ensure_ascii=False)
        file.write("\n")
    print(f"saved {name}")
