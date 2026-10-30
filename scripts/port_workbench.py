#!/usr/bin/env python3
"""Reproducible source-to-Rust migration inventory and evidence checks.

This tool does NOT infer behavior equivalence from filenames, symbol matches,
compilation or static source. The source archive remains on the user's machine.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import sys

MATRIX = Path('docs/FILE_PORT_MATRIX.md')
EVIDENCE = Path('docs/PORT_EVIDENCE.jsonl')
BASELINE = Path('docs/PORT_BASELINE.json')
ROW_PATTERN = re.compile(r'^\|\s*`([^`]+)`\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|([^|]+)\|([^|]+)\|([^|]+)\|\s*$')
IMPORT_PATTERN = re.compile(
    r'\b(?:import|export)\s+(?:[\s\S]{0,600}?\s+from\s*)?[\'\"](\.{1,2}/[^\'\"]+)[\'\"]'
    r'|\brequire\s*\(\s*[\'\"](\.{1,2}/[^\'\"]+)[\'\"]\s*\)'
    r'|\bimport\s*\(\s*[\'\"](\.{1,2}/[^\'\"]+)[\'\"]\s*\)',
    re.MULTILINE,
)
# Heuristic inventory only; neither a JS parser nor a functional equivalence proof.
EXPORT_PATTERN = re.compile(
    r'^\s*export\s+(?:(?:default|declare|async|abstract)\s+)*'
    r'(?:function|class|interface|type|enum|const|let|var|namespace)\s+([A-Za-z_$][\w$]*)',
    re.MULTILINE,
)
EXPECTED_TOTAL = 1906
EXPECTED_EXCLUDED = 22
ENTRYPOINTS = ['src/main.rs', 'src/agent.rs', 'src/tools.rs', 'src/mcp.rs', 'src/sdk_queue.rs']
CRITICAL = {'agent': 7, 'commands': 6, 'bridge': 8, 'cli': 6, 'services': 7,
            'tools': 7, 'tasks': 7, 'hooks': 6, 'ink': 6, 'components': 5,
            'utils': 4, 'context': 6, 'keybindings': 5, 'vendor': 5}


class AuditError(ValueError):
    pass


def label_status(text: str) -> str:
    if text.startswith('未迁移'):
        return 'unported'
    if text.startswith('已验收'):
        return 'verified'
    if text.startswith('按用户要求删除'):
        return 'excluded'
    if text.startswith(('部分实现', '部分覆盖')):
        return 'partial'
    raise AuditError('未知迁移状态: ' + text)


def read_matrix(repo: Path) -> dict[str, dict]:
    p = repo / MATRIX
    if not p.is_file():
        raise AuditError(f'不存在逐文件台账: {p}')
    rows: dict[str, dict] = {}
    for line_number, line in enumerate(p.read_text(encoding='utf-8').splitlines(), 1):
        if not line.startswith('| `'):
            continue
        match = ROW_PATTERN.fullmatch(line)
        if not match:
            raise AuditError(f'{p}:{line_number}: 表格行无法解析（不能跳过）')
        path, lines, size, disposition, rust, status = (s.strip() for s in match.groups())
        if path in rows:
            raise AuditError(f'{p}:{line_number}: 重复原路径 {path}')
        if not (path.startswith('src/') or path.startswith('vendor/')):
            raise AuditError(f'{p}:{line_number}: 非法路径 {path}')
        rows[path] = dict(path=path, source_lines=int(lines), source_bytes=int(size),
                          disposition=disposition, rust_mapping=rust,
                          status=label_status(status), detail=status, matrix_line=line_number)
    if not rows:
        raise AuditError(f'{p}: 未找到任何原路径行')
    return rows


def verify_counts(rows: dict[str, dict], strict: bool = True) -> Counter:
    counts = Counter(r['status'] for r in rows.values())
    if strict and (len(rows) != EXPECTED_TOTAL or counts['excluded'] != EXPECTED_EXCLUDED):
        raise AuditError(f'原路径台账异常：当前 {len(rows)} 条、排除 {counts["excluded"]} 条；'
                         f'期望 {EXPECTED_TOTAL} / {EXPECTED_EXCLUDED}。')
    if len(rows) != sum(counts.values()):
        raise AuditError('原路径分类计数不平衡')
    return counts


def resolve_import(origin: str, value: str, available: set[str]) -> str | None:
    if not value.startswith('.'):
        return None
    parts: list[str] = origin.split('/')[:-1]
    for item in value.split('/'):
        if item in ('', '.'):
            continue
        if item == '..':
            if not parts:
                return None
            parts.pop()
        else:
            parts.append(item)
    path = '/'.join(parts)
    if path.endswith(('.js', '.jsx', '.mjs')):
        path = re.sub(r'\.(?:jsx?|mjs)$', '', path)
    elif path.endswith(('.ts', '.tsx')):
        path = re.sub(r'\.tsx?$', '', path)
    return next((p for p in (path, path + '.ts', path + '.tsx', path + '.js',
                             path + '/index.ts', path + '/index.tsx', path + '/index.js')
                 if p in available), None)


def rust_targets(raw: str) -> list[str]:
    return sorted(set(re.findall(r'`?(src/[\w./-]+\.rs)`?', raw)))


def collect(rows: dict[str, dict], repo: Path, source: Path) -> tuple[list[dict], dict]:
    available = set(rows)
    dependencies = {}
    errors = []
    for name, row in rows.items():
        file = source / name
        if not file.is_file():
            errors.append(f'原版源码缺失: {name}')
            continue
        data = file.read_bytes()
        actual_size = len(data)
        # splitlines mirrors the usual file line-count semantics, including no final newline.
        actual_lines = data.count(b'\n') + 1
        if (actual_size, actual_lines) != (row['source_bytes'], row['source_lines']):
            errors.append(f'原版源码与台账不一致: {name}: 原始 {actual_lines}/{actual_size}, '
                          f'台账 {row["source_lines"]}/{row["source_bytes"]}')
        text = data.decode('utf-8')
        refs = set()
        for m in IMPORT_PATTERN.finditer(text):
            name_ref = next((g for g in m.groups() if g is not None), None)
            resolved = resolve_import(name, name_ref, available) if name_ref else None
            if resolved and resolved != name:
                refs.add(resolved)
        dependencies[name] = refs
        row['sha256'] = hashlib.sha256(data).hexdigest()
        row['declared_exports'] = sorted(set(EXPORT_PATTERN.findall(text)))
        row['dependencies'] = sorted(refs)
        targets = rust_targets(row['rust_mapping'])
        row['rust_targets'] = targets
        row['mapped_rust_missing'] = sorted(t for t in targets if not (repo / t).is_file())
        row['mapped_rust_present'] = sorted(t for t in targets if (repo / t).is_file())
    if errors:
        raise AuditError('\n'.join(errors[:25]) +
                         (f'\n另有 {len(errors)-25} 条异常' if len(errors) > 25 else ''))

    consumers = defaultdict(set)
    for source_name, refs in dependencies.items():
        for dep in refs:
            consumers[dep].add(source_name)
    dependents = {k: len(v) for k, v in consumers.items()}
    all_rows = []
    for name, row in rows.items():
        prefix = name.split('/')[1] if name.startswith('src/') else 'vendor'
        group = prefix if prefix in CRITICAL else 'other'
        score = (CRITICAL.get(group, 3) * 10 + min(100, 3 * dependents.get(name, 0))
                 + min(30, len(row['dependencies'])) + (10 if row['status']=='unported' else 0))
        all_rows.append({**row, 'group': group, 'direct_consumers': dependents.get(name, 0),
                         'priority_score': score, 'consumers_sample': sorted(consumers[name])[:25]})
    groups = defaultdict(lambda: {'unported': 0, 'partial': 0, 'verified': 0, 'excluded': 0,
                                  'source_lines_open': 0, 'cross_references': 0, 'entries': []})
    for row in all_rows:
        group = groups[row['group']]
        group[row['status']] += 1
        if row['status'] in ('partial', 'unported'):
            group['source_lines_open'] += row['source_lines']
            group['cross_references'] += row['direct_consumers']
            group['entries'].append(row['path'])
    return all_rows, {k: dict(v, entries=sorted(v['entries'])) for k, v in groups.items()}


def json_dump(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + '\n',
                    encoding='utf-8')


def report(rows: dict[str, dict], repo: Path, source: Path, output: Path, strict: bool) -> dict:
    counts = verify_counts(rows, strict=strict)
    all_rows, groups = collect(rows, repo, source)
    priority = sorted((r for r in all_rows if r['status'] in ('unported', 'partial')),
                      key=lambda r: (-r['priority_score'], -r['direct_consumers'], r['path']))
    ranked_groups = sorted(groups.items(), key=lambda pair: (
        -(pair[1]['unported'] * 12 + pair[1]['partial'] * 5 + pair[1]['cross_references']),
        pair[0]))
    inventory = dict(schema='port-workbench/v1', source_root=str(source.resolve()),
                     repo_root=str(repo.resolve()), total=len(rows), status_counts=dict(counts),
                     source_manifest_sha256=hashlib.sha256((repo/MATRIX).read_bytes()).hexdigest(),
                     entries=all_rows, groups=groups)
    json_dump(output / 'source-port-inventory.json', inventory)
    json_dump(output / 'port-work-plan.json', dict(schema='port-work-plan/v1',
        disclaimer='静态导入关系只用于任务排队，不能作为 Rust 完成或行为等价证据。',
        groups=[dict(name=n, **{k:v for k,v in g.items() if k!='entries'},
                     candidates=[r['path'] for r in priority if r['group']==n][:45])
                for n,g in ranked_groups],
        prioritized_paths=[dict(path=r['path'], status=r['status'],
                 dependencies=r['dependencies'], consumers=r['consumers_sample'],
                 existing_rust=r['mapped_rust_present'], priority=r['priority_score'])
                for r in priority]))
    summary = [
        '# 222 原版源码与 Rust 迁移工作台（自动生成）', '',
        '> 本报告仅显示静态范围、依赖与待办优先级。不得据此认定代码已完成或行为等价。', '',
        f'- 原始路径：{len(rows)}',
        f'- 未迁移：{counts["unported"]}',
        f'- 部分实现/覆盖：{counts["partial"]}',
        f'- 已登记独立验收：{counts["verified"]}',
        f'- 排除：{counts["excluded"]}',
        f'- 待源码级完整验收：{counts["unported"] + counts["partial"]}',
        '', '## 按完整业务域归组', '',
        '| 业务域 | 未迁移 | 部分实现 | 待处理原行数 | 被依赖次数 |',
        '|---|---:|---:|---:|---:|',
    ]
    for n,g in ranked_groups:
        if g['unported'] or g['partial']:
            summary.append(f'| {n} | {g["unported"]} | {g["partial"]} | '
                           f'{g["source_lines_open"]} | {g["cross_references"]} |')
    summary.extend(['', '## 重点共享依赖路径（辅助定位整条功能链）', '',
                    '| 原路径 | 状态 | 直接依赖者 | 原始行数 | 已登记 Rust 对应 |',
                    '|---|---|---:|---:|---|'])
    for r in priority[:50]:
        summary.append(f'| `{r["path"]}` | {r["status"]} | {r["direct_consumers"]} | '
                       f'{r["source_lines"]} | {", ".join(r["rust_targets"]) or "未登记"} |')
    summary.extend(['', '## 完成标准', '',
        '每项必须有：原版源码函数及异常语义 → Rust 实际入口及下游 → 正常/错误/取消/恢复验证 → 当前提交与仍存差异。',
        '只有完整证据存在时才更新为已验收；未测试的代码、AST 映射、仅有编译成功均不得核销。', ''])
    output.mkdir(parents=True, exist_ok=True)
    (output/'PORT_WORK_PLAN.md').write_text('\n'.join(summary), encoding='utf-8')
    return {'counts': dict(counts), 'outputs': [str(output/f) for f in
             ('source-port-inventory.json','port-work-plan.json','PORT_WORK_PLAN.md')],
            'priority_top': [r['path'] for r in priority[:15]]}


def affected_source_paths(entries: list[dict], changed_files: list[str]) -> dict:
    """Find source paths and their direct/transitive import consumers to recheck."""
    by_path = {row['path']: row for row in entries}
    reverse: dict[str, set[str]] = defaultdict(set)
    for row in entries:
        for dep in row['dependencies']:
            reverse[dep].add(row['path'])
    seeds: set[str] = set()
    unmatched = []
    for changed in changed_files:
        found = {r['path'] for r in entries if r['path']==changed or changed in r['rust_targets']}
        if not found:
            unmatched.append(changed)
        seeds.update(found)
    affected = set(seeds)
    frontier = list(seeds)
    while frontier:
        for caller in reverse[frontier.pop()]:
            if caller not in affected:
                affected.add(caller)
                frontier.append(caller)
    return dict(changed_files=changed_files, direct_source_paths=sorted(seeds),
                impacted_source_paths=sorted(affected), unmatched=unmatched,
                disclaimers='静态依赖受影响范围仅辅助回归；动态加载、运行时注册和外部协议必须额外检查')


def check_evidence(repo: Path, rows: dict[str, dict], strict: bool) -> dict:
    counts = verify_counts(rows, strict=strict)
    baseline_path = repo / BASELINE
    if not baseline_path.is_file():
        raise AuditError(f'缺少冻结的起始状态快照：{baseline_path}')
    baseline = json.loads(baseline_path.read_text(encoding='utf-8'))
    previous = baseline.get('statuses')
    if not isinstance(previous, dict) or set(previous) != set(rows):
        raise AuditError('起始快照与逐文件台账路径集合不一致')
    evidence_path = repo / EVIDENCE
    proof = {}
    if evidence_path.exists():
        for ln, line in enumerate(evidence_path.read_text(encoding='utf-8').splitlines(), 1):
            if not line.strip():
                continue
            item = json.loads(line)
            path = item.get('path')
            if path in proof:
                raise AuditError(f'{evidence_path}:{ln} 重复证明: {path}')
            proof[path] = item
    newly_verified = [p for p,r in rows.items()
                      if r['status']=='verified' and previous[p]!='verified']
    source_hashes = baseline.get('source_hashes', {})
    if set(source_hashes) != set(rows):
        raise AuditError('缺少 1,906 条原版文件 SHA-256 锁定值')
    for path in newly_verified:
        item = proof.get(path)
        required = ('source_sha256', 'rust_entrypoints', 'test_names',
                    'tested_commit', 'failure_and_recovery_verified', 'remaining_gaps')
        if not item or any(not item.get(k) and k!='remaining_gaps' for k in required):
            raise AuditError(f'{path}: 标记已验收前必须提供完整可追溯证据')
        if item['source_sha256'] != source_hashes[path]:
            raise AuditError(f'{path}: 原版 SHA-256 与冻结源码不一致')
        if item['remaining_gaps']:
            raise AuditError(f'{path}: 存在未解决差异，禁止改为已验收')
        if item['failure_and_recovery_verified'] is not True:
            raise AuditError(f'{path}: 故障与恢复未验收')
        for entry in item['rust_entrypoints']:
            if not (repo / entry).is_file():
                raise AuditError(f'{path}: Rust 生产入口不存在: {entry}')
        rust_sources = [p.read_text(encoding='utf-8') for p in (repo/'src').rglob('*.rs')]
        for test in item['test_names']:
            token = test.split('::')[-1]
            if not re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', token):
                raise AuditError(f'{path}: 测试标识不合法: {test}')
            if not any(re.search(r'\bfn\s+' + re.escape(token) + r'\s*\(', s) for s in rust_sources):
                raise AuditError(f'{path}: Rust 测试函数不存在: {test}')
        if not re.fullmatch(r'[0-9a-f]{40}', item['tested_commit']):
            raise AuditError(f'{path}: tested_commit 必须是有效完整提交 SHA')
    returned = [p for p,r in rows.items() if r['status']=='excluded' and previous[p]!='excluded']
    if returned:
        raise AuditError('新增排除项需人工审批并重置基线: '+', '.join(returned))
    return dict(total=len(rows), counts=dict(counts), new_verified=len(newly_verified),
                evidence_records=len(proof), excluded=counts['excluded'])


def current_counts(rows: dict[str, dict], strict: bool = True) -> dict:
    """Derive the sole current progress snapshot from FILE_PORT_MATRIX.md."""
    counts = verify_counts(rows, strict=strict)
    total = len(rows)
    excluded = counts['excluded']
    eligible = total - excluded
    accepted = counts['verified']
    partial = counts['partial']
    unported = counts['unported']
    remaining = partial + unported
    if accepted + remaining != eligible:
        raise AuditError('非排除路径的状态数量不平衡')
    return dict(total=total, excluded=excluded, eligible=eligible, accepted=accepted,
                partial=partial, unported=unported, remaining=remaining,
                accepted_percent=round(accepted * 100 / eligible, 2) if eligible else 0.0)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('scan', 'check', 'freeze-baseline', 'impact', 'counts'))
    parser.add_argument('--repo', type=Path, default=Path('.'))
    parser.add_argument('--source', type=Path, help='解压后的 2.1.88 原始源码根目录')
    parser.add_argument('--output', type=Path, default=Path('port-audit-output'))
    parser.add_argument('--allow-fixture', action='store_true', help='仅供单元测试使用')
    parser.add_argument('--changed', action='append', default=[], help='变更过的原路径或 Rust 路径，可重复指定')
    args = parser.parse_args(argv)
    try:
        rows = read_matrix(args.repo)
        counts = verify_counts(rows, strict=not args.allow_fixture)
        if args.command == 'counts':
            result = current_counts(rows, strict=not args.allow_fixture)
        elif args.command == 'impact':
            if args.source is None or not args.changed:
                raise AuditError('impact 必须提供 --source 和至少一项 --changed')
            entries, _ = collect(rows, args.repo, args.source)
            result = affected_source_paths(entries, args.changed)
            json_dump(args.output / 'change-impact.json', result)
        elif args.command == 'scan':
            if args.source is None:
                raise AuditError('scan 必须提供 --source 原版源码根目录')
            result = report(rows, args.repo, args.source, args.output,
                            strict=not args.allow_fixture)
        elif args.command == 'check':
            result = check_evidence(args.repo, rows, strict=not args.allow_fixture)
        else:
            # This operation is explicitly separate; it never changes any migration row.
            p = args.repo / BASELINE
            if p.exists():
                raise AuditError('已有冻结快照；拒绝覆盖以规避新增验收检查')
            if args.source is None:
                raise AuditError('冻结基线必须提供 --source，并核对全部原始源码')
            inventory, _ = collect(rows, args.repo, args.source)
            result = dict(schema='port-baseline/v1', expected_total=len(rows),
                          source_hashes={r['path']:r['sha256'] for r in inventory},
                          statuses={p:r['status'] for p,r in rows.items()})
            json_dump(p, result)
            result = {'baseline':str(p), 'counts':dict(counts)}
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (AuditError, OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        print(f'迁移工作台失败: {exc}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
