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
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.request
import urllib.error
from functools import lru_cache
import time

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
EXPECTED_EXCLUDED = 192
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


def generated_runtime_reachability(repo: Path, rows: dict[str, dict]) -> dict:
    """Classify generated adapters by *external code references*, not parity.

    The registry and InspectSource CLI are intentionally excluded: a registry
    export is inspectable data, not proof that the user-facing runtime calls it.
    This is a static search and may include references in unreachable code.
    """
    base = repo / 'src/generated_ports'
    modules = sorted(base.glob('p_*.rs')) if base.is_dir() else []
    references: dict[str, set[str]] = defaultdict(set)
    pattern = re.compile(r'\b(p_[0-9a-f]{8,16})\s*::')
    root = repo / 'src'
    for file in sorted(root.rglob('*.rs')):
        if base in file.parents:
            continue
        text = file.read_text(encoding='utf-8')
        for name in set(pattern.findall(text)):
            references[name].add(str(file.relative_to(repo)))
    mapping: dict[str, list[str]] = defaultdict(list)
    for row in rows.values():
        for target in rust_targets(row['rust_mapping']):
            if target.startswith('src/generated_ports/p_'):
                mapping[Path(target).stem].append(row['path'])
    entries = []
    for module in modules:
        name = module.stem
        callers = sorted(references.get(name, ()))
        source_paths = sorted(mapping.get(name, ()))
        entries.append(dict(module=str(module.relative_to(repo)),
                            source_paths=source_paths,
                            non_registry_references=callers,
                            classification='outside_reference' if callers else 'inspect_only'))
    return dict(schema='generated-reachability/v1',
                disclaimer='静态模块引用不是业务可达性或等价验收证明；inspect_only 仅表示只可经 registry/InspectSource 路径查询。',
                generated_modules=len(entries),
                inspect_only=sum(e['classification']=='inspect_only' for e in entries),
                outside_reference=sum(e['classification']=='outside_reference' for e in entries),
                entries=entries)


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


@lru_cache(maxsize=128)
def github_json(url: str) -> dict:
    """Fail closed on unverifiable evidence, retry bounded transient API errors.

    Multiple verified paths often reference the same workflow; cache only
    successful responses within this check process to avoid repeated calls.
    """
    headers = {'Accept': 'application/vnd.github+json',
               'User-Agent': 'atlas-agent-port-workbench'}
    token = os.environ.get('GITHUB_TOKEN')
    if token:
        headers['Authorization'] = 'Bearer ' + token
    request = urllib.request.Request(url, headers=headers)
    max_attempts = 7
    for attempt in range(max_attempts):
        try:
            with urllib.request.urlopen(request, timeout=12) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            retryable = error.code in (429, 500, 502, 503, 504)
            if error.code == 403 and (
                error.headers.get('Retry-After')
                or error.headers.get('X-RateLimit-Remaining') == '0'
            ):
                retryable = True
            if not retryable or attempt + 1 == max_attempts:
                raise AuditError(
                    f'无法核验 GitHub Actions 测试运行证据: HTTP {error.code}, '
                    f'URL={url}'
                ) from error
        except (urllib.error.URLError, TimeoutError) as error:
            if attempt + 1 == max_attempts:
                raise AuditError(
                    f'无法核验 GitHub Actions 测试运行证据: '
                    f'{type(error).__name__}, URL={url}'
                ) from error
        # GitHub's historical Actions jobs endpoint can return several
        # consecutive 502s; maintain strict evidence checks and retry
        # with capped backoff instead of treating the missing data as success.
        time.sleep(min(2 ** attempt, 12))
    raise AssertionError('Unreachable: all evidence fetch attempts exhausted')


def verify_github_test_evidence(item: dict, repository: str | None = None,
                                fetch_json=None) -> None:
    """Require the exact tested commit to have a successful *real test job*.

    Static test function names, a syntactically valid SHA or a successful
    formatting job are insufficient proof of a source-path parity promotion.
    """
    run_id = item.get('workflow_run_id')
    if type(run_id) is not int or run_id < 1:
        raise AuditError('已验收证据缺少有效 workflow_run_id')
    repository = repository or os.environ.get('GITHUB_REPOSITORY', 'sy220284/222')
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        raise AuditError('非法 GitHub 仓库名称')
    fetch_json = fetch_json or github_json
    url = f'https://api.github.com/repos/{repository}/actions/runs/{run_id}'
    run = fetch_json(url)
    if run.get('head_sha') != item.get('tested_commit'):
        raise AuditError('验收工作流实际 HEAD 与 tested_commit 不匹配')
    if run.get('conclusion') != 'success':
        raise AuditError('验收工作流未成功完成')
    if run.get('repository', {}).get('full_name') not in (None, repository):
        raise AuditError('验收工作流不属于目标仓库')
    jobs = fetch_json(url + '/jobs?per_page=100').get('jobs', [])
    accepted = {'编译与全部测试', 'Windows Shell 与跨平台编译'}
    for job in jobs:
        if job.get('name') not in accepted or job.get('conclusion') != 'success':
            continue
        steps = job.get('steps', [])
        if any(step.get('conclusion') == 'success' and
               ('Rust 单元测试' in step.get('name', '') or
                '集成测试' in step.get('name', '')) for step in steps):
            return
    raise AuditError('没有实际执行成功的 Rust 测试作业，跳过的 Linux/Windows 测试不能用于验收')


