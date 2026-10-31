#!/usr/bin/env python3
"""One source-parity gate for the 2.1.88 original and the Rust port.

A file mapping or successful compilation never implies behavioral parity.
"""
from __future__ import annotations
import argparse
import csv
import io
import hashlib
import html
import json
import re
from pathlib import Path
import sys
from zipfile import ZipFile, BadZipFile
from source_function_index import (build_function_index, validate_contract_coverage,
                                   write_queue)
from port_workbench import (AuditError, check_evidence, current_counts, read_matrix,
                            rust_targets, verify_github_test_evidence)

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



def verify_spec_archive(archive: Path, baseline: dict):
    """Verify all specification members, deduplicate mapping by original path.

    The documentation contains static reasoning; none of its entries are
    automatically promoted to equivalent Rust runtime behavior.
    """
    if not archive.is_file():
        return ["规格包不存在: " + str(archive)], {}
    errors = []
    stats = {"members":0,"manifest_checked":0,"mapped_firstparty":0,"duplicates":0,
             "source_declarations":0,"function_like":0,"file_level_only":0,
             "function_queue": []}
    with ZipFile(archive) as z:
        names = {item.filename for item in z.infolist() if not item.is_dir()}
        stats["members"] = len(names)
        if len(names) != len([item for item in z.infolist() if not item.is_dir()]):
            errors.append("规格包包含重复 ZIP 成员")
        manifest = "MANIFEST.sha256"
        if manifest not in names:
            errors.append("规格包未提供 MANIFEST.sha256")
        else:
            seen_names = set()
            for line in z.read(manifest).decode("utf-8").splitlines():
                try:
                    sha, name = line.split("  ", 1)
                except ValueError:
                    errors.append("规格包 manifest 记录格式错误")
                    continue
                if name in seen_names or name not in names:
                    errors.append("规格包 manifest 重复或缺文件: " + name)
                    continue
                seen_names.add(name)
                if hashlib.sha256(z.read(name)).hexdigest() != sha:
                    errors.append("规格包文件内容哈希不一致: " + name)
                stats["manifest_checked"] += 1
            if seen_names != names - {manifest}:
                errors.append("规格包 manifest 未覆盖所有文件")
        mapping = "核验数据/文件与模块双向映射.tsv"
        if mapping not in names:
            errors.append("规格包缺失源码与文档双向映射表")
        else:
            rows = list(csv.DictReader(io.StringIO(
                z.read(mapping).decode("utf-8-sig")), delimiter="\t"))
            expected_src = {p for p in baseline["source_hashes"] if p.startswith("src/")}
            mapped = set()
            docs = set()
            for item in rows:
                path = "src/" + item["源文件"]
                doc = item["对应详细文档"]
                if path in mapped or doc in docs:
                    errors.append("源码或详细规格重复: " + path)
                    stats["duplicates"] += 1
                mapped.add(path)
                docs.add(doc)
                if path not in expected_src:
                    errors.append("规格登记了不存在的原版源码: " + path)
                elif item["源文件sha256"] != baseline["source_hashes"][path]:
                    errors.append("规格使用不同版本源码: " + path)
                if doc not in names:
                    errors.append("详细规格文档缺失: " + doc)
                else:
                    detail = z.read(doc).decode("utf-8")
                    section = detail.split("## 一、函数与类型声明入口", 1)
                    if len(section) != 2:
                        errors.append("规格缺少函数声明章节: " + doc)
                        continue
                    table = section[1].split("## 二", 1)[0]
                    pattern = re.compile(
                        r"^\|\s*(\d+)\s*[–-]\s*(\d+)\s*\|\s*([A-Za-z][A-Za-z0-9_]*)"
                        r"\s*\|\s*\x60([^\x60]+)\x60\s*\|"
                    )
                    seen_anchors = set()
                    added = 0
                    for text_line in table.splitlines():
                        m = pattern.match(text_line)
                        if not m:
                            continue
                        start, end, kind, symbol = m.groups()
                        start, end = int(start), int(end)
                        if not 1 <= start <= end <= int(item["源文件行数"]):
                            errors.append("源码函数位置越界: " + path + ":" + symbol)
                            continue
                        anchor = (start, end, kind, symbol)
                        if anchor in seen_anchors:
                            errors.append("规格函数声明重复: " + path + ":" + symbol)
                            continue
                        seen_anchors.add(anchor)
                        if kind == "ImportDeclaration":
                            continue
                        stats["function_queue"].append({
                            "source":path, "source_sha256":item["源文件sha256"],
                            "spec_doc":doc, "symbol":symbol, "kind":kind,
                            "source_lines":[start,end], "audit_status":"pending",
                        })
                        added += 1
                        if kind in ("FunctionDeclaration","MethodDeclaration"):
                            stats["function_like"] += 1
                    stats["source_declarations"] += added
                    if added == 0:
                        # Constants-only modules still contain executable exports.
                        # Treat their whole file as a source-review obligation.
                        stats["file_level_only"] += 1
                        stats["function_queue"].append({
                            "source":path, "source_sha256":item["源文件sha256"],
                            "spec_doc":doc, "symbol":"<file-level>",
                            "kind":"FileLevelDeclarationOrConstant",
                            "source_lines":[1,int(item["源文件行数"])],
                            "audit_status":"pending",
                        })
            stats["mapped_firstparty"] = len(mapped)
            errors.extend("第一方规格缺失: " + p for p in sorted(expected_src - mapped))
            # Duplicate payloads in empty stderr/stdout are allowed as evidence;
            # duplicated functional documents remain a fatal mistake.
            doc_hashes = {}
            for doc in sorted(docs & names):
                sha = hashlib.sha256(z.read(doc)).hexdigest()
                if sha in doc_hashes:
                    errors.append("功能文档内容重复: " + doc_hashes[sha] + " / " + doc)
                doc_hashes[sha] = doc
    return errors, stats


