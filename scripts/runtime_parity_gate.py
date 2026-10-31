#!/usr/bin/env python3
"""Execute paired ORIGINAL and Rust scenarios, check real observations and coverage.

The original lives in the submitter's isolated workspace. Neither source code
nor stdout/stderr are uploaded as CI artifacts. No shell evaluation is used.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REQUIRED_OBSERVATION = (
    "outcome", "errors", "state", "calls", "permissions", "events",
    "side_effects", "persistence", "cancellation",
)
KINDS = frozenset((
    "normal", "edge", "error", "permission_allow", "permission_deny",
    "cancel", "concurrent", "restart", "stream", "side_effect",
))
DIMENSIONS = frozenset(REQUIRED_OBSERVATION)


def _load_cases(directory: Path):
    if not directory.is_dir():
        return [], ["动态等价场景目录不存在: " + str(directory)]
    cases, errors, seen = [], [], set()
    for file in sorted(directory.glob("*.json")):
        try:
            case = json.loads(file.read_text(encoding="utf-8"))
            ident = case["id"]
            if not isinstance(ident, str) or not ident or ident in seen:
                raise ValueError("场景 ID 为空或重复")
            seen.add(ident)
            source_ids = case["source_ids"]
            if not isinstance(source_ids, list) or not source_ids or len(source_ids) != len(set(source_ids)):
                raise ValueError("source_ids 缺失或重复")
            if any(not isinstance(s, str) or not s for s in source_ids):
                raise ValueError("source_ids 含非法标识")
            if case["kind"] not in KINDS:
                raise ValueError("场景类型未知")
            if not isinstance(case["input"], (dict, list)):
                raise ValueError("输入必须是 JSON 对象或数组")
            timeout = case["timeout_seconds"]
            if type(timeout) not in (int,float) or not 0 < timeout <= 180:
                raise ValueError("超时必须在 (0,180] 秒")
            for side in ("original", "rust"):
                cmd = case[side]
                if not isinstance(cmd, list) or not cmd or not all(
                    isinstance(t, str) and t for t in cmd
                ):
                    raise ValueError(side + " 必须是直接执行的非空 argv 数组")
                if any("\x00" in t for t in cmd):
                    raise ValueError("命令参数包含 NUL")
                required_root = "{ORIGINAL}" if side=="original" else "{REPO}"
                if not any(required_root in arg for arg in cmd):
                    raise ValueError(side + " 适配器命令必须引用对应真实工作区 " + required_root)
            case["__path"] = str(file)
            cases.append(case)
        except (OSError, KeyError, ValueError, TypeError, json.JSONDecodeError) as exc:
            errors.append(file.name + ": " + str(exc))
    if not cases:
        errors.append("没有任何可执行的原版/Rust 成对动态场景")
    return cases, errors


def _command(template: list, root: Path, original: Path, sandbox: Path, test_id: str):
    vals = {"{REPO}":str(root),"{ORIGINAL}":str(original),
            "{SANDBOX}":str(sandbox),"{CASE_ID}":test_id}
    result = []
    for arg in template:
        for token, value in vals.items():
            arg = arg.replace(token, value)
        if "{" in arg and ("{REPO" in arg or "{ORIGINAL" in arg or "{SANDBOX" in arg):
            raise ValueError("未解析的工作区命令占位符")
        result.append(arg)
    return result


def _snapshot_sandbox(sandbox: Path):
    """Capture actual on-disk effects, independent of adapter self-report."""
    result = {}
    for path in sorted(sandbox.rglob("*")):
        if path.is_symlink():
            raise ValueError("测试沙箱创建了符号链接")
        if path.is_file():
            if path.stat().st_size > 10_000_000:
                raise ValueError("动态测试产生过大文件，需审查副作用")
            result[path.relative_to(sandbox).as_posix()] = {
                "bytes":path.stat().st_size,
                "sha256":hashlib.sha256(path.read_bytes()).hexdigest(),
            }
    return result


def _execute(template, repo, original, sandbox, case, side):
    cmd = _command(template, repo, original, sandbox, case["id"])
    env = {
        "PATH": os.environ.get("PATH",""),
        "LANG":"C.UTF-8", "LC_ALL":"C.UTF-8",
        "HOME":str(sandbox / "home"), "TMPDIR":str(sandbox / "tmp"),
        "SOURCE_DATE_EPOCH":"1720000000",
        "TZ":"UTC", "NO_COLOR":"1",
        "PARITY_SEED":"220", "PARITY_CASE_ID":case["id"],
        "PARITY_SIDE":side,
    }
    (sandbox/"home").mkdir(parents=True)
    (sandbox/"tmp").mkdir()
    payload = json.dumps({"id":case["id"],"input":case["input"]},
                         ensure_ascii=False,sort_keys=True)
    try:
        result = subprocess.run(
            cmd, input=payload, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, cwd=original if side=="original" else repo,
            timeout=case["timeout_seconds"], env=env, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, side + " 实际运行失败: " + type(exc).__name__ + " " + str(exc)[:220]
    if len(result.stdout) > 2_000_000 or len(result.stderr) > 2_000_000:
        return None, side + " 输出超过 2MB，拒绝验收"
    try:
        observation = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None, side + " 未返回唯一的 JSON 动态观察结果（退出码 " + str(result.returncode) + "）"
    if not isinstance(observation, dict):
        return None, side + " 动态观察结果不是对象"
    if observation.get("schema") != "source-rust-observation/v1":
        return None, side + " 动态观察协议版本不匹配"
    if observation.get("case_id") != case["id"]:
        return None, side + " 未执行所请求的场景 ID"
    # A driver must disclose actual source IDs it exercised. This trace and
    # the adapter itself require independent code review; no synthetic pass.
    ids = observation.get("executed_source_ids")
    if (not isinstance(ids, list) or
        any(not isinstance(item, str) or not item for item in ids) or
        len(ids) != len(set(ids)) or set(ids) != set(case["source_ids"])):
        return None, side + " 真实执行路径未覆盖场景声明的所有原版函数 ID"
    missing = [field for field in REQUIRED_OBSERVATION if field not in observation]
    if missing:
        return None, side + " 缺少行为观察字段: " + ", ".join(missing)
    for key in ("errors", "calls", "permissions", "events", "side_effects"):
        if not isinstance(observation[key], list):
            return None, side + " 行为观察字段必须是实际事件列表: " + key
    for key in ("state", "persistence", "cancellation"):
        if not isinstance(observation[key], dict):
            return None, side + " 行为观察字段必须是状态对象: " + key
    if case["kind"] == "permission_deny" and not any(
        entry == "deny" or (isinstance(entry, dict) and
                            entry.get("decision") == "deny")
        for entry in observation["permissions"]
    ):
        return None, side + " 权限拒绝场景缺少实际 deny 决定"
    if case["kind"] == "permission_allow" and not any(
        entry == "allow" or (isinstance(entry, dict) and
                             entry.get("decision") == "allow")
        for entry in observation["permissions"]
    ):
        return None, side + " 权限允许场景缺少实际 allow 决定"
    if case["kind"] == "cancel" and observation["cancellation"].get("cancelled") is not True:
        return None, side + " 取消场景没有取消完成证据"
    if case["kind"] == "error" and not observation["errors"]:
        return None, side + " 错误路径场景没有真实错误记录"
    if case["kind"] == "restart" and not observation["persistence"]:
        return None, side + " 重启恢复场景缺少持久化状态证据"
    if case["kind"] == "stream" and not any(
        event == "chunk" or (isinstance(event, dict) and
                             event.get("kind") in ("chunk","delta"))
        for event in observation["events"]
    ):
        return None, side + " 流式行为缺少实际流式分块事件"
    try:
        filesystem_effects = _snapshot_sandbox(sandbox)
    except (OSError, ValueError) as exc:
        return None, side + " 文件副作用审计失败: " + str(exc)
    observation = dict(observation)
    observation["filesystem_effects"] = filesystem_effects
    observation["process_exit_code"] = result.returncode
    # Adapter diagnostics are not a substitute for the structured error field.
    observation["process_stderr_sha256"] = hashlib.sha256(
        result.stderr.encode("utf-8")).hexdigest()
    return observation, None


def _required_kinds(index, contracts, expected):
    """Every behavior family requires a case or a source-anchored N/A review."""
    requirements, errors = {}, []
    for sid, path in expected.items():
        records = contracts.get(path, {}).get("functions", [])
        record = next((r for r in records if isinstance(r, dict) and
                       r.get("source_id") == sid), None)
        if record is None:
            errors.append(sid + ": 未登记完整原版函数级审计合同")
            requirements[sid] = set(KINDS)
            continue
        scope = record.get("not_applicable_scenarios", {})
        if not isinstance(scope, dict):
            errors.append(sid + ": 不适用场景说明不是 JSON 对象")
            scope = {}
        for kind, evidence in scope.items():
            if kind not in KINDS or kind in ("normal", "edge"):
                errors.append(sid + ": 不允许跳过正常、边界或未知类型: " + str(kind))
                continue
            if not isinstance(evidence, dict):
                errors.append(sid + ": 缺失不适用场景源码依据: " + kind)
                continue
            anchor = evidence.get("source_lines")
            reason = evidence.get("reason")
            parent = next((r for r in index.get(path, []) if r["source_id"] == sid), None)
            limit = parent["source_lines"] if parent else None
            if (not isinstance(anchor, list) or len(anchor)!=2 or
                any(type(n) is not int for n in anchor) or
                not limit or not limit[0] <= anchor[0] <= anchor[1] <= limit[1] or
                not isinstance(reason, str) or len(reason.strip()) < 20 or
                evidence.get("source_sha256") != parent["source_sha256"]):
                errors.append(sid + ": 不适用场景缺少可追溯原版源码位置、哈希和具体理由: " + kind)
        valid = {k for k in scope if k in KINDS and k not in ("normal","edge") and
                 isinstance(scope[k], dict) and
                 scope[k].get("source_sha256") == (next((r["source_sha256"] for r in index.get(path, []) if r["source_id"] == sid), None)) and
                 isinstance(scope[k].get("reason"), str) and len(scope[k]["reason"].strip()) >= 20 and
                 isinstance(scope[k].get("source_lines"), list) and len(scope[k]["source_lines"])==2 and
                 all(type(n) is int for n in scope[k]["source_lines"])}
        requirements[sid] = set(KINDS) - valid
    return requirements, errors


def run_dynamic_parity(repo: Path, original: Path, case_dir: Path,
                       index: dict, eligible: set, output: Path, contracts=None):
    """Fail closed on missing scenarios, mismatch or unavailable runtime.

    Every AST function, exported initializer, file-only module and vendor path
    must be exercised by an independently executable matching pair.
    """
    repo, original = repo.resolve(), original.resolve()
    cases, errors = _load_cases(case_dir)
    expected = {}
    for path in sorted(eligible):
        nodes = index.get(path)
        if not nodes:
            expected[f"{path}:file-level:1"] = path
        else:
            for node in nodes:
                ident = node["source_id"]
                if ident in expected:
                    errors.append("原版函数审计 ID 重复: " + ident)
                expected[ident] = path
    required, review_errors = _required_kinds(index, contracts or {}, expected)
    errors.extend(review_errors)
    coverage = {key:set() for key in expected}
    attempted = passed = 0
    mismatches = []
    for case in cases:
        ident, ids = case["id"], case["source_ids"]
        invalid = sorted(set(ids) - expected.keys())
        if invalid:
            errors.append(ident + ": 引用了排除或不存在的原版函数: " + ", ".join(invalid[:3]))
            continue
        attempted += 1
        with tempfile.TemporaryDirectory(prefix="source-rust-parity-") as tmp:
            temp = Path(tmp)
            result_original, err1 = _execute(
                case["original"],repo,original,temp/"original",case,"original")
            result_rust, err2 = _execute(
                case["rust"],repo,original,temp/"rust",case,"rust")
        if err1 or err2:
            mismatches.append({"id":ident,"kind":case["kind"],
                               "reason":"; ".join(e for e in (err1,err2) if e)})
            continue
        # Strict structured comparison includes states, permissions, order,
        # cancellation, persisted outputs, side effects, exceptions and exit.
        if result_original != result_rust:
            fields = sorted(k for k in set(result_original) | set(result_rust)
                            if result_original.get(k) != result_rust.get(k))
            mismatches.append({"id":ident,"kind":case["kind"],
                               "reason":"运行结果不等价","different_fields":fields})
            continue
        passed += 1
        for sid in ids:
            coverage[sid].add(case["kind"])
    # Reject relabeling the exact same input as both normal and edge.
    for sid in expected:
        normal_inputs = {json.dumps(c["input"],ensure_ascii=False,sort_keys=True)
                         for c in cases if c["kind"]=="normal" and sid in c["source_ids"]}
        edge_inputs = {json.dumps(c["input"],ensure_ascii=False,sort_keys=True)
                       for c in cases if c["kind"]=="edge" and sid in c["source_ids"]}
        if normal_inputs and edge_inputs and normal_inputs == edge_inputs:
            errors.append(sid + ": 正常与边界场景输入完全相同，无真实边界覆盖")
    # All ten behavior families must run unless a source-anchored, human
    # reviewed contract explicitly documents why a family does not apply.
    uncovered = {sid:sorted(required[sid]-kinds)
                 for sid,kinds in coverage.items()
                 if not required[sid].issubset(kinds)}
    if uncovered:
        errors.append(f"{len(uncovered)} 个原版功能审计对象缺少动态场景及可核验的不适用依据")
    if mismatches:
        errors.append(f"{len(mismatches)} 个原版与 Rust 的实际运行结果不一致或执行失败")
    if not passed:
        errors.append("不存在任何已通过的原版/Rust 成对实际运行证据")
    report = {"schema":"source-rust-dynamic-parity/v1",
              "pass":not errors,"required_units":len(expected),
              "scenarios":len(cases),"executed":attempted,"passed":passed,
              "missing_units":len(uncovered),
              "missing_examples":dict(list(uncovered.items())[:50]),
              "mismatches":mismatches[:100],"errors":errors,
              "note":"源代码须由提交者工作区提供；此结果仅对本次真实执行有效。"}
    output.mkdir(parents=True,exist_ok=True)
    (output/"DYNAMIC_PARITY.json").write_text(
        json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    return report