def approved_removed_paths(repo: Path) -> set[str]:
    scope = repo / 'docs/REMOVED_SCOPE.json'
    if not scope.is_file():
        return set()
    content = json.loads(scope.read_text(encoding='utf-8'))
    if content.get('schema') != 'user-approved-removal-scope/v1':
        raise AuditError('移除范围授权文件版本无效')
    paths = [entry['path'] for entry in content['items']]
    if len(paths) != len(set(paths)):
        raise AuditError('移除范围授权路径重复')
    return set(paths)


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
    all_verified = [p for p,r in rows.items() if r['status']=='verified']
    source_hashes = baseline.get('source_hashes', {})
    if set(source_hashes) != set(rows):
        raise AuditError('缺少 1,906 条原版文件 SHA-256 锁定值')
    # Historic verified rows require the same auditable proof as newly promoted
    # rows. Otherwise deleting an old JSONL record silently retains acceptance.
    for path in all_verified:
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
        # Rust unit tests reside in src/, while end-to-end tests using the
        # real executable live in tests/. Both must be discoverable; a passed
        # integration test is stronger evidence than a source-only assertion.
        # Ignore docstrings and file extensions other than .rs.
        rust_sources = [
            source.read_text(encoding='utf-8')
            for directory in (repo / 'src', repo / 'tests')
            if directory.is_dir()
            for source in directory.rglob('*.rs')
        ]
        for test in item['test_names']:
            token = test.split('::')[-1]
            if not re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', token):
                raise AuditError(f'{path}: 测试标识不合法: {test}')
            if not any(re.search(r'\bfn\s+' + re.escape(token) + r'\s*\(', s) for s in rust_sources):
                raise AuditError(f'{path}: Rust 测试函数不存在: {test}')
        if not re.fullmatch(r'[0-9a-f]{40}', item['tested_commit']):
            raise AuditError(f'{path}: tested_commit 必须是有效完整提交 SHA')
        if strict and path in newly_verified:
            verify_github_test_evidence(item)
    returned = {p for p,r in rows.items() if r['status']=='excluded' and previous[p]!='excluded'}
    approved = approved_removed_paths(repo)
    if returned != approved:
        raise AuditError('新增排除项需人工审批并逐项登记；未获授权: ' +
                         ', '.join(sorted(returned - approved)) +
                         '；授权未应用: ' + ', '.join(sorted(approved - returned)))
    return dict(total=len(rows), counts=dict(counts), new_verified=len(newly_verified),
                evidence_records=len(proof), excluded=counts['excluded'])



def detect_generated_stubs(rows: dict[str, dict], repo: Path) -> dict:
    """Find concrete AST placeholders; don't confuse source coverage with behavior.

    The inventory reads Rust source bytes and the generated inspection catalog.
    Only actual `Value::Null` expressions in AST-generated modules are counted,
    and already handwritten ports are excluded from automatic 'stub' labeling.
    """
    registry_text = '\n'.join(p.read_text(encoding='utf-8')
                               for p in (repo / 'src/generated_ports').glob('registry_*.rs'))
    results = []
    for row in rows.values():
        if row['status'] == 'excluded':
            continue
        for file in rust_targets(row['rust_mapping']):
            if not re.fullmatch(r'src/generated_ports/p_[0-9a-f]{13}\.rs', file):
                continue
            path = repo / file
            if not path.is_file():
                raise AuditError(f'已登记的 Rust 模块不存在: {file}')
            code = path.read_text(encoding='utf-8')
            automatic = code.startswith(('//! Native Rust UI structure derived from original:',
                                         '//! Rust exports derived from original:'))
            if not automatic:
                continue
            nulls = len(re.findall(r'\bValue::Null\b', code))
            # `match export { _ => None }` means even the inspection adapter
            # has no entry for the generated function.
            source_pattern = re.compile('"' + re.escape(row['path']) + r'"\s*=>\s*match export\s*\{([^}]*)\}', re.S)
            match = source_pattern.search(registry_text)
            has_export = bool(match and re.search(r'"[A-Za-z_][A-Za-z0-9_]*"\s*=>', match.group(1)))
            has_render = bool(re.search(r'\bpub fn render_source_view\s*\(', code))
            if nulls or (has_render and not has_export):
                results.append(dict(source=row['path'], rust=file, source_lines=row['source_lines'],
                                    null_expression_count=nulls, view_export_unreachable=has_render and not has_export,
                                    rust_lines=len(code.splitlines())))
    results.sort(key=lambda r: (-int(r['view_export_unreachable']),
                                -r['null_expression_count'], -r['source_lines'], r['source']))
    return dict(schema='port-stubs/v1', disclaimer='仅是自动生成空壳的静态检测，不证明手写代码功能完整。',
                count=len(results), with_null=sum(r['null_expression_count']>0 for r in results),
                unreachable_views=sum(r['view_export_unreachable'] for r in results),
                total_null_expressions=sum(r['null_expression_count'] for r in results),
                items=results)



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