# This inventory is generated from exact ZIP bytes. It is a list of review
# candidates, not evidence of complete parsing or behavioral equivalence.
DECLARATION = re.compile(
    r"^\s*(?:(?:export|default|declare|abstract|async)\s+)*"
    r"(function|class|interface|type|enum|namespace|const|let|var)\s+"
    r"([A-Za-z_$][\w$]*)\b"
)


def source_declarations(data: bytes):
    declarations = []
    for number, line in enumerate(data.decode("utf-8").splitlines(), 1):
        found = DECLARATION.match(line)
        if found:
            kind, name = found.groups()
            declarations.append({"id": f"{kind}:{name}@{number}",
                                 "line": number,
                                 "declaration_sha256": hashlib.sha256(
                                     line.encode("utf-8")).hexdigest()})
    return declarations


def original_source_files(archive: Path):
    files = {}
    with ZipFile(archive) as z:
        for info in z.infolist():
            if info.is_dir():
                continue
            parts = info.filename.replace("\\", "/").split("/")
            if ".." in parts or info.filename.startswith("/"):
                raise AuditError("原版 ZIP 包含非法路径: " + info.filename)
            root = next((i for i in range(len(parts) - 2)
                         if parts[i] == "source" and parts[i + 1] in ("src", "vendor")), None)
            if root is None:
                continue
            path = "/".join(parts[root + 1:])
            if path in files or info.file_size > 30 * 1024 * 1024:
                raise AuditError("原版路径重复或文件过大: " + path)
            files[path] = z.read(info)
    return files


