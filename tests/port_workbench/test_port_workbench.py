import contextlib
import io
import json
import tempfile
import unittest
import urllib.error
from unittest import mock
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from port_workbench import (AuditError, affected_source_paths, check_evidence, collect, label_status,
                            read_matrix, report, resolve_import, verify_counts, current_counts, detect_generated_stubs,
                            generated_runtime_reachability, verify_github_test_evidence, github_json, main,
                            build_dashboard, write_dashboard, git_changed_files,
                            indexed_rust_test_names)


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
        (self.repo / 'src/main.rs').write_text('mod lib;\n#[test]\nfn behavior() {}')
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

    def test_generated_only_exports_are_not_counted_as_runtime_connected(self):
        generated = self.repo / 'src/generated_ports'
        generated.mkdir()
        (generated / 'p_deadbeef01.rs').write_text('pub fn only_inspect() {}')
        (generated / 'p_cafebeef02.rs').write_text('pub fn actual() {}')
        (generated / 'registry_00.rs').write_text('p_deadbeef01::only_inspect();')
        (self.repo / 'src/agent.rs').write_text('fn call() { crate::generated_ports::p_cafebeef02::actual(); }')
        rows = read_matrix(self.repo)
        rows['src/lib.ts']['rust_mapping'] = '\u0060src/generated_ports/p_deadbeef01.rs\u0060'
        result = generated_runtime_reachability(self.repo, rows)
        self.assertEqual(result['generated_modules'], 2)
        self.assertEqual(result['inspect_only'], 1)
        self.assertEqual(result['outside_reference'], 1)
        self.assertEqual(result['entries'][1]['non_registry_references'], [])
        self.assertEqual(result['entries'][0]['non_registry_references'], ['src/agent.rs'])

    def test_reachability_cli_writes_complete_json_report(self):
        (self.repo / 'src/generated_ports').mkdir()
        (self.repo / 'src/generated_ports/p_deadbeef01.rs').write_text('pub fn sample() {}')
        output = Path(self.tmp.name) / 'reachability'
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(main(['reachability', '--repo', str(self.repo),
                                   '--allow-fixture', '--output', str(output)]), 0)
        report = json.loads((output/'generated-reachability.json').read_text())
        self.assertEqual(report['inspect_only'], 1)
        self.assertEqual(report['entries'][0]['classification'], 'inspect_only')

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


    def test_stub_inventory_flags_unreachable_generated_nulls_without_mislabeling_hand_port(self):
        port_dir = self.repo / 'src/generated_ports'
        port_dir.mkdir()
        stub = port_dir / ('p_'+'a'*13+'.rs')
        stub.write_text('//! Native Rust UI structure derived from original: src/cli.ts\n'
                        'pub fn render_source_view() { Value::Null }\n')
        (port_dir/'registry_00.rs').write_text('"src/cli.ts" => match export { _ => None },\n')
        self.matrix.write_text(self.matrix.read_text().replace('| 待定 | 未迁移 |',
            f'| `src/generated_ports/{stub.name}` | 部分实现：生成待核验 |'))
        inventory = detect_generated_stubs(read_matrix(self.repo), self.repo)
        self.assertEqual(inventory['count'], 1)
        self.assertEqual(inventory['with_null'], 1)
        self.assertEqual(inventory['unreachable_views'], 1)
        stub.write_text('//! Hand-port of src/cli.ts\npub fn render_source_view() { 42 }\n')
        clean = detect_generated_stubs(read_matrix(self.repo), self.repo)
        self.assertEqual(clean['count'], 0)

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

    def test_evidence_index_requires_actual_rust_test_attribute(self):
        rust = self.repo / 'src'
        rust.mkdir(exist_ok=True)
        (rust / 'sample.rs').write_text(
            'fn false_positive() {}\n'
            '#[test]\nfn real_test() {}\n'
            '#[tokio::test]\nasync fn real_async_test() {}\n',
            encoding='utf-8',
        )
        names = indexed_rust_test_names(self.repo)
        self.assertIn('real_test', names)
        self.assertIn('real_async_test', names)
        self.assertNotIn('false_positive', names)

    def test_historic_verified_entry_cannot_lose_its_evidence(self):
        self.write_matrix(status1='已验收')
        self.baseline()
        with self.assertRaisesRegex(AuditError, '可追溯证据'):
            check_evidence(self.repo, read_matrix(self.repo), False)

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

    def test_github_evidence_retries_transient_failure_and_caches_success(self):
        github_json.cache_clear()
        payload = json.dumps({'head_sha':'a'*40, 'conclusion':'success'}).encode()
        with mock.patch('port_workbench.urllib.request.urlopen') as open_url, \
             mock.patch('port_workbench.time.sleep') as sleep:
            open_url.side_effect = [
                urllib.error.HTTPError('https://api.github.com/test', 503,
                                      'Unavailable', {}, None),
                io.BytesIO(payload),
            ]
            value = github_json('https://api.github.com/test')
            self.assertEqual(value['conclusion'], 'success')
            self.assertEqual(github_json('https://api.github.com/test'), value)
            self.assertEqual(open_url.call_count, 2)
            sleep.assert_called_once()
        github_json.cache_clear()

    def test_github_evidence_recovers_after_multiple_502_errors(self):
        github_json.cache_clear()
        payload = json.dumps({'head_sha': 'a'*40, 'conclusion': 'success'}).encode()
        failures = [
            urllib.error.HTTPError('https://api.github.com/test', 502,
                                   'Bad Gateway', {}, None)
            for _ in range(4)
        ]
        with mock.patch('port_workbench.urllib.request.urlopen') as open_url, \
             mock.patch('port_workbench.time.sleep') as sleep:
            open_url.side_effect = [*failures, io.BytesIO(payload)]
            self.assertEqual(
                github_json('https://api.github.com/test')['conclusion'], 'success')
            self.assertEqual(open_url.call_count, 5)
            self.assertEqual(sleep.call_count, 4)
        github_json.cache_clear()

    def test_github_evidence_still_rejects_persistent_502(self):
        github_json.cache_clear()
        with mock.patch('port_workbench.urllib.request.urlopen') as open_url, \
             mock.patch('port_workbench.time.sleep') as sleep:
            open_url.side_effect = urllib.error.HTTPError(
                'https://api.github.com/test', 502, 'Bad Gateway', {}, None)
            with self.assertRaisesRegex(AuditError, 'HTTP 502'):
                github_json('https://api.github.com/test')
            self.assertEqual(open_url.call_count, 7)
            self.assertEqual(sleep.call_count, 6)
        github_json.cache_clear()

    def test_github_evidence_never_accepts_permanent_http_errors(self):
        github_json.cache_clear()
        with mock.patch('port_workbench.urllib.request.urlopen') as open_url:
            open_url.side_effect = urllib.error.HTTPError(
                'https://api.github.com/test', 404, 'Not found', {}, None)
            with self.assertRaisesRegex(AuditError, 'HTTP 404'):
                github_json('https://api.github.com/test')
            self.assertEqual(open_url.call_count, 1)
        github_json.cache_clear()

    def test_verified_evidence_requires_actual_successful_test_job_on_exact_head(self):
        sha = 'a'*40
        item = {'tested_commit': sha, 'workflow_run_id': 123}
        run = {'head_sha': sha, 'conclusion': 'success',
               'repository': {'full_name': 'sy220284/222'}}
        job = {'name':'编译与全部测试', 'conclusion':'success',
               'steps':[{'name':'Linux Rust 单元测试', 'conclusion':'success'}]}
        def lookup(url):
            return {'jobs':[job]} if url.endswith('/jobs?per_page=100') else run
        verify_github_test_evidence(item, repository='sy220284/222', fetch_json=lookup)
        job['conclusion'] = 'skipped'
        with self.assertRaisesRegex(AuditError, '没有实际执行成功'):
            verify_github_test_evidence(item, repository='sy220284/222', fetch_json=lookup)
        job['conclusion'] = 'success'
        run['head_sha'] = 'b'*40
        with self.assertRaisesRegex(AuditError, 'HEAD'):
            verify_github_test_evidence(item, repository='sy220284/222', fetch_json=lookup)
        with self.assertRaisesRegex(AuditError, 'workflow_run_id'):
            verify_github_test_evidence({'tested_commit':sha}, repository='sy220284/222', fetch_json=lookup)

    def test_promotion_valid_record_can_be_checked(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        item=dict(path='src/cli.ts', source_sha256=__import__('hashlib').sha256((self.src/'src/cli.ts').read_bytes()).hexdigest(),
                  rust_entrypoints=['src/main.rs'],test_names=['tests::behavior'],
                  tested_commit='b'*40, failure_and_recovery_verified=True, remaining_gaps=[])
        (self.repo/'docs/PORT_EVIDENCE.jsonl').write_text(json.dumps(item)+'\n')
        self.assertEqual(check_evidence(self.repo,read_matrix(self.repo),False)['new_verified'],1)

    def test_integration_test_in_tests_directory_is_real_acceptance_evidence(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        integration = self.repo / 'tests'
        integration.mkdir()
        (integration / 'real_cli.rs').write_text(
            '#[test] fn real_executable_roundtrip() { assert!(true); }'
        )
        item=dict(
            path='src/cli.ts',
            source_sha256=__import__('hashlib').sha256(
                (self.src / 'src/cli.ts').read_bytes()
            ).hexdigest(),
            rust_entrypoints=['src/main.rs'],
            test_names=['real_cli::real_executable_roundtrip'],
            tested_commit='b' * 40,
            failure_and_recovery_verified=True,
            remaining_gaps=[],
        )
        evidence = self.repo / 'docs/PORT_EVIDENCE.jsonl'
        evidence.write_text(json.dumps(item) + '\n')
        result = check_evidence(self.repo, read_matrix(self.repo), False)
        self.assertEqual(result['new_verified'], 1)
        self.assertEqual(result['evidence_records'], 1)

    def test_fictitious_integration_test_cannot_be_accepted(self):
        self.baseline()
        self.write_matrix(status1='已验收')
        integration = self.repo / 'tests'
        integration.mkdir()
        (integration / 'real_cli.rs').write_text(
            '#[test] fn real_executable_roundtrip() { assert!(true); }'
        )
        item=dict(
            path='src/cli.ts',
            source_sha256=__import__('hashlib').sha256(
                (self.src / 'src/cli.ts').read_bytes()
            ).hexdigest(),
            rust_entrypoints=['src/main.rs'],
            test_names=['real_cli::invented_test_name'],
            tested_commit='b' * 40,
            failure_and_recovery_verified=True,
            remaining_gaps=[],
        )
        (self.repo / 'docs/PORT_EVIDENCE.jsonl').write_text(
            json.dumps(item) + '\n'
        )
        with self.assertRaisesRegex(AuditError, 'Rust 测试函数不存在'):
            check_evidence(self.repo, read_matrix(self.repo), False)

    def test_exclusion_cannot_be_added_without_approval(self):
        self.baseline()
        self.write_matrix(status1='按用户要求删除')
        with self.assertRaisesRegex(AuditError,'新增排除项'):
            check_evidence(self.repo,read_matrix(self.repo),False)

    def test_user_approved_exclusions_are_scope_locked(self):
        self.baseline()
        self.write_matrix(status1='按用户要求删除')
        scope={'schema':'user-approved-removal-scope/v1',
               'items':[{'path':'src/cli.ts','reason':'用户取消云端范围'}]}
        (self.repo/'docs/REMOVED_SCOPE.json').write_text(json.dumps(scope))
        self.assertEqual(check_evidence(self.repo,read_matrix(self.repo),False)['excluded'],1)
        scope['items'].append({'path':'src/lib.ts','reason':'未经执行'})
        (self.repo/'docs/REMOVED_SCOPE.json').write_text(json.dumps(scope))
        with self.assertRaisesRegex(AuditError,'授权未应用'):
            check_evidence(self.repo,read_matrix(self.repo),False)

    def test_relative_dependency_climb_and_filename_resolution(self):
        self.assertEqual(resolve_import('src/components/x/a.ts','../../lib.js',{'src/lib.ts'}),'src/lib.ts')
        self.assertIsNone(resolve_import('src/a.ts','../../../evil',{'evil.ts'}))
        self.assertEqual(resolve_import('src/main.ts','./dir',{'src/dir/index.tsx'}),'src/dir/index.tsx')


    def test_dashboard_uses_matrix_scope_and_marks_unverified_behavior(self):
        self.baseline()
        report = build_dashboard(self.repo, read_matrix(self.repo), strict=False)
        self.assertEqual(report['scope']['total'], 2)
        self.assertEqual(report['scope']['eligible'], 2)
        self.assertEqual(report['scope']['remaining'], 2)
        self.assertEqual(report['scope']['accepted'], 0)
        self.assertEqual(len(report['chains']), 2)
        self.assertEqual(report['work_queue'][0], 'src/cli.ts')
        by_path = {item['source_path']: item for item in report['chains']}
        self.assertTrue(by_path['src/cli.ts']['mapping_missing'])
        self.assertEqual(by_path['src/lib.ts']['rust_present'], ['src/main.rs'])
        self.assertEqual(by_path['src/lib.ts']['evidence'], 'not_yet_verified')
        self.assertEqual(by_path['src/lib.ts']['runtime_behavior'],
                         'requires_real_entrypoint_test')
        self.assertFalse(report['provenance']['behavior_parity_verified_by_dashboard'])

    def test_dashboard_with_source_checks_hash_and_transitive_impact(self):
        self.baseline()
        report = build_dashboard(self.repo, read_matrix(self.repo), source=self.src,
                                 changed_files=['src/main.rs', 'docs/readme.md'],
                                 strict=False)
        impact = report['impact']
        self.assertEqual(impact['analysis'], 'source_import_closure')
        self.assertEqual(impact['direct_source_paths'], ['src/lib.ts'])
        self.assertEqual(impact['impacted_source_paths'], ['src/cli.ts', 'src/lib.ts'])
        self.assertEqual(impact['nonexcluded_needs_recheck'], ['src/cli.ts', 'src/lib.ts'])
        self.assertEqual(impact['unmatched'], ['docs/readme.md'])
        self.assertTrue(impact['unmatched_require_manual_review'])
        self.assertEqual(report['chains'][0]['source_dependencies'], ['src/lib.ts'])
        (self.src / 'src/lib.ts').write_text(self.source2.replace('return 1', 'return 2'))
        with self.assertRaisesRegex(AuditError, '哈希与冻结基线不一致'):
            build_dashboard(self.repo, read_matrix(self.repo), source=self.src,
                            strict=False)

    def test_dashboard_direct_only_impact_does_not_claim_dependency_closure(self):
        self.baseline()
        result = build_dashboard(self.repo, read_matrix(self.repo),
                                 changed_files=['src/main.rs'], strict=False)
        self.assertEqual(result['impact']['analysis'],
                         'direct_mapping_only_no_source_dependency_graph')
        self.assertEqual(result['impact']['impacted_source_paths'], ['src/lib.ts'])
        self.assertNotIn('src/cli.ts', result['impact']['impacted_source_paths'])

    def test_dashboard_requires_approved_exclusions_and_real_rust_targets(self):
        self.baseline()
        (self.repo / 'src/main.rs').unlink()
        result = build_dashboard(self.repo, read_matrix(self.repo), strict=False)
        self.assertEqual(result['chains'][1]['rust_missing'], ['src/main.rs'])
        self.assertEqual(sum(item['missing_rust'] for item in result['domain_summary']), 1)
        self.write_matrix(status1='按用户要求删除')
        with self.assertRaisesRegex(AuditError, '新增排除项'):
            build_dashboard(self.repo, read_matrix(self.repo), strict=False)

    def test_dashboard_cli_emits_json_and_markdown_without_fabricating_tests(self):
        self.baseline()
        output = Path(self.tmp.name) / 'dashboard'
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            self.assertEqual(main(['dashboard', '--repo', str(self.repo),
                                   '--output', str(output), '--allow-fixture']), 0)
        snapshot = json.loads((output / 'PORT_DASHBOARD.json').read_text())
        markdown = (output / 'PORT_DASHBOARD.md').read_text()
        self.assertEqual(snapshot['scope']['remaining'], 2)
        self.assertIn('待闭环优先项', markdown)
        self.assertIn('src/lib.ts', markdown)
        self.assertEqual(snapshot['chains'][1]['test_names'], [])
        self.assertEqual(json.loads(stdout.getvalue())['scope']['eligible'], 2)

    def test_changed_from_git_does_not_silently_ignore_diff_failures(self):
        with mock.patch('port_workbench.subprocess.run') as run:
            run.return_value = mock.Mock(returncode=0,
                                         stdout='src/main.rs\nsrc/main.rs\n',
                                         stderr='')
            self.assertEqual(git_changed_files(self.repo), ['src/main.rs'])
            run.return_value = mock.Mock(returncode=128, stdout='',
                                         stderr='unknown revision HEAD^')
            with self.assertRaisesRegex(AuditError, 'HEAD'):
                git_changed_files(self.repo)



if __name__ == '__main__':
    unittest.main()
