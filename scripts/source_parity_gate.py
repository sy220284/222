#!/usr/bin/env python3
"""One source-parity gate for the 2.1.88 original and the Rust port.

A file mapping or successful compilation never implies behavioral parity.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from zipfile import ZipFile, BadZipFile
from port_workbench import AuditError, check_evidence, current_counts, read_matrix, rust_targets

DIMENSIONS = ("inputs", "outputs", "logic", "errors", "permissions", "state",
              "ordering", "cancellation", "persistence", "side_effects",
              "runtime_call_chain", "test_cases")
CONTRACTS = Path("docs/SOURCE_PARITY_CONTRACTS.jsonl")


def original_entries(archive: Path):
    """Read in bounded blocks without extracting the archive or trusting its paths."""
    results = {}
    with ZipFile(archive) as z:
        for info in z.infolist():
            if info.is_dir():
                continue
            name = info.filename.replace("\\", "/")
            parts = name.split("/")
            if ".." in parts or name.startswith("/"):
                raise AuditError("原版 ZIP 包含非法路径: " + name)
            root = next((i for i in range(len(parts) - 2)
                         if parts[i] == "source" and parts[i+1] in ("src", "vendor")), None)
            if root is None:
                continue
            path = "/".join(parts[root+1:])
            if path in results:
                raise AuditError("原版文件重复: " + path)
            if info.file_size > 30 * 1024 * 1024:
                raise AuditError("原版单文件过大: " + path)
            digest = hashlib.sha256()
            size, lines = 0, 1
            with z.open(info) as f:
                while block := f.read(131072):
                    digest.update(block)
                    size += len(block)
                    lines += block.count(b"\n")
            results[path] = (digest.hexdigest(), size, lines)
    return results


def verify_original(archive: Path, rows: dict, baseline: dict):
    if not archive.is_file():
        return ["原版压缩包不存在: " + str(archive)]
    actual = original_entries(archive)
    expected = baseline["source_hashes"]
    errors = ["原版源码缺失: " + p for p in sorted(set(expected) - set(actual))]
    errors += ["原版源码多出: " + p for p in sorted(set(actual) - set(expected))]
    for path in sorted(set(actual) & set(expected)):
        sha, size, lines = actual[path]
        if sha != expected[path]:
            errors.append("原版 SHA256 不一致: " + path)
        if (size, lines) != (rows[path]["source_bytes"], rows[path]["source_lines"]):
            errors.append("原版大小或行数不一致: " + path)
    return errors


def load_contracts(path: Path):
    items, errors = {}, []
    if not path.is_file():
        return {}, ["未建立唯一函数行为契约文件: " + str(path)]
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            item = json.loads(line)
            path_key = item["path"]
            if not isinstance(path_key, str) or path_key in items:
                raise ValueError("路径非法或重复")
            items[path_key] = item
        except (ValueError, TypeError, KeyError) as exc:
            errors.append(str(path) + ":" + str(number) + ": " + str(exc))
    return items, errors


def verify_contracts(rows, baseline, contracts):
    eligible = {p for p,r in rows.items() if r["status"] != "excluded"}
    errors = ["无合同的原版路径: " + p for p in sorted(eligible - set(contracts))]
    errors += ["无效范围的合同: " + p for p in sorted(set(contracts) - eligible)]
    verified = 0
    for path in sorted(eligible & set(contracts)):
        row, item = rows[path], contracts[path]
        issues = []
        if item.get("source_sha256") != baseline["source_hashes"][path]:
            issues.append("原版 SHA 不一致")
        if row["status"] != "verified" or item.get("review_status") != "verified":
            issues.append("尚未验收")
        if item.get("unresolved") != []:
            issues.append("仍有行为差异")
        targets = item.get("rust_entrypoints")
        if not isinstance(targets, list) or not targets or any(
            not isinstance(t, str) or t not in rust_targets(row["rust_mapping"]) for t in targets
        ):
            issues.append("生产入口与台账不匹配")
        functions = item.get("functions")
        if not isinstance(functions, list) or not functions:
            issues.append("缺少函数或常量行为记录")
        else:
            seen = set()
            for f in functions:
                if not isinstance(f, dict):
                    issues.append("函数记录格式错误")
                    continue
                ident, span = f.get("id"), f.get("source_lines")
                if not isinstance(ident, str) or not ident or ident in seen:
                    issues.append("函数 ID 缺失或重复")
                else:
                    seen.add(ident)
                if (not isinstance(span, list) or len(span) != 2
                    or any(type(i) is not int for i in span)
                    or not 1 <= span[0] <= span[1] <= row["source_lines"]):
                    issues.append("源码行号无效")
                for dim in DIMENSIONS:
                    if not f.get(dim):
                        issues.append("行为维度缺失: " + dim)
        coverage = item.get("full_source_review")
        if not isinstance(coverage, dict) or (
            coverage.get("lines_reviewed") != row["source_lines"] or
            coverage.get("unreviewed_branches") != 0
        ):
            issues.append("逐分支全文件审计缺失")
        for key in ("reviewer", "reviewed_at", "tested_commit", "workflow_run_id"):
            if not item.get(key):
                issues.append("缺少验收凭据: " + key)
        if issues:
            errors.append(path + ": " + ", ".join(dict.fromkeys(issues)))
        else:
            verified += 1
    return errors, {"required":len(eligible),"recorded":len(contracts),
                    "missing":len(eligible - set(contracts)), "fully_evidenced":verified}


def run(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    try:
        rows = read_matrix(args.repo)
        progress = current_counts(rows)
        baseline = json.loads((args.repo / "docs/PORT_BASELINE.json").read_text(encoding="utf8"))
        if (baseline.get("expected_total") != len(rows) or
            set(baseline.get("source_hashes", {})) != set(rows)):
            raise AuditError("冻结源码路径不完整")
        evidence = check_evidence(args.repo, rows, strict=args.command == "gate")
        contracts, errors = load_contracts(args.repo / CONTRACTS)
        problem, stats = verify_contracts(rows, baseline, contracts)
        errors.extend(problem)
        original_checked = False
        if args.source_archive:
            original_problems = verify_original(args.source_archive, rows, baseline)
            errors.extend(original_problems)
            original_checked = not original_problems
        elif args.command == "gate":
            errors.append("缺少原版源码 ZIP，拒绝放行")
        if args.command == "gate" and progress["remaining"]:
            errors.append("尚有 " + str(progress["remaining"]) + " 条源码路径未完整验收")
        if args.command == "gate" and progress["accepted"] != stats["fully_evidenced"]:
            errors.append("已验收台账与逐函数证据未对齐")
        passed = not errors and (args.command == "audit" or original_checked)
        result = {"schema":"source-parity/v1", "pass":passed, "mode":args.command,
                  "original_checked":original_checked,"progress":progress,
                  "evidence":evidence,"contracts":stats,
                  "error_count":len(errors),"errors":errors}
        (output/"SOURCE_PARITY.json").write_text(
            json.dumps(result, ensure_ascii=False, indent=2)+"\n", encoding="utf8")
        summary = ["# 原版源码统一验收", "",
                   "结果：" + ("通过" if passed else "未通过"),
                   "原版 ZIP 重新验证：" + ("是" if original_checked else "否"),
                   "已验收：" + str(progress["accepted"]) + "/" + str(progress["eligible"]),
                   "完整逐函数契约：" + str(stats["fully_evidenced"])+"/"+str(stats["required"]),
                   "", "## 缺口"]
        summary += ["- "+e for e in errors[:100]]
        if len(errors)>100:
            summary.append("- 另外 "+str(len(errors)-100)+" 项见 JSON")
        (output/"SOURCE_PARITY.md").write_text("\n".join(summary)+"\n", encoding="utf8")
        print(json.dumps({k:v for k,v in result.items() if k!="errors"},ensure_ascii=False))
        return 0 if passed else 1
    except (AuditError, OSError, ValueError, KeyError, BadZipFile) as exc:
        msg = {"schema":"source-parity/v1","pass":False,"error":str(exc)}
        (output/"SOURCE_PARITY.json").write_text(json.dumps(msg,ensure_ascii=False)+"\n",encoding="utf8")
        print("原版源码验收失败："+str(exc),file=sys.stderr)
        return 1


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("command",choices=("audit","gate"))
    p.add_argument("--repo",type=Path,default=Path("."))
    p.add_argument("--source-archive",type=Path)
    p.add_argument("--output",type=Path,default=Path(".port-audit"))
    return run(p.parse_args())


if __name__ == "__main__":
    sys.exit(main())