def git_changed_files(repo: Path) -> list[str]:
    """Inspect this commit's changed paths without depending on an external source tree."""
    process = subprocess.run(
        ['git', 'diff', '--name-only', '--no-ext-diff', 'HEAD^', 'HEAD'],
        cwd=repo, capture_output=True, text=True, check=False)
    if process.returncode:
        raise AuditError('无法获得当前提交变更文件，请确保存在 HEAD^ 并检出至少两个提交: '
                         + process.stderr.strip())
    return sorted(set(path.strip() for path in process.stdout.splitlines() if path.strip()))


def build_dashboard(repo: Path, rows: dict[str, dict], source: Path | None = None,
                    changed_files: list[str] | None = None, strict: bool = True) -> dict:
    """Combine the ONE matrix, frozen baseline and evidence into reviewable work.

    A Rust file mapping or a static dependency is a lead, never proof of
    callable runtime behavior. An old verified baseline may lack modern proof.
    """
    audit = check_evidence(repo, rows, strict=strict)
    snapshot = current_counts(rows, strict=strict)
    baseline = json.loads((repo / BASELINE).read_text(encoding='utf-8'))
    source_hashes = baseline['source_hashes']
    approved = approved_removed_paths(repo)
    proof = {}
    evidence_file = repo / EVIDENCE
    if evidence_file.exists():
        for line in evidence_file.read_text(encoding='utf-8').splitlines():
            if line.strip():
                item = json.loads(line)
                proof[item['path']] = item

    source_entries = None
    if source is not None:
        source_entries, _ = collect(rows, repo, source)
        for entry in source_entries:
            if entry['sha256'] != source_hashes[entry['path']]:
                raise AuditError('原版源码哈希与冻结基线不一致: ' + entry['path'])
    source_info = {item['path']: item for item in source_entries or []}

    usage = Counter(target for row in rows.values() if row['status'] != 'excluded'
                    for target in rust_targets(row['rust_mapping']))
    chains = []
    domains = defaultdict(lambda: {'verified': 0, 'partial': 0, 'unported': 0,
                                   'excluded': 0, 'missing_mapping': 0,
                                   'missing_rust': 0, 'legacy_verified': 0})
    for path, row in rows.items():
        targets = rust_targets(row['rust_mapping'])
        existing = [target for target in targets if (repo / target).is_file()]
        missing = sorted(set(targets) - set(existing))
        status = row['status']
        item = proof.get(path)
        evidence_state = ('not_applicable' if status == 'excluded' else
                          'recorded' if status == 'verified' and item else
                          'legacy_baseline_needs_recheck' if status == 'verified' else
                          'not_yet_verified')
        domain = (path.split('/')[1] if path.startswith('src/') else 'vendor')
        if domain not in CRITICAL:
            domain = 'other'
        domain_counts = domains[domain]
        domain_counts[status] += 1
        if status != 'excluded':
            if not targets:
                domain_counts['missing_mapping'] += 1
            if missing:
                domain_counts['missing_rust'] += 1
            if evidence_state == 'legacy_baseline_needs_recheck':
                domain_counts['legacy_verified'] += 1
        source_item = source_info.get(path)
        score = (CRITICAL.get(domain, 3) * 10 +
                 (80 if status == 'unported' else 45 if status == 'partial' else 0) +
                 8 * min(10, max((usage[target] for target in targets), default=0)) +
                 (25 if not targets else 0) +
                 (15 if missing else 0) +
                 (min(50, source_item['direct_consumers'] * 4) if source_item else 0))
        chains.append(dict(
            source_path=path, source_sha256=source_hashes[path], status=status,
            scope='excluded' if status == 'excluded' else 'required',
            authorized_exclusion=path in approved,
            domain=domain, source_lines=row['source_lines'],
            rust_targets=targets, rust_present=existing, rust_missing=missing,
            mapping_missing=status != 'excluded' and not targets,
            source_exports=source_item['declared_exports'] if source_item else None,
            source_dependencies=source_item['dependencies'] if source_item else None,
            source_consumers=source_item['consumers_sample'] if source_item else None,
            evidence=evidence_state,
            tested_commit=item.get('tested_commit') if item and status == 'verified' else None,
            workflow_run_id=item.get('workflow_run_id') if item and status == 'verified' else None,
            test_names=item.get('test_names', []) if item and status == 'verified' else [],
            verified_entrypoints=item.get('rust_entrypoints', []) if item and status == 'verified' else [],
            remaining_gaps=item.get('remaining_gaps', []) if item and status == 'verified' else [],
            current_gap=row['detail'] if status in ('partial', 'unported') else None,
            runtime_behavior='requires_real_entrypoint_test' if status != 'excluded' else 'excluded',
            priority=score if status in ('partial', 'unported') else None))

    impacted = None
    if changed_files:
        normalized = [Path(name).as_posix().removeprefix('./') for name in changed_files]
        if source_entries is not None:
            impacted = affected_source_paths(source_entries, normalized)
            impacted['analysis'] = 'source_import_closure'
        else:
            direct = {chain['source_path'] for chain in chains
                      if chain['source_path'] in normalized or
                      set(chain['rust_targets']).intersection(normalized)}
            unmatched = sorted(set(normalized) -
                               {name for name in normalized if any(
                                   chain['source_path'] == name or name in chain['rust_targets']
                                   for chain in chains)})
            impacted = dict(changed_files=normalized, direct_source_paths=sorted(direct),
                            impacted_source_paths=sorted(direct), unmatched=unmatched,
                            analysis='direct_mapping_only_no_source_dependency_graph',
                            disclaimers='需携带 --source 才能识别原版依赖者；动态调用仍须人工复核')
        impacted['verified_needs_recheck'] = sorted(
            chain['source_path'] for chain in chains
            if chain['source_path'] in impacted['impacted_source_paths']
            and chain['status'] == 'verified')
        impacted['nonexcluded_needs_recheck'] = sorted(
            chain['source_path'] for chain in chains
            if chain['source_path'] in impacted['impacted_source_paths']
            and chain['status'] != 'excluded')
        impacted['unmatched_require_manual_review'] = bool(impacted['unmatched'])

    queue = sorted((chain for chain in chains if chain['priority'] is not None),
                   key=lambda item: (-item['priority'], item['source_path']))
    domain_order = sorted(domains.items(), key=lambda pair: (
        -(pair[1]['unported'] * 4 + pair[1]['partial'] * 2), pair[0]))
    return dict(
        schema='port-dashboard/v1',
        provenance={'matrix': str(MATRIX), 'baseline': str(BASELINE),
                    'approved_scope': 'docs/REMOVED_SCOPE.json',
                    'evidence': str(EVIDENCE),
                    'original_source_checked': source is not None,
                    'behavior_parity_verified_by_dashboard': False},
        scope={**snapshot, 'new_exclusions_authorized': len(approved),
               'scope_check_passed': True,
               'legacy_verified_without_new_evidence': sum(
                   chain['evidence'] == 'legacy_baseline_needs_recheck'
                   for chain in chains)},
        evidence={'newly_verified_since_baseline': audit['new_verified'],
                  'recorded_entries': audit['evidence_records']},
        domain_summary=[dict(name=name, **stats) for name, stats in domain_order],
        chains=chains, work_queue=[chain['source_path'] for chain in queue],
        impact=impacted,
        disclaimer='映射/编译/静态依赖/文件存在均不证明功能等价。变更影响只指导复验，验收必须有生产入口、原版行为、异常取消恢复与真实测试证据。')


