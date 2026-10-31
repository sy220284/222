#!/usr/bin/env python3
"""Generate a complete, honest 2.1.88 -> Rust source-test worklist.

CI requires no private original files and always inventories all required
source paths. Contributors with their own original workspace + frozen spec
may additionally build every source AST unit into a deterministic test task.
No unchecked path or generated placeholder counts as a passing test.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import sys
import tempfile

from port_workbench import AuditError, current_counts, read_matrix, rust_targets
from source_test_gate import REGISTRY, _jsonl

CRITICAL = ("src/agent", "src/commands", "src/tools", "src/services",
            "src/entrypoints", "src/bridge", "src/sdk", "src/QueryEngine")


def priority(path: str, entry: dict | None, source_units: list) -> int:
    score = 80 if path.startswith(CRITICAL) else 30
    score += min(len(source_units), 50)
    if any(n.get("flags", {}).get("async") for n in source_units):
        score += 15
    if any(n.get("static_metrics", {}).get("抛错数", 0) for n in source_units):
        score += 10
    if entry is not None:
        score -= 30
    return score


def build_worklist(rows: dict, hashes: dict, registration: list,
                   index: dict | None = None) -> tuple[list, dict]:
    """Produce actionable source-linked tasks, never false acceptance."""
    required = {p for p, value in rows.items() if value["status"] != "excluded"}
    entries = {}
    for row in registration:
        path = row.get("path")
        if path not in required or path in entries:
            raise AuditError("原版测试登记引用排除、未知或重复路径: " + str(path))
        if row.get("source_sha256") != hashes[path]:
            raise AuditError("原版测试登记哈希不一致: " + path)
        entries[path] = row
    if set(rows) != set(hashes):
        raise AuditError("源码冻结索引与迁移台账不同步")
    detail = index is not None
    items = []
    for path in sorted(required):
        row, entry = rows[path], entries.get(path)
        nodes = (index or {}).get(path, [])
        # The frozen TypeScript AST specification indexes 1,902 first-party
        # sources. The four vendor originals are separately audited as
        # whole-source units after all 1,906 source hashes are verified.
        if detail and path.startswith("vendor/"):
            nodes = [{
                "source_id": f"{path}:vendor-file-level:1",
                "kind": "VendorFileLevel",
                "name": "complete original vendor source",
                "source_lines": [1, row["source_lines"]],
            }]
        ids = {node["source_id"] for node in nodes}
        linked = set(entry.get("source_ids", [])) if entry else set()
        if detail and not nodes:
            raise AuditError("完整 AST 索引缺少原版必须迁移路径: " + path)
        if detail and not linked.issubset(ids):
            raise AuditError("测试登记存在不存在的原版函数 ID: " + path)
        rust = rust_targets(row["rust_mapping"])
        required_actions = []
        if not rust:
            required_actions.append("逐源码确定 Rust 生产实现入口")
        if row["status"] != "verified":
            required_actions.append("实现原版行为并通过真实 Rust 运行验证")
        if entry is None:
            required_actions.append("创建原版真实输出黄金测试与 Rust 生产适配器")
        else:
            required_actions.append("复核原版全部分支、跨模块集成和端到端行为")
        if detail and (ids - linked):
            required_actions.append(f"补齐 {len(ids-linked)} 个未关联黄金测试的 AST 对象")
        if entry and row["status"] == "verified" and not (ids - linked if detail else True):
            required_actions = ["复核黄金场景实际分支与端到端等价后确认验收"]
        item = {
            "path": path,
            "source_sha256": hashes[path],
            "matrix_status": row["status"],
            "mapped_rust_targets": rust,
            "registered_golden": entry is not None,
            "registered_case_file": entry.get("golden") if entry else None,
            "registered_test_names": [x.get("name") for x in entry.get("tests", [])] if entry else [],
            "ast_index_verified": detail,
            "ast_units": [
                {"id": node["source_id"], "kind": node.get("kind"),
                 "name": node.get("name"), "lines": node.get("source_lines"),
                 "signature": node.get("signature"),
                 "risk_signals": {
                     "async": node.get("flags", {}).get("async", False),
                     "throws": node.get("static_metrics", {}).get("抛错数", 0),
                     "branches": node.get("static_metrics", {}).get("分支节点数", 0),
                     "await": node.get("static_metrics", {}).get("await数", 0),
                 },
                 "golden_linked": node["source_id"] in linked}
                for node in nodes
            ],
            "units_without_golden": len(ids - linked) if detail else None,
            "priority": priority(path, entry, nodes),
            "next_actions": required_actions,
            "accepted_by_factory": False,  # Only actual executed tests and CI can accept.
        }
        items.append(item)
    items.sort(key=lambda item: (-item["priority"], item["path"]))
    counts = Counter(row["matrix_status"] for row in items)
    stats = {
        "schema": "original-source-test-worklist/v1",
        "source_inventory": len(rows),
        "required_paths": len(required),
        "excluded_paths": len(rows)-len(required),
        "golden_registered_paths": len(entries),
        "missing_golden_paths": len(required)-len(entries),
        "verified_matrix_paths": counts["verified"],
        "partial_paths": counts["partial"],
        "unported_paths": counts["unported"],
        "full_ast_verified": detail,
        "indexed_ast_units": sum(len(x["ast_units"]) for x in items) if detail else None,
        "ast_units_without_golden": (
            sum(x["units_without_golden"] for x in items) if detail else None
        ),
        "all_function_tests_built": False,
    }
    return items, stats


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--output", type=Path, default=Path(".port-audit"))
    parser.add_argument("--source-root", type=Path,
                        help="提交者本机不入库的原版 source 根目录")
    parser.add_argument("--spec-archive", type=Path,
                        help="冻结的完整逐行规格 ZIP")
    args = parser.parse_args(argv)
    repo, out = args.repo.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    try:
        rows = read_matrix(repo)
        current_counts(rows)
        baseline = json.loads((repo/"docs/PORT_BASELINE.json").read_text("utf-8"))
        hashes = baseline["source_hashes"]
        registry = _jsonl(repo/REGISTRY)
        index = None
        if bool(args.source_root) != bool(args.spec_archive):
            raise AuditError("生成完整逐函数测试工单必须同时指定私有 --source-root 和 --spec-archive")
        if args.source_root:
            from source_function_index import build_function_index
            from source_parity_gate import (
                pack_workspace_original, verify_original, verify_spec_archive,
                verify_source_lines,
            )
            with tempfile.TemporaryDirectory(prefix="source-test-factory-") as tmp:
                source_zip = Path(tmp)/"original.zip"
                pack_workspace_original(args.source_root.resolve(), repo, source_zip)
                source_errors = verify_original(source_zip, rows, baseline)
                if source_errors:
                    raise AuditError("原版源码冻结哈希核验失败: " + source_errors[0])
                spec_errors, _ = verify_spec_archive(args.spec_archive, baseline)
                if spec_errors:
                    raise AuditError("原版完整规格核验失败: " + spec_errors[0])
                line_errors, _ = verify_source_lines(source_zip, args.spec_archive)
                if line_errors:
                    raise AuditError("原版逐行规格与源码不一致: " + line_errors[0])
                index, issues, _ = build_function_index(args.spec_archive, hashes)
                if issues:
                    raise AuditError("原版 AST 完整索引校验失败: " + issues[0])
        tasks, stats = build_worklist(rows, hashes, registry, index)
        (out/"SOURCE_TEST_WORKLIST.jsonl").write_text(
            "".join(json.dumps(row, ensure_ascii=False, separators=(",",":"))+"\n"
                    for row in tasks), encoding="utf-8")
        (out/"SOURCE_TEST_WORKLIST.json").write_text(
            json.dumps(stats, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
        lines = [
            "# 原版全量源码测试建设清单", "",
            f"保留源码路径：{stats['required_paths']}",
            f"已登记黄金测试：{stats['golden_registered_paths']}",
            f"尚待黄金测试：{stats['missing_golden_paths']}",
            f"已完整原版 AST 索引：{'是' if index is not None else '否（CI 仅能验证冻结路径）'}",
            f"原版 AST 对象：{stats['indexed_ast_units']}",
            f"AST 尚缺黄金关联：{stats['ast_units_without_golden']}",
            "", "## 下一批源码任务（优先级排序）",
        ]
        for task in tasks[:40]:
            lines.append(f"- `{task['path']}`：{'; '.join(task['next_actions'])}")
        lines += ["", "逐路径完整工单、原版函数身份及源码风险信号见 JSONL。",
                  "本工单只提供真实源码定位和待执行测试，不增加任何已验收计数。"]
        (out/"SOURCE_TEST_WORKLIST.md").write_text(
            "\n".join(lines)+"\n", encoding="utf-8")
        print(json.dumps(stats,ensure_ascii=False))
        return 0
    except (AuditError,OSError,ValueError,KeyError,TypeError) as error:
        (out/"SOURCE_TEST_WORKLIST.json").write_text(
            json.dumps({"schema":"original-source-test-worklist/v1","error":str(error)},
                       ensure_ascii=False)+"\n",encoding="utf-8")
        print("原版全量测试建设清单失败："+str(error),file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
