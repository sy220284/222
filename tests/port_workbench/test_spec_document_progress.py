"""Checks for archived specification -> migration status crosswalk."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import spec_document_progress as audit


class SpecDocumentProgressTests(unittest.TestCase):
    def test_full_inventory_and_exclusion_with_exact_source_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            docs = repo / 'docs'
            docs.mkdir()
            source_hash = hashlib.sha256(b'a=1\n').hexdigest()
            (docs / 'FILE_PORT_MATRIX.md').write_text(
                '| Source | Lines | Bytes | Action | Rust | Status |\n'
                '|---|---:|---:|---|---|---|\n'
                '| `src/main.ts` | 1 | 4 | 重写 | `src/main.rs` | 部分实现：待验收 |\n'
                '| `src/bridge.ts` | 1 | 4 | 厂商专属桥接 | 无 | 按用户要求删除：远程桥接 |\n',
                encoding='utf8')
            (docs / 'PORT_BASELINE.json').write_text(json.dumps({
                'expected_total': 2, 'source_hashes': {
                    'src/main.ts': source_hash, 'src/bridge.ts': source_hash,
                }
            }))
            (docs / 'REMOVED_SCOPE.json').write_text(json.dumps({
                'items': [{'path': 'src/bridge.ts', 'reason': '原厂专用桥接'}]
            }))
            mapping = (
                '模块\t源文件\t源文件行数\t可执行单元\t静态操作节点\t唯一标注行数\t源文件sha256\t对应详细文档\n'
                + 'M\tmain.ts\t1\t1\t1\t1\t' + source_hash + '\t逐文件逐行/M/main.ts.md\n'
                + 'M\tbridge.ts\t1\t1\t1\t1\t' + source_hash + '\t逐文件逐行/M/bridge.ts.md\n'
            )
            archive = repo / 'spec.zip'
            with ZipFile(archive, 'w') as zip_file:
                zip_file.writestr(audit.MAPPING_TSV, mapping)
                zip_file.writestr('逐文件逐行/M/main.ts.md', 'main')
                zip_file.writestr('逐文件逐行/M/bridge.ts.md', 'bridge')
                zip_file.writestr('README.md', 'test')
            entries, exclusions, summary = audit.build(archive, repo)
            self.assertEqual(len(entries), 4)
            self.assertEqual(summary['original_source_documents'], 2)
            self.assertEqual(len(exclusions), 1)
            self.assertEqual(exclusions[0]['排除原因'], '原厂专用桥接')
            self.assertEqual(
                next(x for x in entries if x['原版源码'] == 'src/main.ts')['Rust实现入口'],
                '`src/main.rs`')
            self.assertEqual(
                sum(item['当前迁移状态'] == '已授权排除' for item in entries), 1)


if __name__ == '__main__':
    unittest.main()
