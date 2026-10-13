#!/usr/bin/env python3
"""The endpoint map of docs/book/src/reference/endpoints.md, built from the
`METHOD /path` that opens each public method's doc comment.

    python3 scripts/endpoint-map.py           # print the table
    python3 scripts/endpoint-map.py --write   # replace the table in the page
    python3 scripts/endpoint-map.py --check   # fail if the page is stale (CI)
"""
import glob
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PAGE = ROOT / "docs/book/src/reference/endpoints.md"
HEADER = "| Method | Path | Module | Function |"


def table():
    rows = []
    for file in sorted(glob.glob(str(ROOT / "crates/zeroclaw-api-client/src/*.rs"))):
        source = pathlib.Path(file).read_text(encoding="utf-8")
        module = pathlib.Path(file).stem
        for match in re.finditer(r"((?:\s*///[^\n]*\n)+)\s*pub async fn (\w+)", source):
            route = re.search(r"`(GET|POST|PUT|PATCH|DELETE|OPTIONS) (/[^`?]*)", match.group(1))
            if route:
                rows.append((route.group(2), route.group(1), module, match.group(2)))
    lines = [HEADER, "|---|---|---|---|"]
    lines += [f"| `{m}` | `{p}` | `{mod}` | `{f}` |" for p, m, mod, f in sorted(rows)]
    return "\n".join(lines) + "\n"


def page_with(new_table):
    text = PAGE.read_text(encoding="utf-8")
    return text[: text.index(HEADER)] + new_table


if __name__ == "__main__":
    generated = table()
    if "--write" in sys.argv:
        PAGE.write_text(page_with(generated), encoding="utf-8")
    elif "--check" in sys.argv:
        if PAGE.read_text(encoding="utf-8") != page_with(generated):
            sys.exit("docs/book/src/reference/endpoints.md is stale: run scripts/endpoint-map.py --write")
    else:
        sys.stdout.write(generated)