def index_contracts(repo: Path, archive: Path, output: Path, write: bool = False, spec_archive: Path | None = None):
    """Create a reproducible, source-backed review queue for EVERY eligible path.

    Never grant review_status=verified, copy historical approval or invent behavior.
    Existing human-reviewed contracts are preserved exactly; the review queue
    can be promoted only after independent source and Rust behavior tests.
    """
    rows = read_matrix(repo)
    baseline = json.loads((repo / "docs/PORT_BASELINE.json").read_text(encoding="utf-8"))
    problems = verify_original(archive, rows, baseline)
    if problems:
        raise AuditError("原版 ZIP 核验失败: " + "; ".join(problems[:8]))
    files = original_source_files(archive)
    function_by_source = {}
    if spec_archive is not None:
        spec_errors, _ = verify_spec_archive(spec_archive, baseline)
        if spec_errors:
            raise AuditError("详细规格校验失败: " + "; ".join(spec_errors[:8]))
        function_by_source, semantic_errors, _ = build_function_index(
            spec_archive, baseline["source_hashes"])
        if semantic_errors:
            raise AuditError("原版函数索引损坏: " + "; ".join(semantic_errors[:8]))
    old, parse_errors = load_contracts(repo / CONTRACTS)
    if parse_errors:
        raise AuditError("现有行为契约损坏: " + "; ".join(parse_errors[:8]))
    eligible = sorted(p for p, r in rows.items() if r["status"] != "excluded")
    if set(old) - set(eligible):
        raise AuditError("现有行为契约存在无效路径")
    indexed = []
    for path in eligible:
        row = rows[path]
        existing = old.get(path)
        source = files[path]
        declarations = source_declarations(source)
        candidates = function_by_source.get(path, [])
        if not candidates and spec_archive is not None:
            candidates = [{"source_id": f"{path}:file-level:1",
                           "kind": "FileLevel", "source_lines": [1, row["source_lines"]]}]
        if existing is not None and existing.get("review_status") == "verified":
            indexed.append(existing)
            continue
        old_functions = {f.get("source_id"): f for f in (existing or {}).get("functions", [])
                         if isinstance(f, dict) and f.get("source_id")}
        seeded_functions = [
            old_functions.get(f["source_id"], {
                "source_id": f["source_id"], "id": f["source_id"],
                "source_lines": f["source_lines"],
                "source_ast": f, "review_status": "pending"
            }) for f in candidates
        ] if spec_archive is not None else (existing or {}).get("functions", [])
        item = {
            "path": path,
            "source_sha256": baseline["source_hashes"][path],
            "source_lines": row["source_lines"],
            "source_declarations": declarations,
            "index_method": "static-declaration-candidates/v1",
            "review_status": "pending",
            "rust_entrypoints": rust_targets(row["rust_mapping"]),
            "functions": seeded_functions,
            "full_source_review": (existing or {}).get("full_source_review", {
                "lines_reviewed": 0, "unreviewed_branches": None}),
            "unresolved": (existing or {}).get("unresolved", ["逐函数语义与 Rust 真实行为待核实"])
        }
        # Never erase manually recorded but not-yet-accepted evidence.
        for key in ("reviewer", "reviewed_at", "tested_commit", "workflow_run_id"):
            if existing is not None and key in existing:
                item[key] = existing[key]
        indexed.append(item)
    payload = "".join(json.dumps(i, ensure_ascii=False, sort_keys=True) + "\n" for i in indexed)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(payload, encoding="utf-8")
    if write:
        target = repo / CONTRACTS
        target.write_text(payload, encoding="utf-8")
    return {"required": len(eligible), "indexed": len(indexed),
            "declaration_candidates": sum(len(i.get("source_declarations", [])) for i in indexed),
            "ast_function_candidates": sum(len(i.get("functions", [])) for i in indexed),
            "written": str(output), "contracts_updated": write}



