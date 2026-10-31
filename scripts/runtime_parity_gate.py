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
    if not isinstance(ids, list) or len(ids) != len(set(ids)) or set(ids) != set(case["source_ids"]):
        return None, side + " 真实执行路径未覆盖场景声明的所有原版函数 ID"
    missing = [field for field in REQUIRED_OBSERVATION if field not in observation]
    if missing:
        return None, side + " 缺少行为观察字段: " + ", ".join(missing)
    observation = dict(observation)
    observation["process_exit_code"] = result.returncode
    # Adapter diagnostics are not a substitute for the structured error field.
    observation["process_stderr_sha256"] = hashlib.sha256(
        result.stderr.encode("utf-8")).hexdigest()
    return observation, None


def run_dynamic_parity(repo: Path, original: Path, case_dir: Path,
                       index: dict, eligible: set, output: Path):
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
    # Baseline + adversarial path must run for every required source unit.
    uncovered = {sid:sorted({"normal","edge"}-kinds)
                 for sid,kinds in coverage.items()
                 if not {"normal","edge"}.issubset(kinds)}
    if uncovered:
        errors.append(f"{len(uncovered)} 个原版功能审计对象缺少动态正常/边界场景")
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
