#!/usr/bin/env python3
"""Reproducible Codex mobile feature coverage audit.

This is an interface and source-code audit, not a runtime success certificate.
Calls inside mobile UI are divided into dedicated flows and generic RPC access.
"""
from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "android-app/app/src/main/assets/rpc_methods.json"
PARAMS = ROOT / "android-app/app/src/main/assets/rpc_schemas.json"
STATE = ROOT / "android-app/app/src/main/java/com/codex/mobile/MobileState.kt"
UI = ROOT / "android-app/app/src/main/java/com/codex/mobile/MainActivity.kt"
ENGINE = ROOT / "android-app/app/src/main/java/com/codex/mobile/CodexRpcEngine.java"

# Windows sandbox administration is OS-specific, not an Android user workflow.
DESKTOP_ONLY_PREFIXES = ("windowsSandbox/",)
MINIMUM_DIRECT = {
    "thread/list", "thread/start", "thread/resume", "thread/read",
    "turn/start", "turn/interrupt", "model/list", "account/login/start",
}
APPROVAL_EVENTS = (
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "mcpServer/elicitation/request",
    "item/permissions/requestApproval",
)


def main() -> int:
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    params = json.loads(PARAMS.read_text(encoding="utf-8"))
    all_methods = [method for methods in catalog["groups"].values() for method in methods]
    schema_methods = [item["method"] for item in params["methods"]]
    if set(all_methods) != set(schema_methods):
        missing = set(schema_methods) - set(all_methods)
        extra = set(all_methods) - set(schema_methods)
        raise SystemExit(f"Source mismatch: missing={sorted(missing)}, extra={sorted(extra)}")
    if len(all_methods) != len(set(all_methods)):
        raise SystemExit("Duplicate Codex method names in mobile catalog")
    state = STATE.read_text(encoding="utf-8")
    ui = UI.read_text(encoding="utf-8")
    engine = ENGINE.read_text(encoding="utf-8")
    direct = set(re.findall(r'\b(?:ask|request)\s*\(\s*"([^"]+)"', state))
    required_missing = MINIMUM_DIRECT - direct
    if required_missing:
        raise SystemExit("Critical mobile flows not wired: " + str(sorted(required_missing)))
    if "invokeMethod(" not in state or "RpcSchemaForm(" not in ui:
        raise SystemExit("Generic schema-driven feature entrypoint missing")
    if '"app-server"' in engine and '"--listen", "stdio://"' not in engine:
        raise SystemExit("App-server transport is not wired for local stdio")
    if any(x not in state for x in APPROVAL_EVENTS):
        raise SystemExit("Safety review requires an explicit approval handler")
    if not (ROOT / ".github/scripts/android_smoke.py").exists():
        raise SystemExit("Android 17 device verification script missing")

    device_only = [x for x in all_methods if x.startswith(DESKTOP_ONLY_PREFIXES)]
    dedicated = sorted(direct & set(all_methods))
    generic = [x for x in all_methods if x not in direct and x not in device_only]
    print("Codex Android feature coverage – source analysis only")
    print("  Total upstream RPC requests: ", len(all_methods))
    print("  Dedicated mobile flows:       ", len(dedicated))
    print("  Schema-driven RPC access:     ", len(generic))
    print("  OS-specific desktop methods:  ", len(device_only))
    print("  Device/runtime verified:      ", "not determined by this tool")
    print("Direct features:")
    for method in dedicated:
        print("  +", method)
    print("Largest unfinished UI categories:")
    remaining = Counter(method.split("/")[0] for method in generic)
    for name, count in remaining.most_common(15):
        print(f"  {name:<24}{count:>3} methods")
    print("Note: An RPC entrypoint is not proof that a capability works on Android.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (KeyError, ValueError, OSError) as error:
        print("Parity audit failed:", error, file=sys.stderr)
        sys.exit(1)
