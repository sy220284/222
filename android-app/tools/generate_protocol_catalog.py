#!/usr/bin/env python3
"""Extract and verify Codex App Server request coverage from the upstream Rust source."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "codex-rs/app-server-protocol/src/protocol/common.rs"
ASSETS = ROOT / "android-app/app/src/main/assets"
UPSTREAM = "67f0994b532b8740f14b5b738af7b7b2a915b1d6"

REQUEST_RE = re.compile(
    r'^\s*(\w+)\s*=>\s*"([^"]+)"\s*\{([\s\S]*?)^\s*\},',
    re.MULTILINE,
)
TYPE_RE = re.compile(r'\bparams\s*:\s*(?:#\[[^\]]+\]\s*)?(v1|v2)::([A-Za-z]\w+)')


def extract() -> tuple[dict, dict]:
    source = SOURCE.read_text(encoding="utf-8")
    begin = source.index("client_request_definitions! {")
    end = source.index("server_request_definitions! {", begin)
    definitions = []
    groups: dict[str, list[str]] = {}
    for match in REQUEST_RE.finditer(source, begin, end):
        method = match.group(2)
        params = TYPE_RE.search(match.group(3))
        definitions.append({
            "method": method,
            "paramsType": params.group(2) if params else None,
            "namespace": params.group(1) if params else None,
        })
        groups.setdefault(method.split("/")[0], []).append(method)
    if len(definitions) < 170:
        raise RuntimeError(f"Codex 方法解析不足：只读取 {len(definitions)} 项")
    names = [x["method"] for x in definitions]
    if len(names) != len(set(names)):
        raise RuntimeError("存在重复方法名，请复核上游协议")
    return (
        {
            "origin": "codex-rs/app-server-protocol/src/protocol/common.rs",
            "upstream": UPSTREAM,
            "total": len(definitions),
            "groups": groups,
        },
        {
            "source": "codex-rs/app-server-protocol/src/protocol/common.rs",
            "upstream": UPSTREAM,
            "methods": definitions,
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="check generated assets without changes")
    args = parser.parse_args()
    catalog, schemas = extract()
    targets = {
        ASSETS / "rpc_methods.json": catalog,
        ASSETS / "rpc_schemas.json": schemas,
    }
    failed = []
    for path, expected in targets.items():
        if args.check:
            if not path.is_file() or json.loads(path.read_text(encoding="utf-8")) != expected:
                failed.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                json.dumps(expected, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
    if failed:
        raise SystemExit("RPC 协议目录与上游源码不同步：" + ", ".join(failed))
    print(f"Codex App Server 已映射 {catalog['total']} 个请求方法")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
