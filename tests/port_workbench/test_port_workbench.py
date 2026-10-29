import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from port_workbench import (AuditError, affected_source_paths, check_evidence, collect, label_status,
                            read_matrix, report, resolve_import, verify_counts, current_counts, main)


class WorkbenchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.repo = root / 'rust'; self.repo.mkdir()
        self.src = root / 'source'; self.src.mkdir()
        (self.repo / 'docs').mkdir()
        (self.repo / 'src').mkdir()
        (self.src / 'src').mkdir()
        source1 = "import {helper} from './lib.js'\nexport function run() { return helper() }\n"
        source2 = 'export function helper(){return 1}'
        (self.src / 'src/cli.ts').write_text(source1)
        (self.src / 'src/lib.ts').write_text(source2)
        (self.repo / 'src/main.rs').write_text('mod lib; #[test] fn behavior() {}')
        self.source1 = source1; self.source2=source2
        self.matrix = self.repo / 'docs/FILE_PORT_MATRIX.md'
        self.write_matrix()

    def write_matrix(self, status1='未迁移', status2='部分实现'):
        source_rows=[]
        for name,content,status,target in [('cli.ts', self.source1, status1, '待定'),
                                           ('lib.ts', self.source2, status2, '`src/main.rs`')]:
            source_rows.append(f'| `src/{name}` | {content.count(chr(10))+1} | '
                               f'{len(content.encode())} | 重写 | {target} | {status} |')
        self.matrix.write_text('| 原路径 | 原行数 | 原字节数 | 处理方式 | Rust 目标 | 当前验证状态 |\n'
                               + '\n'.join(source_rows) + '\n')

    def baseline(self):
        rows = read_matrix(self.repo)
        (self.repo / 'docs/PORT_BASELINE.json').write_text(json.dumps({
            'statuses':{p: r['status'] for p,r in rows.items()},
            'source_hashes':{p:__import__('hashlib').sha256((self.src/p).read_bytes()).hexdigest() for p in rows}}))

    def test_counts_command_tracks_matrix_changes_without_manual_copying(self):
        initial = current_counts(read_matrix(self.repo), strict=False)
        self.assertEqual((initial['partial'], initial['unported'], initial['remaining']),
                         (1, 1, 2))
        self.write_matrix(status1='部分实现', status2='部分实现')
        moved = current_counts(read_matrix(self.repo), strict=False)
        self.assertEqual((moved['partial'], moved['unported'], moved['remaining']),
                         (2, 0, 2))
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(main(['counts', '--repo', str(self.repo), '--allow-fixture']), 0)
        printed = json.loads(output.getvalue())
        self.assertEqual(printed, moved)

    def test_static_inventory_and_grouping(self):
        rows = read_matrix(self.repo)
        self.assertEqual(verify_counts(rows, strict=False)['unported'], 1)
        items,groups=collect(rows, self.repo, self.src)
        cli=next(x for x in items if x['path']=='src/cli.ts')
        lib=next(x for x in items if x['path']=='src/lib.ts')
        self.assertEqual(cli['dependencies'], ['src/lib.ts'])
        self.assertEqual(lib['direct_consumers'], 1)
        self.assertEqual(lib['mapped_rust_present'], ['src/main.rs'])
        self.assertEqual(cli['mapped_rust_missing'], [])
        self.assertTrue(cli['sha256'])
        result=report(rows,self.repo,self.src,Path(self.tmp.name)/'out',strict=False)
        self.assertEqual(result['counts'], {'partial':1,'unported':1})
        self.assertEqual(len(json.loads((Path(self.tmp.name)/'out/source-port-inventory.json').read_text())['entries']),2)

    def test_change_impact_captures_callers_and_rust_owners(self):
        entries,_=collect(read_matrix(self.repo),self.repo,self.src)
        result=affected_source_paths(entries,['src/main.rs'])
        self.assertEqual(result['direct_source_paths'], ['src/lib.ts'])
        self.assertEqual(result['impacted_source_paths'], ['src/cli.ts','src/lib.ts'])
        self.assertFalse(result['unmatched'])

    def test_duplicate_source_path_is_error(self):
        self.matrix.write_text(self.matrix.read_text()+self.matrix.read_text().splitlines()[1]+'\n')
        with self.assertRaisesRegex(AuditError,'重复原路径'):
            read_matrix(self.repo)

    def test_unknown_migration_status_is_error(self):
        self.write_matrix(status1='已差不多')
        with self.assertRaisesRegex(AuditError,'未知迁移状态'):
            read_matrix(self.repo)

    def test_missing_original_source_fails_loudly(self):
        (self.src/'src/lib.ts').unlink()
        with self.assertRaisesRegex(AuditError,'原版源码缺失'):
            collect(read_matrix(self.repo),self.repo,self.src)

    def test_source_metadata_mismatch_fails(self):
        (self.src/'src/lib.ts').write_text('export function helper(){return false}')
        with self.assertRaisesRegex(AuditError,'原版源码与台账不一致'):
            collect(read_matrix(self.repo),self.repo,self.src)

    def test_unchanged_baseline_is_valid(self):
        self.baseline()
        self.assertEqual(check_evidence(self.repo,read_matrix(self.repo),False)['new_verified'],0)

    def test_promotion_without_evidence_is_rejected(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        with self.assertRaisesRegex(AuditError,'可追溯证据'):
            check_evidence(self.repo,read_matrix(self.repo),False)

    def test_promotion_with_remaining_gaps_is_rejected(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        item=dict(path='src/cli.ts', source_sha256=__import__('hashlib').sha256((self.src/'src/cli.ts').read_bytes()).hexdigest(),
                  rust_entrypoints=['src/main.rs'],test_names=['smoke'],tested_commit='b'*40,
                  failure_and_recovery_verified=True, remaining_gaps=['no real CLI'])
        (self.repo/'docs/PORT_EVIDENCE.jsonl').write_text(json.dumps(item)+'\n')
        with self.assertRaisesRegex(AuditError,'存在未解决差异'):
            check_evidence(self.repo,read_matrix(self.repo),False)

    def test_promotion_valid_record_can_be_checked(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        item=dict(path='src/cli.ts', source_sha256=__import__('hashlib').sha256((self.src/'src/cli.ts').read_bytes()).hexdigest(),
                  rust_entrypoints=['src/main.rs'],test_names=['tests::behavior'],
                  tested_commit='b'*40, failure_and_recovery_verified=True, remaining_gaps=[])
        (self.repo/'docs/PORT_EVIDENCE.jsonl').write_text(json.dumps(item)+'\n')
        self.assertEqual(check_evidence(self.repo,read_matrix(self.repo),False)['new_verified'],1)

    def test_exclusion_cannot_be_added_without_approval(self):
        self.baseline()
        self.write_matrix(status1='按用户要求删除')
        with self.assertRaisesRegex(AuditError,'新增排除项'):
            check_evidence(self.repo,read_matrix(self.repo),False)

    def test_relative_dependency_climb_and_filename_resolution(self):
        self.assertEqual(resolve_import('src/components/x/a.ts','../../lib.js',{'src/lib.ts'}),'src/lib.ts')
        self.assertIsNone(resolve_import('src/a.ts','../../../evil',{'evil.ts'}))
        self.assertEqual(resolve_import('src/main.ts','./dir',{'src/dir/index.tsx'}),'src/dir/index.tsx')


if __name__ == '__main__':
    unittest.main()
