#!/usr/bin/env python3
"""Compares the gateway's routes with the ones the client calls.

Reads every `.route("/path", get(..).post(..))` in a ZeroClaw checkout's
`crates/zeroclaw-gateway/src` and the endpoint map
(`docs/book/src/reference/endpoints.md`). Fails when the gateway has a route
the client neither calls nor deliberately skips, or when the client calls a
route the gateway no longer has.

    python3 scripts/route-drift.py <zeroclaw checkout>

Feature-gated routes (`webauthn`, `a2a`, `plugins-wasm`, ...) are read
whatever the build's features: the client covers every route the source
declares.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MAP = ROOT / "docs/book/src/reference/endpoints.md"

# Routes the client deliberately does not call; the endpoint map lists why.
SKIPPED = {
    "/whatsapp", "/whatsapp/{}", "/linq", "/linq/{}", "/nextcloud-talk",
    "/nextcloud-talk/{}", "/webhook/gmail", "/hooks/claude-code", "/sop/{}",
    "/api/plugins", "/_app/", "/_app/{}", "/api/docs", "/acp", "/ws/nodes",
    # Aliases of routes the client calls under another spelling.
    "/a2a/.well-known/agents-card.json", "GET /api/doctor",
}

VERBS = r"(?<![\w])(?:axum::routing::)?(get|post|put|delete|patch|options)(?:_service)?\s*\("


def normalize(path):
    return re.sub(r"\{[^}]+\}", "{}", path)


def gateway_routes(src):
    routes = set()
    for file in sorted(src.glob("**/*.rs")):
        if "tests" in file.parts:
            continue
        # ponytail: test modules sit at the end of each file; a test module
        # placed mid-file would hide the routes after it.
        text = file.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        consts = dict(re.findall(r'const (\w+): &str = "([^"]+)"', text))
        for m in re.finditer(r'\.route\(\s*("(?:[^"\\]|\\.)*"|&format!\("[^"]*"\)|\w+)\s*,', text):
            raw = m.group(1)
            if raw.startswith("&format!"):
                path = re.sub(r"(?<!\{)\{(\w+)\}(?!\})", lambda c: consts.get(c.group(1), c.group(0)), raw[10:-2])
                path = path.replace("{{", "{").replace("}}", "}")
                if "{prefix}" in path:  # the optional path-prefix redirect
                    continue
            elif raw.startswith('"'):
                path = raw[1:-1]
            else:
                path = consts.get(raw)
                if path is None:
                    sys.exit(f"{file}: cannot resolve route constant {raw}")
            end, depth = m.end(), 1
            while depth:
                depth += {"(": 1, ")": -1}.get(text[end], 0)
                end += 1
            for verb in re.findall(VERBS, text[m.end():end]):
                routes.add((verb.upper(), normalize(path)))
    return routes


def client_routes():
    rows = re.findall(r"^\| `(\w+)` \| `([^`]+)`", MAP.read_text(encoding="utf-8"), re.M)
    return {(verb, normalize(path)) for verb, path in rows}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    src = pathlib.Path(sys.argv[1]) / "crates/zeroclaw-gateway/src"
    gateway = gateway_routes(src)
    client = client_routes()
    missing = sorted(r for r in gateway - client if r[1] not in SKIPPED and f"{r[0]} {r[1]}" not in SKIPPED)
    gone = sorted(client - gateway)
    for verb, path in missing:
        print(f"not called by the client: {verb} {path}")
    for verb, path in gone:
        print(f"not in the gateway:       {verb} {path}")
    print(f"{len(gateway)} gateway routes, {len(client)} called, "
          f"{len(missing)} uncovered, {len(gone)} stale")
    sys.exit(1 if missing or gone else 0)


if __name__ == "__main__":
    main()