def verify_source_lines(source_archive: Path, spec_archive: Path):
    """Compare all first-party source lines with the documented literal text.

    The spec encodes ordinary ASCII spaces as &nbsp;, while literal original
    U+00A0 nonbreaking spaces must remain intact.
    """
    errors, total_files, total_lines = [], 0, 0
    with ZipFile(source_archive) as original, ZipFile(spec_archive) as spec:
        members = {}
        for info in original.infolist():
            if not info.is_dir() and "/source/" in info.filename:
                path = info.filename.split("/source/", 1)[1]
                if path.startswith(("src/", "vendor/")):
                    if path in members:
                        raise AuditError("原版源码成员重复: " + path)
                    members[path] = info.filename
        mapping = "核验数据/文件与模块双向映射.tsv"
        rows = list(csv.DictReader(io.StringIO(spec.read(mapping).decode("utf-8-sig")),delimiter="\t"))
        pattern = re.compile(r"^\|\s*(\d+)\s*\|\s*<code>(.*?)</code>\s*\|")
        for item in rows:
            path = "src/" + item["源文件"]
            if path not in members:
                errors.append("原版第一方源文件缺失: " + path)
                continue
            total_files += 1
            code = original.read(members[path]).decode("utf-8").splitlines()
            total_lines += len(code)
            document = spec.read(item["对应详细文档"]).decode("utf-8")
            documented = {}
            for line in document.splitlines():
                match = pattern.match(line)
                if not match:
                    continue
                lineno = int(match.group(1))
                literal = html.unescape(match.group(2).replace("&nbsp;", " "))
                if literal == "（空行）":
                    literal = ""
                if lineno in documented:
                    errors.append("文档源码行号重复: " + path + ":" + str(lineno))
                documented[lineno] = literal
            if len(documented) != len(code):
                errors.append("文档行数遗漏或重复: " + path)
            for index, source_line in enumerate(code, 1):
                if documented.get(index) != source_line:
                    errors.append("源码逐行原文不一致: " + path + ":" + str(index))
    return errors, {"firstparty_files":total_files, "physical_lines":total_lines,
                    "line_mismatches":sum(s.startswith("源码逐行原文不一致") for s in errors)}


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


def verify_contracts(rows, baseline, contracts, repo=None, evidence_records=None, verify_ci=False):
    eligible = {p for p,r in rows.items() if r["status"] != "excluded"}
    errors = ["无合同的原版路径: " + p for p in sorted(eligible - set(contracts))]
    errors += ["无效范围的合同: " + p for p in sorted(set(contracts) - eligible)]
    verified = 0
    indexed = 0
    for path in sorted(eligible & set(contracts)):
        row, item = rows[path], contracts[path]
        issues = []
        if item.get("source_sha256") != baseline["source_hashes"][path]:
            issues.append("原版 SHA 不一致")
        if item.get("index_method") == "static-declaration-candidates/v1":
            if item.get("source_lines") != row["source_lines"] or not isinstance(item.get("source_declarations"), list):
                issues.append("源码索引结构不完整")
            else:
                indexed += 1
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
        if repo is not None and isinstance(targets, list):
            fingerprints = item.get("rust_source_sha256", {})
            if not isinstance(fingerprints, dict):
                issues.append("Rust 当前文件哈希证据缺失")
                fingerprints = {}
            for target in targets:
                if not isinstance(target, str) or target not in rust_targets(row["rust_mapping"]):
                    continue
                rust_file = repo / target
                if not rust_file.is_file():
                    issues.append("Rust 生产文件不存在: " + target)
                elif fingerprints.get(target) != hashlib.sha256(rust_file.read_bytes()).hexdigest():
                    issues.append("Rust 文件已发生变化，需重新进行行为验收: " + target)
        if evidence_records is not None:
            proof = evidence_records.get(path)
            if proof is None:
                issues.append("缺少唯一验收证据 PORT_EVIDENCE: " + path)
            elif any(proof.get(key) != item.get(key) for key in (
                "source_sha256","tested_commit","workflow_run_id","test_names"
            )):
                issues.append("逐函数契约与 CI 验收证据不一致")
        coverage = item.get("full_source_review")
        if not isinstance(coverage, dict) or (
            coverage.get("lines_reviewed") != row["source_lines"] or
            coverage.get("unreviewed_branches") != 0
        ):
            issues.append("逐分支全文件审计缺失")
        for key in ("reviewer", "reviewed_at", "tested_commit", "workflow_run_id"):
            if not item.get(key):
                issues.append("缺少验收凭据: " + key)
        if not issues and verify_ci:
            try:
                verify_github_test_evidence(item)
            except AuditError as exc:
                issues.append("CI 记录核验失败: " + str(exc))
        if issues:
            errors.append(path + ": " + ", ".join(dict.fromkeys(issues)))
        else:
            verified += 1
    return errors, {"required":len(eligible),"recorded":len(contracts),
                    "missing":len(eligible - set(contracts)), "indexed":indexed, "fully_evidenced":verified}


