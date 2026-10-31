#!/usr/bin/env python3
"""20,788 original TypeScript AST function/method records -> canonical audit IDs.

This is a read-only source evidence index, never an assertion of Rust parity.
Preserve distinct callbacks sharing the same source line; position ordinal is
necessary because existing specification TSV omits character offsets.
"""
from __future__ import annotations
from collections import Counter, defaultdict
import csv
import hashlib
import io
import json
from pathlib import Path
from zipfile import ZipFile


FUNCTIONS_TSV = "核验数据/全函数与方法核查.tsv"
MAPPING_TSV = "核验数据/文件与模块双向映射.tsv"
KINDS = {"ArrowFunction", "FunctionDeclaration", "MethodDeclaration",
         "GetAccessor", "FunctionExpression", "Constructor"}


def build_function_index(archive: Path, source_hashes: dict):
    issues = []
    with ZipFile(archive) as z:
        funcs = list(csv.DictReader(
            io.StringIO(z.read(FUNCTIONS_TSV).decode("utf-8-sig")), delimiter="\t"
        ))
        mapping = list(csv.DictReader(
            io.StringIO(z.read(MAPPING_TSV).decode("utf-8-sig")), delimiter="\t"
        ))
    files = {row["源文件"]: row for row in mapping}
    if len(files) != len(mapping):
        issues.append("第一方源码路径在规格映射中重复")
    indices = defaultdict(list)
    collisions = Counter()
    for number, row in enumerate(funcs, 2):
        path = "src/" + row["文件"]
        expected_hash = source_hashes.get(path)
        original = files.get(row["文件"])
        if expected_hash is None or original is None:
            issues.append(f"TSV:{number}: 原函数指向不存在的原版路径: {path}")
            continue
        if row["原始源文件sha256"] != expected_hash or original["源文件sha256"] != expected_hash:
            issues.append(f"TSV:{number}: 原函数所属源码 SHA256 不一致: {path}")
            continue
        kind = row["AST类别"]
        if kind not in KINDS:
            issues.append(f"TSV:{number}: 未识别的原版函数类型: {kind}")
            continue
        try:
            first, last = int(row["开始行"]), int(row["结束行"])
            limit = int(original["源文件行数"])
        except (ValueError, KeyError):
            issues.append(f"TSV:{number}: 原函数起止行数无效: {path}")
            continue
        if not 1 <= first <= last <= limit:
            issues.append(f"TSV:{number}: 原函数位置越界: {path}:{first}-{last}")
            continue
        # Every AST node is distinct. Same-line callbacks are never merged.
        collision_key = (path, kind, first, last)
        offset = collisions[collision_key]
        collisions[collision_key] += 1
        function_id = f"{path}:{kind}:{first}-{last}:occurrence-{offset+1}"
        flags = {
            "async": row["是否async"] == "True",
            "generator": row["是否generator"] == "True",
        }
        metrics = {}
        for field in ("步骤数","分支节点数","显式返回数","抛错数",
                      "捕获数","await数","状态写入数"):
            try:
                n = int(row[field])
                if n < 0:
                    raise ValueError("negative")
                metrics[field] = n
            except (ValueError, KeyError):
                issues.append(f"TSV:{number}: 原函数静态指标无效: {field}")
        indices[path].append({
            "source_id":function_id,
            "source":path,"source_sha256":expected_hash,
            "kind":kind,"name":row["函数或方法"],
            "parent":row["父级作用域"],"source_lines":[first,last],
            "signature":{"parameters":row["参数"],
                         "return_type":row["返回类型声明"]},
            "flags":flags,"static_metrics":metrics,
            "review_status":"pending",
        })
    for path in sorted(source_hashes):
        if path.startswith("src/") and path not in indices:
            # May be constants, type declarations, import-only files.
            indices[path].append({
                "source_id":f"{path}:file-level:1",
                "source":path,"source_sha256":source_hashes[path],
                "kind":"FileLevel","name":"whole source file",
                "source_lines":[1,int(files[path]["源文件行数"])] if path.removeprefix("src/") in files else [],
                "review_status":"pending",
            })
    shared_position_groups = sum(count > 1 for count in collisions.values())
    return indices, issues, {
        "ast_functions":len(funcs),
        "indexed_functions":sum(len(x) for x in indices.values()),
        "firstparty_files":len(files),
        "shared_position_groups":shared_position_groups,
        "paths_with_no_function":sum(1 for p, vals in indices.items()
                                     if len(vals) == 1 and vals[0]["kind"] == "FileLevel"),
    }


def validate_contract_coverage(index: dict, contracts: dict, eligible: set):
    """Cross-check every original function against the actual reviewed contract.

    Presence of a function ID remains independent of its behavior verification.
    """
    missing = []
    for path in sorted(eligible):
        expected = {row["source_id"] for row in index.get(path, ())}
        if not expected:
            expected = {f"{path}:file-level:1"}  # vendor/ native interfaces
        contract = contracts.get(path, {})
        actual = [f.get("source_id") for f in contract.get("functions", [])
                  if isinstance(f, dict)]
        missing_ids = expected - set(actual)
        extra_ids = set(actual) - expected
        if missing_ids or extra_ids or len(actual) != len(set(actual)):
            missing.append({
                "path":path,"missing":len(missing_ids),"extra":len(extra_ids),
                "duplicate":len(actual)-len(set(actual)),
                "examples":sorted(missing_ids)[:3],
            })
    return missing


def write_queue(index, output: Path):
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w",encoding="utf-8") as dst:
        for path in sorted(index):
            for entry in index[path]:
                dst.write(json.dumps(entry,ensure_ascii=False,sort_keys=True)+"\n")
