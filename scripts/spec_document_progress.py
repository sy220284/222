#!/usr/bin/env python3
"""Audit all spec ZIP entries against the repository migration source of truth.

Inventory records are not evidence of behavioral equivalence. This command
never changes PORT_BASELINE, FILE_PORT_MATRIX, REMOVED_SCOPE, or PORT_EVIDENCE.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import io
import json
from pathlib import Path
import re
from zipfile import ZipFile

MAPPING_TSV = '核验数据/文件与模块双向映射.tsv'
MATRIX_RE = re.compile(r'^\|\s*`([^`]+)`\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|([^|]+)\|([^|]+)\|([^|]+)\|\s*$')
RELATED_SOURCE = re.compile(r'source/src/([\w./-]+\.(?:tsx?|jsx?))')


def read_matrix(repo: Path) -> dict[str, dict[str, str]]:
    rows = {}
    for number, line in enumerate((repo / 'docs/FILE_PORT_MATRIX.md').read_text(encoding='utf8').splitlines(), 1):
        if not line.startswith('| `'):
            continue
        match = MATRIX_RE.fullmatch(line)
        if not match:
            raise ValueError(f'Unparseable migration row at line {number}')
        path, source_lines, bytes_count, disposition, target, status = (v.strip() for v in match.groups())
        if path in rows:
            raise ValueError(f'Duplicate migration source {path}')
        if status.startswith('按用户要求删除'):
            stage = '已授权排除'
        elif status.startswith('已验收'):
            stage = '历史已验收（本轮待回归）'
        elif status.startswith(('部分实现', '部分覆盖')):
            stage = '部分实现（仍需验收）'
        elif status.startswith('未迁移'):
            stage = '未迁移'
        else:
            raise ValueError(f'Unknown migration status at {path}: {status}')
        rows[path] = {'source_lines':source_lines, 'source_bytes':bytes_count,
                      'disposition':disposition, 'rust_mapping':target,
                      'stage':stage, 'detail':status}
    return rows


def artifact_kind(name: str) -> str:
    prefix = name.split('/', 1)[0]
    return {
        '逐文件逐行':'第一方逐文件规格',
        '第三方依赖逐行':'依赖源码逐行规格',
        '功能模块':'模块综述',
        '外部依赖模块':'依赖说明',
        '专项契约':'专项契约',
        '核验数据':'核验数据',
    }.get(prefix, prefix)


def build(archive: Path, repo: Path) -> tuple[list[dict], list[dict], dict]:
    matrix = read_matrix(repo)
    baseline = json.loads((repo/'docs/PORT_BASELINE.json').read_text(encoding='utf8'))
    approved = json.loads((repo/'docs/REMOVED_SCOPE.json').read_text(encoding='utf8'))
    approved_reasons = {r['path']:r['reason'] for r in approved['items']}
    if len(matrix) != baseline['expected_total'] or set(matrix) != set(baseline['source_hashes']):
        raise ValueError('Matrix and frozen source baseline disagree')
    exclusion_paths = {p for p, r in matrix.items() if r['stage']=='已授权排除'}
    if not set(approved_reasons).issubset(exclusion_paths):
        raise ValueError('Removed scope does not match the actual approved exclusions')

    with ZipFile(archive) as z:
        mapping = list(csv.DictReader(io.StringIO(z.read(MAPPING_TSV).decode('utf8')), delimiter='\t'))
        source_to_doc = {}
        doc_to_source = {}
        module_of = {}
        for info in mapping:
            path = 'src/' + info['源文件']
            doc = info['对应详细文档']
            if path in source_to_doc or doc in doc_to_source:
                raise ValueError(f'Duplicate detailed document mapping: {doc}')
            if path not in matrix:
                raise ValueError(f'Missing migration matrix row: {path}')
            if baseline['source_hashes'][path] != info['源文件sha256']:
                raise ValueError(f'Source fingerprint differs from baseline: {path}')
            source_to_doc[path] = doc
            doc_to_source[doc] = path
            module_of[path] = info['模块']
        infos = [v for v in z.infolist() if not v.is_dir()]
        all_names = {v.filename for v in infos}
        if not set(doc_to_source).issubset(all_names):
            raise ValueError('Spec archive misses a mapped original source document')
        docs = []
        for index, item in enumerate(infos, 1):
            name = item.filename
            kind = artifact_kind(name)
            source = doc_to_source.get(name, '')
            if not source and kind == '专项契约' and name.endswith('.md'):
                match = RELATED_SOURCE.search(z.read(name)[:12000].decode('utf8', errors='replace'))
                if match:
                    candidate = 'src/' + match.group(1)
                    if candidate in matrix:
                        source = candidate
            entry = matrix.get(source)
            if entry:
                stage = entry['stage']
                action = ('已授权排除' if stage == '已授权排除'
                          else '待逐函数检查、实际调用链及行为测试验收')
            else:
                stage = ('依赖参考资料，未单独计入1,906条' if '依赖' in kind
                         else '关联证据或总览，待关联检查')
                action = '待对照关联源码或模块'
            docs.append({
                '序号': index, '规格文档路径': name, '文档类型': kind,
                '归属模块': module_of.get(source,''), '原版源码': source,
                '当前迁移状态': stage,
                '文档处理状态': action,
                'Rust实现入口': entry['rust_mapping'] if entry else '',
                '既有差异/验收记录': entry['detail'] if entry else '',
                '文档字节数': item.file_size,
                '文档SHA256': hashlib.sha256(z.read(name)).hexdigest(),
            })
    exclusions = []
    for path in sorted(exclusion_paths):
        entry = matrix[path]
        exclusions.append({'原版源码':path, '排除原因': approved_reasons.get(path, entry['disposition']),
                           '授权出处':'REMOVED_SCOPE.json' if path in approved_reasons else '原始迁移矩阵',
                           '现有状态':entry['detail']})
    details = [d for d in docs if d['文档类型']=='第一方逐文件规格']
    if len(details) != len(source_to_doc):
        raise ValueError('Some original source docs were not represented exactly once')
    stats = Counter(r['stage'] for r in matrix.values())
    modules = defaultdict(Counter)
    for path in source_to_doc:
        modules[module_of[path]][matrix[path]['stage']] += 1
    summary = {'archive_members':len(docs),'original_source_documents':len(details),
               'migration_paths':len(matrix),'approved_exclusions':len(exclusions),
               'statuses':dict(stats), 'modules':{k:dict(v) for k,v in sorted(modules.items())},
               'unmapped_original_paths':sorted(set(matrix)-set(source_to_doc))}
    return docs, exclusions, summary


def write_csv(path: Path, entries: list[dict]) -> None:
    if not entries:
        raise ValueError(f'No records: {path}')
    with path.open('w', encoding='utf-8-sig', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=list(entries[0]))
        writer.writeheader()
        writer.writerows(entries)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', required=True, type=Path, help='User-supplied documentation ZIP')
    parser.add_argument('--repo', type=Path, default=Path('.'))
    parser.add_argument('--output', type=Path, default=Path('.spec-document-progress'))
    args = parser.parse_args()
    docs, exclusions, summary = build(args.archive, args.repo)
    args.output.mkdir(parents=True, exist_ok=True)
    write_csv(args.output/'SPEC_DOCUMENT_PROGRESS.csv', docs)
    write_csv(args.output/'SPEC_DOCUMENT_EXCLUSIONS.csv', exclusions)
    (args.output/'SPEC_DOCUMENT_SUMMARY.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2)+'\n',encoding='utf8')
    print(json.dumps(summary,ensure_ascii=False))


if __name__ == '__main__':
    main()
