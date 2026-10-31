#!/usr/bin/env python3
"""Reproduce a 2.1.88 prompt-literal catalog from the user-supplied source ZIP.

This catalog stores literal source evidence. Dynamically interpolated JS templates
remain raw; runtime behavior must be connected and tested separately.
"""
import argparse
import csv
import hashlib
import json
import re
import zipfile
from pathlib import Path

OMIT = {
    'source/src/skills/bundled/scheduleRemoteAgents.ts',
    'source/src/utils/teleport.tsx',
}


def contains_prompts(row):
    path = row['code_path'].lower()
    return (
        row['category'] in {'工具提示词', '内置子 Agent', '内置技能提示词'}
        or any(piece in path for piece in (
            'prompt', 'summary', 'review', 'magicdocs', 'memorytypes',
            'autodream', 'extractmemories',
        ))
    )


def extract_literals(code):
    # This extracts verbatim source slices, not evaluated JavaScript.
    # A lexical candidate may span template interpolation or a comment:
    # no extracted entry is considered runnable without an independent test.
    pattern = re.compile(r'''(?s)(?<!\\)([`"'])(.*?)(?<!\\)\1''')
    results = []
    for match in pattern.finditer(code):
        value = match.group(2)
        if len(value) < 150:
            continue
        results.append({
            'line': code.count('\n', 0, match.start()) + 1,
            'quote': match.group(1),
            'raw': value,
            'dynamic': '${' in value,
        })
    return results


def build(archive, index):
    with zipfile.ZipFile(archive) as zipped:
        with open(index, encoding='utf-8-sig', newline='') as handle:
            rows = list(csv.DictReader(handle))
        if len(rows) != 120 or len({item['code_path'] for item in rows}) != 120:
            raise ValueError('Expected 120 distinct original prompt-source entries')
        output = []
        for row in rows:
            path = row['code_path']
            data = zipped.read('claude-code-2.1.88-main/' + path)
            source = data.decode('utf-8')
            is_excluded = path in OMIT
            included = contains_prompts(row) and not is_excluded
            output.append({
                'path': path.removeprefix('source/'),
                'category': row['category'],
                'source_sha256': hashlib.sha256(data).hexdigest(),
                'source_bytes': len(data),
                'excluded': is_excluded,
                'literal_candidates': extract_literals(source) if included else [],
            })
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--index', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    rows = build(args.archive, args.index)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('w', encoding='utf-8', newline='\n') as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False, separators=(',', ':')) + '\n')
    fragments = sum(len(row['literal_candidates']) for row in rows)
    print(f'Wrote {len(rows)} sources, {fragments} literal candidates, {args.output.stat().st_size} bytes')
    print('Source exclusion:', ', '.join(sorted(x['path'] for x in rows if x['excluded'])))


if __name__ == '__main__':
    main()