def run(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.command == "index":
        try:
            if not args.source_archive:
                raise AuditError("建立源码索引必须提供真实原版 ZIP")
            stats = index_contracts(args.repo, args.source_archive,
                                    output / "SOURCE_CONTRACT_INDEX.jsonl",
                                    write=args.write_contracts,
                                    spec_archive=getattr(args, "spec_archive", None))
            print(json.dumps(stats, ensure_ascii=False))
            return 0
        except (AuditError, OSError, ValueError, BadZipFile, UnicodeDecodeError) as exc:
            print("原版索引失败: " + str(exc), file=sys.stderr)
            return 1
    try:
        rows = read_matrix(args.repo)
        progress = current_counts(rows)
        baseline = json.loads((args.repo / "docs/PORT_BASELINE.json").read_text(encoding="utf8"))
        if (baseline.get("expected_total") != len(rows) or
            set(baseline.get("source_hashes", {})) != set(rows)):
            raise AuditError("冻结源码路径不完整")
        evidence = check_evidence(args.repo, rows, strict=args.command == "gate")
        contracts, errors = load_contracts(args.repo / CONTRACTS)
        evidence_records = {}
        proof_file = args.repo / "docs/PORT_EVIDENCE.jsonl"
        if proof_file.is_file():
            for line in proof_file.read_text(encoding="utf8").splitlines():
                if line.strip():
                    proof = json.loads(line)
                    key = proof["path"]
                    if key in evidence_records:
                        raise AuditError("验收证据重复: " + key)
                    evidence_records[key] = proof
        problem, stats = verify_contracts(
            rows, baseline, contracts,
            repo=args.repo, evidence_records=evidence_records,
            verify_ci=args.command == "gate"
        )
        errors.extend(problem)
        original_checked = False
        spec_checked = False
        line_checked = False
        line_stats = {}
        spec_stats = {}
        if args.source_archive:
            original_problems = verify_original(args.source_archive, rows, baseline)
            errors.extend(original_problems)
            original_checked = not original_problems
        elif args.command == "gate":
            errors.append("缺少原版源码 ZIP，拒绝放行")
        if getattr(args, "spec_archive", None):
            spec_problems, spec_stats = verify_spec_archive(args.spec_archive, baseline)
            errors.extend(spec_problems)
            # AST 函数队列统一由 build_function_index 产出；
            # 旧声明清单只用于规格内部一致性，不再生成第二份函数验收表。
            spec_stats.pop("function_queue", None)
            spec_checked = not spec_problems
        elif args.command == "gate":
            errors.append("缺少完整源码功能规格 ZIP，拒绝放行")
        source_function_stats = {}
        if spec_checked:
            function_index, semantic_errors, source_function_stats = build_function_index(
                args.spec_archive, baseline["source_hashes"])
            errors.extend(semantic_errors)
            write_queue(function_index, output/"SOURCE_SEMANTIC_FUNCTIONS.jsonl")
            expected_paths = {p for p,r in rows.items() if r["status"] != "excluded"}
            coverage_gaps = validate_contract_coverage(function_index, contracts, expected_paths)
            source_function_stats["unreviewed_source_paths"] = len(coverage_gaps)
            source_function_stats["missing_function_reviews"] = sum(
                row["missing"] for row in coverage_gaps)
            source_function_stats["extra_function_reviews"] = sum(
                row["extra"] for row in coverage_gaps)
            if coverage_gaps:
                (output/"SOURCE_FUNCTION_COVERAGE_GAPS.json").write_text(
                    json.dumps(coverage_gaps,ensure_ascii=False,indent=2)+"\n",
                    encoding="utf-8")
            if args.command == "gate" and coverage_gaps:
                errors.append(
                    f"有 {len(coverage_gaps)} 条原版路径的真实 AST 函数逐一验收记录缺失")
        if original_checked and spec_checked:
            line_problems, line_stats = verify_source_lines(args.source_archive, args.spec_archive)
            errors.extend(line_problems)
            line_checked = not line_problems
        if args.command == "gate" and not line_checked:
            errors.append("原版源码与逐行详细规格未完成全量原文复核")
        if args.command == "gate" and progress["remaining"]:
            errors.append("尚有 " + str(progress["remaining"]) + " 条源码路径未完整验收")
        if args.command == "gate" and progress["accepted"] != stats["fully_evidenced"]:
            errors.append("已验收台账与逐函数证据未对齐")
        passed = not errors and (args.command == "audit" or (original_checked and spec_checked and line_checked))
        result = {"schema":"source-parity/v1", "pass":passed, "mode":args.command,
                  "original_checked":original_checked,"spec_checked":spec_checked,
                  "line_checked":line_checked,"line_stats":line_stats,
                  "spec_stats":spec_stats,"source_function_stats":source_function_stats,
                  "progress":progress,
                  "evidence":evidence,"contracts":stats,
                  "error_count":len(errors),"errors":errors}
        (output/"SOURCE_PARITY.json").write_text(
            json.dumps(result, ensure_ascii=False, indent=2)+"\n", encoding="utf8")
        summary = ["# 原版源码统一验收", "",
                   "结果：" + ("通过" if passed else "未通过"),
                   "原版 ZIP 重新验证：" + ("是" if original_checked else "否"),
                   "已验收：" + str(progress["accepted"]) + "/" + str(progress["eligible"]),
                   "源码路径登记：" + str(stats["recorded"])+"/"+str(stats["required"]),
                   "原版逐函数索引：" + str(stats["indexed"])+"/"+str(stats["required"]),
                   "完整逐函数契约：" + str(stats["fully_evidenced"])+"/"+str(stats["required"]),
                   "", "## 缺口"]
        summary += ["- "+e for e in errors[:100]]
        if len(errors)>100:
            summary.append("- 另外 "+str(len(errors)-100)+" 项见 JSON")
        (output/"SOURCE_PARITY.md").write_text("\n".join(summary)+"\n", encoding="utf8")
        print(json.dumps({k:v for k,v in result.items() if k!="errors"},ensure_ascii=False))
        # A completed diagnostic report describes the gaps; it is not
        # source-parity acceptance. The strict gate continues to reject them.
        return 0 if args.command == "report" or passed else 1
    except (AuditError, OSError, ValueError, KeyError, BadZipFile) as exc:
        msg = {"schema":"source-parity/v1","pass":False,"error":str(exc)}
        (output/"SOURCE_PARITY.json").write_text(json.dumps(msg,ensure_ascii=False)+"\n",encoding="utf8")
        print("原版源码验收失败："+str(exc),file=sys.stderr)
        return 1


def main():
    p = argparse.ArgumentParser(description=__doc__)
    legacy = ("scan", "check", "counts", "impact", "stubs", "reachability", "dashboard")
    p.add_argument("command",choices=("audit","gate","report","index",*legacy,"sync-spec"))
    p.add_argument("--repo",type=Path,default=Path("."))
    p.add_argument("--source-archive",type=Path)
    p.add_argument("--spec-archive",type=Path)
    p.add_argument("--write-contracts", action="store_true", help="index: 显式更新待审契约清单")
    p.add_argument("--source", type=Path, help="scan/impact/dashboard: 解压后的原版 source 目录")
    p.add_argument("--changed", action="append", default=[])
    p.add_argument("--changed-from-git", action="store_true")
    p.add_argument("--allow-fixture", action="store_true")
    p.add_argument("--write", action="store_true", help="sync-spec: 显式更新汇总")
    p.add_argument("--output",type=Path,default=Path(".port-audit"))
    args = p.parse_args()
    if args.command in legacy:
        from port_workbench import main as run_workbench
        translated = [args.command, "--repo", str(args.repo),
                      "--output", str(args.output)]
        if args.source:
            translated += ["--source", str(args.source)]
        if args.changed_from_git:
            translated.append("--changed-from-git")
        if args.allow_fixture:
            translated.append("--allow-fixture")
        for changed in args.changed:
            translated += ["--changed", changed]
        return run_workbench(translated)
    if args.command == "sync-spec":
        from sync_spec_progress_summary import main as sync_spec_main
        return sync_spec_main(["--repo", str(args.repo)] +
                              (["--write"] if args.write else []))
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
