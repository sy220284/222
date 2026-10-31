#!/usr/bin/env python3
"""Original-source-driven Rust regression acceptance.

The normal CI gate is deliberately independent of the submitter's private
original runtime: it validates SOURCE -> Rust -> live tests -> golden results.
The optional verify-original command replays frozen golden values against the
submitter's own exact 2.1.88 source tree, without uploading that tree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from collections import Counter

from port_workbench import AuditError, current_counts, read_matrix, rust_targets

REGISTRY = Path("docs/SOURCE_TEST_REGISTRY.jsonl")
LEGACY = Path("docs/PORT_EVIDENCE.jsonl")
LAYERS = {"unit", "regression", "integration", "e2e"}
PROFILES = {
    "pure": {"unit", "regression"},
    "stateful": {"unit", "regression", "integration"},
    "user_flow": {"regression", "integration", "e2e"},
}
TEST_NAME = re.compile(r"^([A-Za-z_][A-Za-z_0-9:]*)\: test$", re.M)


def _jsonl(path: Path):
    if not path.is_file():
        raise AuditError("缺少验收记录文件: " + str(path))
    records = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise AuditError(f"{path}:{number}: 非法 JSON: {exc}") from exc
        if not isinstance(value, dict):
            raise AuditError(f"{path}:{number}: 必须是对象")
        records.append(value)
    return records


def _safe_file(repo: Path, raw: str):
    if not isinstance(raw, str) or not raw or "\\" in raw:
        raise ValueError("文件必须为仓库相对 POSIX 路径")
    path = Path(raw)
    if path.is_absolute() or any(v in (".", "..") for v in path.parts):
        raise ValueError("文件不得逃逸仓库: " + raw)
    full = repo / path
    if not full.is_file() or full.is_symlink():
        raise ValueError("仓库文件不存在或为符号链接: " + raw)
    if not full.resolve().is_relative_to(repo.resolve()):
        raise ValueError("文件指向仓库外: " + raw)
    return full


def inventory_tests(path: Path):
    if not path.is_file():
        raise AuditError("缺少真实 Rust --list 测试清单: " + str(path))
    content = path.read_text(encoding="utf-8")
    names = set(TEST_NAME.findall(content))
    if not names or "test" not in content:
        raise AuditError("Rust 测试清单为空/格式异常，禁止视作运行成功")
    return names


def inspect(repo: Path, rows: dict, hashes: dict, entries: list, legacy: list,
            discovered: set | None, full: bool):
    errors, paths, linked = [], {}, {}
    required = {path for path, row in rows.items() if row["status"] != "excluded"}
    actual_verified = {path for path in required if rows[path]["status"] == "verified"}
    for record in entries:
        path = record.get("path")
        if not isinstance(path, str) or path not in required:
            errors.append("注册了不存在或排除的原版路径: " + str(path))
            continue
        if path in paths:
            errors.append("测试注册重复原版路径: " + path)
            continue
        paths[path] = record
        if record.get("source_sha256") != hashes.get(path):
            errors.append(path + ": 登记源码 SHA 与冻结 2.1.88 不一致")
        profile = record.get("profile")
        if profile not in PROFILES:
            errors.append(path + ": 缺少明确的 pure/stateful/user_flow 职责类别")
            continue
        test_refs = record.get("tests")
        if not isinstance(test_refs, list) or not test_refs:
            errors.append(path + ": 无真实 Rust 测试引用")
            continue
        layers, names = set(), set()
        for ref in test_refs:
            if not isinstance(ref, dict):
                errors.append(path + ": 测试引用格式错误")
                continue
            name, layer = ref.get("name"), ref.get("layer")
            if (not isinstance(name, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9:]*", name)
                    or layer not in LAYERS):
                errors.append(path + ": 测试名称或类别不合法: " + str(ref))
                continue
            if name in names:
                errors.append(path + ": 同一测试重复登记: " + name)
            names.add(name)
            layers.add(layer)
            if discovered is not None and name not in discovered:
                errors.append(path + ": Rust 实际 --list 中找不到: " + name)
        missing_layers = PROFILES[profile] - layers
        if missing_layers:
            errors.append(path + ": 缺少职责要求的测试层: " + ", ".join(sorted(missing_layers)))
        targets = record.get("rust_entrypoints")
        if not isinstance(targets, list) or not targets:
            errors.append(path + ": 未登记 Rust 真实生产文件")
        else:
            for target in targets:
                try:
                    file = _safe_file(repo, target)
                    if not target.startswith("src/") or file.suffix != ".rs":
                        raise ValueError("Rust 入口必须为 src/ 下的生产源文件")
                except ValueError as exc:
                    errors.append(path + ": " + str(exc))
        source_ids = record.get("source_ids")
        if (not isinstance(source_ids, list) or not source_ids or
            any(not isinstance(s, str) or not s.startswith(path + ":")
                for s in source_ids) or
            len(source_ids) != len(set(s for s in source_ids if isinstance(s, str)))):
            errors.append(path + ": 原版 AST 函数身份未正确登记")
        adapter = record.get("adapter")
        if not isinstance(adapter, dict):
            errors.append(path + ": 缺少真实 Rust 黄金回归执行适配器")
        else:
            binary = adapter.get("binary")
            input_key = adapter.get("input_key")
            pointer = adapter.get("observation_pointer")
            if (not isinstance(binary, str) or
                not re.fullmatch(r"parity_[A-Za-z_][A-Za-z_0-9]*", binary)):
                errors.append(path + ": 适配器二进制名称非法")
            else:
                try:
                    _safe_file(repo, "src/bin/" + binary + ".rs")
                except ValueError as exc:
                    errors.append(path + ": " + str(exc))
            if (not isinstance(input_key, str) or
                not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", input_key) or
                not isinstance(pointer, str) or not pointer.startswith("/")
                or pointer == "/"):
                errors.append(path + ": 适配器输入/输出观察字段非法")
        if not any(isinstance(ref, dict) and
                   ref.get("name") == "all_registered_source_goldens_execute_against_production_adapters"
                   and ref.get("layer") == "regression" for ref in test_refs):
            errors.append(path + ": 未纳入可自动执行全部原版黄金样例的统一回归测试")
        differential_kinds = record.get("differential_kinds")
        accepted_kinds = {"normal","edge","error","permission_allow",
                          "permission_deny","cancel","concurrent",
                          "restart","stream","side_effect"}
        if (not isinstance(differential_kinds, list) or
            any(not isinstance(k, str) for k in differential_kinds) or
            len(differential_kinds) != len(set(differential_kinds)) or
            not {"normal","edge"}.issubset(differential_kinds) or
            not set(differential_kinds).issubset(accepted_kinds)):
            errors.append(path + ": 源码职责差异场景未登记（正常与边界强制）")
        driver = record.get("original_driver")
        if driver is not None:
            if (not isinstance(driver, str) or
                not driver.startswith("scripts/source_parity_drivers/") or
                not driver.endswith(".mjs")):
                errors.append(path + ": 原版状态测试脚本必须位于受审的私有复验脚本目录")
            else:
                try:
                    _safe_file(repo, driver)
                except ValueError as exc:
                    errors.append(path + ": " + str(exc))
        golden = record.get("golden")
        if not isinstance(golden, str):
            errors.append(path + ": 缺少原版预期结果固化文件")
        else:
            try:
                fixture = json.loads(_safe_file(repo, golden).read_text(encoding="utf-8"))
                if (fixture.get("schema") != "source-golden/v1" or
                    fixture.get("source_path") != path or
                    fixture.get("source_sha256") != hashes.get(path) or
                    not isinstance(fixture.get("cases"), list) or
                    len(fixture["cases"]) < 2):
                    errors.append(path + ": 原版预期结果、SHA 或样例数量无效")
                else:
                    ids = [c.get("id") for c in fixture["cases"] if isinstance(c, dict)]
                    if len(ids) != len(fixture["cases"]) or len(set(ids)) != len(ids) or any(
                        not isinstance(i, str) or not i for i in ids
                    ):
                        errors.append(path + ": 原版预期样例 ID 缺失、重复或无效")
                    if not all(isinstance(c, dict) and "input" in c and "expected" in c
                               for c in fixture["cases"]):
                        errors.append(path + ": 原版预期样例没有输入与明确预期")
                    else:
                        types = [c.get("kind") for c in fixture["cases"]]
                        if (any(k not in {"normal","edge","error",
                             "permission_allow","permission_deny","cancel",
                             "concurrent","restart","stream","side_effect"}
                             for k in types) or
                            not {"normal", "edge"}.issubset(types)):
                            errors.append(path + ": 黄金结果必须显式包含正常与边界场景")
                        normal_inputs = {
                            json.dumps(c["input"], sort_keys=True, ensure_ascii=False)
                            for c in fixture["cases"] if c.get("kind") == "normal"
                        }
                        edge_inputs = {
                            json.dumps(c["input"], sort_keys=True, ensure_ascii=False)
                            for c in fixture["cases"] if c.get("kind") == "edge"
                        }
                        if normal_inputs == edge_inputs:
                            errors.append(path + ": 正常与边界场景必须使用不同的原版输入")
                        actual_covered = set()
                        default_export = fixture.get("export")
                        operations = {c.get("export", default_export)
                                      for c in fixture["cases"] if isinstance(c, dict)}
                        if len(operations) > 1 and (not isinstance(adapter, dict) or
                            not adapter.get("operation_key")):
                            errors.append(path + ": 多导出原版模块缺少 Rust 函数选择字段")
                        if isinstance(adapter, dict) and adapter.get("operation_key") is not None:
                            key = adapter["operation_key"]
                            if (not isinstance(key, str) or
                                not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", key) or
                                key == adapter.get("input_key")):
                                errors.append(path + ": Rust 函数选择字段名称无效")
                        for case in fixture["cases"]:
                            symbol = case.get("export", default_export)
                            if driver is None and (not isinstance(symbol, str) or
                                not re.fullmatch(r"[A-Za-z_$][A-Za-z_0-9$]*", symbol)):
                                errors.append(path + ": 黄金场景缺少有效的原版导出函数名")
                            case_ids = case.get("source_ids", source_ids if len(source_ids)==1 else [])
                            if (not isinstance(case_ids, list) or
                                not case_ids or any(not isinstance(i,str) for i in case_ids) or
                                not set(case_ids).issubset(source_ids)):
                                errors.append(path + ": 黄金场景未准确绑定原版 AST 函数 ID")
                            else:
                                actual_covered.update(case_ids)
                        if actual_covered != set(source_ids):
                            errors.append(path + ": 黄金场景尚未覆盖已登记的全部原版函数")
            except (OSError, ValueError, KeyError, TypeError) as exc:
                errors.append(path + ": 固化结果文件错误: " + str(exc))
        linked[path] = len(names)
    # Independently recheck pre-existing, already accepted source records.
    old = {}
    for record in legacy:
        path = record.get("path")
        if path in old:
            errors.append("旧验收证据重复登记: " + str(path))
            continue
        old[path] = record
        if path not in rows:
            errors.append("旧证据引用冻结清单外的原版路径: " + str(path))
            continue
        if rows[path]["status"] == "excluded":
            # Keep the historical audit trail; an explicitly authorized
            # deletion of the now-removed production module supersedes its
            # former acceptance, which cannot still count as live coverage.
            continue
        if record.get("source_sha256") != hashes.get(path):
            errors.append(path + ": 旧证据源码哈希已失效")
        refs = record.get("test_names")
        if not isinstance(refs, list) or not refs or any(
            not isinstance(name, str) or not name for name in refs
        ):
            errors.append(path + ": 历史验收缺少真实测试名称")
        elif discovered is not None:
            for name in refs:
                if name not in discovered:
                    errors.append(path + ": 历史已验收测试不再存在: " + name)
        for target in record.get("rust_entrypoints", []):
            try:
                _safe_file(repo, target)
            except ValueError as exc:
                errors.append(path + ": 旧证据生产入口已失效: " + str(exc))
    # New original-verified golden evidence can supersede historical CI records.
    # A migrated source must never depend on fabricating old run IDs.
    legacy_missing = sorted(actual_verified - (old.keys() | paths.keys()))
    if legacy_missing:
        errors.append(f"{len(legacy_missing)} 个台账已验收路径缺少历史测试证据")
    # New, source-proven golden suites must be present for every path
    # before claiming full acceptance. Historical test names alone do not
    # prove original input/output equivalence.
    unregistered = sorted(required - paths.keys())
    not_completed = sorted(path for path in required if rows[path]["status"] != "verified")
    active_old = set(old) & required
    if full:
        if discovered is None:
            errors.append("完整验收需要本次成功执行的 cargo test -- --list")
        if unregistered:
            errors.append(f"{len(unregistered)} 条原版保留路径缺少原版驱动的 Rust 测试登记")
        if not_completed:
            errors.append(f"{len(not_completed)} 条路径仍为部分实现或未移植")
    return {
        "schema": "source-rust-test-suite/v1",
        "mode": "gate" if full else "check",
        "pass": not errors,
        "source_paths": len(rows),
        "eligible": len(required),
        "verified_in_matrix": len(actual_verified),
        "legacy_evidence_paths": len(active_old),
        "archived_excluded_evidence_paths": len(set(old) - required),
        "golden_registered_paths": len(paths),
        "golden_cases": sum(
            len(json.loads((repo / r["golden"]).read_text(encoding="utf-8"))["cases"])
            for r in paths.values() if isinstance(r.get("golden"), str)
            and (repo / r["golden"]).is_file()
        ),
        "missing_source_tests": len(unregistered),
        "missing_source_test_paths": unregistered,
        "remaining_port_paths": len(not_completed),
        "remaining_port_path_names": not_completed,
        "linked_rust_tests": linked,
        "errors": errors,
    }


def verify_original(repo: Path, source: Path, entries: list, hashes: dict):
    """Recalculate golden cases against real private originals, never mutate snapshots."""
    errors = []
    root = source.resolve(strict=True)
    if root == repo.resolve() or root.is_relative_to(repo.resolve()):
        raise AuditError("原版源码不得放在公开工作树内")
    for record in entries:
        path = record["path"]
        file = root / path
        if not file.is_file() or file.is_symlink():
            errors.append("原版工作区缺少实际文件: " + path)
            continue
        if hashlib.sha256(file.read_bytes()).hexdigest() != hashes[path]:
            errors.append("本机原版源码哈希不符合冻结版本: " + path)
            continue
        fixture = json.loads((repo / record["golden"]).read_text(encoding="utf-8"))
        export = fixture.get("export", "")
        if not isinstance(export, str) or (export and
            not re.fullmatch(r"[A-Za-z_$][A-Za-z_0-9$]*", export)):
            errors.append(path + ": 原版默认执行入口名称无效")
            continue
        # The original-side driver, when present, is versioned and reviewed.
        # It imports the actual original from a separate contributor workspace.
        # Plain pure functions use the restricted, fixed inline bridge.
        driver = record.get("original_driver")
        bridge = """
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const module = await import(pathToFileURL(process.argv[1]).href);
const cases = JSON.parse(readFileSync(0, 'utf8'));
const outputs = [];
for (const item of cases) {
  const symbol = item.export ?? process.argv[2];
  const entry = module[symbol];
  if (typeof entry !== 'function') throw Error('original export not callable: ' + symbol);
  const args = Array.isArray(item.args) ? item.args : [item.input];
  outputs.push({id:item.id,expected:await entry(...args)});
}
process.stdout.write(JSON.stringify(outputs));
"""
        if driver:
            command = ["node", "--experimental-transform-types", "--no-warnings",
                       str(_safe_file(repo, driver)), str(file)]
        else:
            command = ["node", "--experimental-strip-types", "--no-warnings",
                       "--input-type=module", "--eval", bridge, str(file), export]
        try:
            process = subprocess.run(
                command,
                input=json.dumps(fixture["cases"], ensure_ascii=False),
                capture_output=True, text=True, encoding="utf-8", timeout=30,
                cwd=root, check=False, env={"PATH":__import__("os").environ.get("PATH",""),
                                          "HOME":str(root), "NO_COLOR":"1"},
            )
            if process.returncode or not process.stdout:
                errors.append(path + ": 原版真实执行失败（退出码 " +
                              str(process.returncode) + "）")
                continue
            result = json.loads(process.stdout)
            expected = [{"id":c["id"],"expected":c["expected"]} for c in fixture["cases"]]
            if result != expected:
                differences = [a["id"] for a,b in zip(result,expected) if a != b]
                errors.append(path + ": 原版实际行为与固化结果不同: " + ", ".join(differences))
        except (OSError, ValueError, subprocess.TimeoutExpired) as exc:
            errors.append(path + ": 原版复验失败: " + str(exc)[:180])
    return errors


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("command", choices=("check","gate","report","verify-original"))
    p.add_argument("--repo", type=Path, default=Path("."))
    p.add_argument("--output", type=Path, default=Path(".port-audit"))
    p.add_argument("--inventory", type=Path,
                   help="本次 Rust 全部测试通过后 cargo test --locked --all-targets -- --list 的原始输出")
    p.add_argument("--source-root", type=Path, help="提交者私有原版 source/ 工作区")
    args = p.parse_args(argv)
    repo, output = args.repo.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    try:
        rows = read_matrix(repo)
        current_counts(rows)
        baseline = json.loads((repo/"docs/PORT_BASELINE.json").read_text(encoding="utf-8"))
        hashes = baseline["source_hashes"]
        if set(rows) != set(hashes) or baseline["expected_total"] != len(rows):
            raise AuditError("冻结原版清单与迁移台账不一致")
        entries = _jsonl(repo/REGISTRY)
        legacy = _jsonl(repo/LEGACY)
        discovered = (inventory_tests(args.inventory) if args.inventory else None)
        result = inspect(repo, rows, hashes, entries, legacy, discovered, args.command=="gate")
        if args.command == "gate":
            # Source-to-test linkage alone is not a proof of *all* branches
            # and side effects. Preserve the original per-function audit as a
            # second, mandatory condition for full-source acceptance.
            from source_parity_gate import (
                CONTRACTS, load_contracts, verify_contracts,
            )
            source_contracts, parsing_errors = load_contracts(repo / CONTRACTS)
            contract_errors, contract_counts = verify_contracts(
                rows, baseline, source_contracts, repo=repo,
                evidence_records={r["path"]: r for r in legacy},
                verify_ci=False,
            )
            result["source_function_contracts"] = contract_counts
            result["source_function_contract_errors"] = parsing_errors + contract_errors
            if parsing_errors or contract_errors:
                result["errors"].append(
                    f"逐函数/逐分支原版契约尚有 {len(parsing_errors)+len(contract_errors)} 处不完整"
                )
        if args.command in ("check","gate") and discovered is None:
            result["errors"].append("缺少本次真实编译测试的 Rust --list 清单")
        if args.command == "verify-original":
            if not args.source_root:
                result["errors"].append("未提供提交者自己工作区 --source-root")
            else:
                result["original_errors"] = verify_original(repo, args.source_root, entries, hashes)
                result["errors"].extend(result["original_errors"])
        result["pass"] = not result["errors"]
        result["actual_discovered_tests"] = len(discovered) if discovered is not None else None
        (output/"SOURCE_TEST_SUITE.json").write_text(
            json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
        report = [
            "# 原版驱动的 Rust 回归测试验收","",
            "结果：" + ("通过" if result["pass"] else "未通过"),
            f"冻结原版路径：{result['eligible']}/{result['source_paths']}（保留/全部）",
            f"已验收台账：{result['verified_in_matrix']}",
            f"原版黄金结果覆盖：{result['golden_registered_paths']}/{result['eligible']}",
            f"真实本轮发现的 Rust 测试：{result['actual_discovered_tests']}",
            f"仍未登记黄金回归：{result['missing_source_tests']}",
            f"仍未移植或未验收：{result['remaining_port_paths']}", "",
            "## 验收问题",
        ] + ["- "+str(x) for x in result["errors"]]
        (output/"SOURCE_TEST_SUITE.md").write_text("\n".join(report)+"\n",encoding="utf-8")
        print(json.dumps({k:v for k,v in result.items()
                          if k not in ("missing_source_test_paths","remaining_port_path_names",
                                       "linked_rust_tests","source_function_contract_errors","errors")},ensure_ascii=False))
        for issue in result["errors"][:100]:
            print("门禁问题: " + str(issue), file=sys.stderr)
        if len(result["errors"]) > 100:
            print(f"另有 {len(result['errors']) - 100} 项详见 JSON", file=sys.stderr)
        return 0 if args.command=="report" or result["pass"] else 1
    except (AuditError, OSError, ValueError, KeyError, TypeError) as exc:
        print("原版驱动的 Rust 测试门禁失败：" + str(exc),file=sys.stderr)
        (output/"SOURCE_TEST_SUITE.json").write_text(json.dumps(
            {"schema":"source-rust-test-suite/v1","pass":False,"error":str(exc)},
            ensure_ascii=False)+"\n",encoding="utf-8")
        return 1


if __name__ == "__main__":
    sys.exit(main())
