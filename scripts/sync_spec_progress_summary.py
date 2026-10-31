#!/usr/bin/env python3
"""Check or repair specification progress counts from the sole migration matrix."""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

from port_workbench import read_matrix, verify_counts

SUMMARY = Path('docs/SPEC_DOCUMENT_PROGRESS.md')


def expected_rows(repo: Path) -> dict[str, int]:
    rows = read_matrix(repo)
    counts = verify_counts(rows)
    return {
        '原版第一方及 vendor 源路径': len(rows),
        '已批准排除': counts['excluded'],
        '应迁移': len(rows) - counts['excluded'],
        '已验收': counts['verified'],
        '部分实现': counts['partial'],
        '未迁移': counts['unported'],
        '尚待完整验收': counts['partial'] + counts['unported'],
    }


def reconcile_summary(text: str, expected: dict[str, int]) -> tuple[str, dict[str, tuple[int, int]]]:
    begin = text.find('## 实际状态\n')
    end = text.find('\n## ', begin + 1)
    if begin < 0 or end < 0:
        raise ValueError('规格进度文档缺少完整的“实际状态”汇总区块')
    section = text[begin:end]
    differences = {}
    for label, desired in expected.items():
        pattern = re.compile(r'(?m)^(\|\s*' + re.escape(label) + r'\s*\|\s*)(\d+)(\s*\|[^\n]*$)')
        matches = list(pattern.finditer(section))
        if len(matches) != 1:
            raise ValueError(f'规格进度汇总行缺失或重复: {label}')
        found = int(matches[0].group(2))
        if found != desired:
            differences[label] = (found, desired)
        section = pattern.sub(lambda m: f'{m.group(1)}{desired}{m.group(3)}', section, count=1)
    return text[:begin] + section + text[end:], differences


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path('.'))
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args(argv)
    target = args.repo / SUMMARY
    try:
        original = target.read_text(encoding='utf-8')
        updated, differences = reconcile_summary(original, expected_rows(args.repo))
    except (OSError, ValueError) as error:
        print(f'规格进度同步失败: {error}', file=sys.stderr)
        return 2
    if differences:
        for name, (before, after) in differences.items():
            print(f'{name}: {before} -> {after}')
        if not args.write:
            print('请先运行 python3 scripts/sync_spec_progress_summary.py --write', file=sys.stderr)
            return 1
        target.write_text(updated, encoding='utf-8')
        print('已将汇总数字与唯一迁移台账同步')
    else:
        print('规格进度汇总与唯一迁移台账一致')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