def write_dashboard(dashboard: dict, output: Path) -> dict:
    """Materialize both machine-readable inventory and a short human summary."""
    json_dump(output / 'PORT_DASHBOARD.json', dashboard)
    s = dashboard['scope']
    lines = [
        '# 222 Rust 迁移工作台（自动生成）', '',
        '> 事实源为逐文件矩阵、批准的移除清单和验收证据；此报告不宣称原版行为自动等价。', '',
        '## 当前范围与验收', '',
        '| 指标 | 数量 |', '|---|---:|',
        f'| 原版路径 | {s["total"]} |',
        f'| 用户批准排除 | {s["excluded"]} |',
        f'| 需要迁移 | {s["eligible"]} |',
        f'| 已登记验收 | {s["accepted"]} |',
        f'| 部分实现 | {s["partial"]} |',
        f'| 未迁移 | {s["unported"]} |',
        f'| 仍待验收 | {s["remaining"]} |',
        f'| 历史验收中缺少新版证据 | {s["legacy_verified_without_new_evidence"]} |',
        '', '## 业务域', '',
        '| 域 | 未迁移 | 部分实现 | 已验收 | 映射缺失 | Rust 文件缺失 |',
        '|---|---:|---:|---:|---:|---:|',
    ]
    for domain in dashboard['domain_summary']:
        lines.append(f'| {domain["name"]} | {domain["unported"]} | '
                     f'{domain["partial"]} | {domain["verified"]} | '
                     f'{domain["missing_mapping"]} | {domain["missing_rust"]} |')
    lines.extend(['', '## 待闭环优先项（人工审查后动手）', '',
                  '| 原版路径 | 状态 | Rust 映射 | 差异线索 |',
                  '|---|---|---|---|'])
    index = {chain['source_path']: chain for chain in dashboard['chains']}
    for path in dashboard['work_queue'][:30]:
        item = index[path]
        mapping = ', '.join(item['rust_targets']) or '待定位'
        gap = (item['current_gap'] or '').replace('|', '/').replace('\n', ' ')
        lines.append(f'| {path} | {item["status"]} | {mapping} | {gap[:100]} |')
    impact = dashboard['impact']
    if impact is not None:
        lines.extend(['', '## 本次改动影响', '',
                      f'- 分析深度：{impact["analysis"]}',
                      f'- 直接映射：{len(impact["direct_source_paths"])} 条',
                      f'- 需要复查：{len(impact["nonexcluded_needs_recheck"])} 条',
                      f'- 已验收但受影响：{len(impact["verified_needs_recheck"])} 条',
                      f'- 未匹配路径（需人工复核）：{", ".join(impact["unmatched"]) or "无"}'])
    lines.extend(['', '## 验收边界', '',
                  dashboard['disclaimer'], '',
                  '完整 1,906 条逐项功能链信息见 PORT_DASHBOARD.json。'])
    output.mkdir(parents=True, exist_ok=True)
    (output / 'PORT_DASHBOARD.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return dict(scope=dashboard['scope'], evidence=dashboard['evidence'],
                impact=dashboard['impact'],
                outputs=[str(output / 'PORT_DASHBOARD.json'),
                         str(output / 'PORT_DASHBOARD.md')])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('scan', 'check', 'freeze-baseline', 'impact', 'counts', 'stubs', 'reachability', 'dashboard'))
    parser.add_argument('--repo', type=Path, default=Path('.'))
    parser.add_argument('--source', type=Path, help='解压后的 2.1.88 原始源码根目录')
    parser.add_argument('--output', type=Path, default=Path('port-audit-output'))
    parser.add_argument('--allow-fixture', action='store_true', help='仅供单元测试使用')
    parser.add_argument('--changed', action='append', default=[], help='变更过的原路径或 Rust 路径，可重复指定')
    parser.add_argument('--changed-from-git', action='store_true',
                        help='dashboard：对本次提交 HEAD^..HEAD 的变更做复验影响分析')
    args = parser.parse_args(argv)
    try:
        rows = read_matrix(args.repo)
        counts = verify_counts(rows, strict=not args.allow_fixture)
        if args.command == 'counts':
            result = current_counts(rows, strict=not args.allow_fixture)
        elif args.command == 'stubs':
            inventory = detect_generated_stubs(rows, args.repo)
            json_dump(args.output / 'source-stub-inventory.json', inventory)
            result = {k: v for k, v in inventory.items() if k != 'items'}
            result['report'] = str(args.output / 'source-stub-inventory.json')
        elif args.command == 'reachability':
            result = generated_runtime_reachability(args.repo, rows)
            output_file = args.output / 'generated-reachability.json'
            json_dump(output_file, result)
            result = {k: v for k, v in result.items() if k != 'entries'}
            result['report'] = str(output_file)
        elif args.command == 'dashboard':
            changed = args.changed + (git_changed_files(args.repo)
                                      if args.changed_from_git else [])
            dashboard = build_dashboard(args.repo, rows, source=args.source,
                                        changed_files=sorted(set(changed)),
                                        strict=not args.allow_fixture)
            result = write_dashboard(dashboard, args.output)
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
