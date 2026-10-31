#!/usr/bin/env python3
"""把增强版40份产品规格逐路径绑定到222的唯一迁移台账；绝不自动上调验收状态。"""
from __future__ import annotations
import argparse
import csv
import hashlib
import io
import json
import re
import sys
import zipfile
from collections import Counter, defaultdict
from pathlib import Path

def load(spec_path: Path, matrix_path: Path, original_path: Path | None):
    matrix = {}
    for line in matrix_path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("| `"):
            continue
        parts = [p.strip() for p in line.split("|")]
        if len(parts) < 8 or not parts[1].startswith(("`src/", "`vendor/")):
            continue
        name = parts[1].strip("`")
        if name in matrix:
            raise ValueError("重复源文件: " + name)
        method, target, state = parts[4:7]
        status = ("批准排除" if method.startswith("排除") else
                  "已登记验收" if state.startswith("已验收") else
                  "未迁移" if state.startswith("未迁移") else "部分实现")
        matrix[name] = (status, target, state)
    with zipfile.ZipFile(spec_path) as spec:
        names = spec.namelist()
        file_list = next((x for x in names if x.endswith("/源码导航/文件归属.tsv")), None)
        if not file_list:
            raise ValueError("规格包缺少源码导航/文件归属.tsv")
        source_rows = list(csv.DictReader(
            io.StringIO(spec.read(file_list).decode("utf-8-sig")), delimiter="\t"))
        docs = {m.group(2): m.group(1) + "_" + m.group(2) + ".md"
                for name in names
                if (m := re.search(r"/功能模块/(\d{2})_(.+)\.md$", name))}
        # Each document's 0.9 section lists the concrete upstream function
        # entrypoints and source-line ranges. Preserve them as a separate
        # manual acceptance queue; a path mapping alone cannot close a function.
        function_rows = []
        function_pattern = re.compile(
            r"(?m)^- `([^`]+)` — `source/([^`\:]+):(\d+)-(\d+)`"
        )
        for archive_name in names:
            match = re.search(r"/功能模块/(\d{2})_(.+)\.md$", archive_name)
            if not match:
                continue
            document = match.group(1) + "_" + match.group(2) + ".md"
            document_text = spec.read(archive_name).decode("utf-8")
            if "### 0.9" not in document_text:
                raise ValueError("规格文档缺少第0.9函数索引：" + document)
            section = document_text.split("### 0.9", 1)[1].split("\n---\n", 1)[0]
            functions = function_pattern.findall(section)
            if not functions:
                raise ValueError("规格文档未列出关键函数：" + document)
            for name, source_path, first, last in functions:
                if source_path not in matrix or int(first) > int(last):
                    raise ValueError("规格关键函数源范围无效：" + document + ":" + name)
                status, target, details = matrix[source_path]
                function_rows.append(dict(
                    document=document, function=name, source=source_path,
                    source_start=int(first), source_end=int(last),
                    rust_target=target, path_status=status,
                    function_verified=False,
                    acceptance="已批准排除" if status == "批准排除" else "待函数级源码与运行行为验收",
                ))
        # The current enhanced artifact carries nine declared upstream
        # entrypoints per module. A lost section must fail reconciliation.
        if len(function_rows) != 360:
            raise ValueError(f"预期360个原版函数入口，实际{len(function_rows)}")
    if len(docs) != 40 or len(source_rows) != 1902 or len(matrix) != 1906:
        raise ValueError("40文档/1902第一方源码/1906冻结路径数量不一致")
    original = zipfile.ZipFile(original_path) if original_path else None
    seen = set()
    groups = defaultdict(list)
    try:
        for row in source_rows:
            path = row["源文件"].removeprefix("source/")
            if path in seen or path not in matrix or row["功能模块"] not in docs:
                raise ValueError("缺失、重复或无归属路径: " + path)
            seen.add(path)
            source_ok = None
            if original:
                name = "claude-code-2.1.88-main/" + row["源文件"]
                try:
                    source_ok = hashlib.sha256(original.read(name)).hexdigest() == row["SHA256"]
                except KeyError:
                    source_ok = False
                if not source_ok:
                    raise ValueError("原源码和规格SHA不一致: " + path)
            state, target, details = matrix[path]
            groups[row["功能模块"]].append(dict(
                source=path, status=state, rust=target,
                declared_behavior=details, original_sha256=row["SHA256"],
                source_sha_verified=source_ok))
    finally:
        if original:
            original.close()
    summary = []
    for title, filename in sorted(docs.items(), key=lambda kv: kv[1]):
        items = groups[title]
        c = Counter(r["status"] for r in items)
        summary.append(dict(document=filename, total=len(items),
            registered=c["已登记验收"], partial=c["部分实现"],
            unported=c["未迁移"], excluded=c["批准排除"],
            closed=False))
    return dict(modules=groups, summary=summary, functions=function_rows,
                original_files=1902, baseline=1906)

def main():
    p = argparse.ArgumentParser(description="40份产品规格与Rust功能实现逐源码对照工作台")
    p.add_argument("--spec", type=Path, required=True)
    p.add_argument("--original-source", type=Path)
    p.add_argument("--matrix", type=Path, default=Path("docs/FILE_PORT_MATRIX.md"))
    p.add_argument("--function-csv-out", type=Path)
    p.add_argument("--json-out", type=Path)
    p.add_argument("--markdown-out", type=Path)
    args = p.parse_args()
    data = load(args.spec, args.matrix, args.original_source)
    lines = ["# 增强版40份规格逐文档闭环工作台", "",
        "本表由上传规格包和唯一迁移台账生成；登记验收不等于双端行为验收。",
        "单文档必须满足全部非排除函数、真实入口、异常、权限、取消、恢复和回归才能关闭。",
        "", "| 文档 | 路径 | 已登记验收 | 部分实现 | 未迁移 | 批准排除 | 结论 |",
        "|---|---:|---:|---:|---:|---:|---|"]
    for m in data["summary"]:
        lines.append(f'| {m["document"]} | {m["total"]} | {m["registered"]} | '
                     f'{m["partial"]} | {m["unported"]} | {m["excluded"]} | 待完整行为验收 |')
    result = "\n".join(lines) + "\n"
    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(result, encoding="utf-8")
    else:
        print(result)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    if args.function_csv_out:
        args.function_csv_out.parent.mkdir(parents=True, exist_ok=True)
        with args.function_csv_out.open("w", encoding="utf-8-sig", newline="") as output:
            fieldnames = ["document", "function", "source", "source_start",
                          "source_end", "rust_target", "path_status",
                          "function_verified", "acceptance"]
            writer = csv.DictWriter(output, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(data["functions"])

if __name__ == "__main__":
    sys.exit(main())
