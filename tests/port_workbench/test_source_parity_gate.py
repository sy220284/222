import hashlib
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from source_parity_gate import AuditError, original_entries, verify_original, verify_contracts, verify_spec_archive, verify_source_lines

class SourceParityGateTests(unittest.TestCase):
    def test_archive_hash_full_inventory_and_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            z=Path(tmp)/"a.zip"
            data=b"export const a=1;\n"
            with ZipFile(z,"w") as f:
                f.writestr("claude-code-2.1.88-main/source/src/a.ts",data)
            rows={"src/a.ts":{"source_bytes":len(data),"source_lines":2}}
            baseline={"source_hashes":{"src/a.ts":hashlib.sha256(data).hexdigest()}}
            self.assertEqual(verify_original(z,rows,baseline),[])
            baseline["source_hashes"]["src/a.ts"]="0"*64
            self.assertTrue(verify_original(z,rows,baseline))
    def test_incomplete_contract_is_never_verified(self):
        rows={"src/a.ts":{"status":"verified","source_lines":2,"rust_mapping":"src/a.rs"}}
        baseline={"source_hashes":{"src/a.ts":"a"*64}}
        problems,stats=verify_contracts(rows,baseline,{})
        self.assertEqual(stats["missing"],1)
        self.assertTrue(problems)
        c={"src/a.ts":{"source_sha256":"a"*64,"review_status":"verified",
            "unresolved":[],"rust_entrypoints":["src/a.rs"],"functions":[{"id":"f","source_lines":[1,2]}],
            "full_source_review":{"lines_reviewed":2,"unreviewed_branches":0},
            "reviewer":"r","reviewed_at":"2026-10-31","tested_commit":"a"*40,"workflow_run_id":1}}
        problems,stats=verify_contracts(rows,baseline,c)
        self.assertEqual(stats["fully_evidenced"],0)
        self.assertTrue(any("行为维度" in p for p in problems))
    def test_report_exposes_remaining_gaps_without_bypassing_gate(self):
        from argparse import Namespace
        from unittest.mock import patch
        import json
        from source_parity_gate import run
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            (repo / "docs").mkdir()
            (repo / "docs/PORT_BASELINE.json").write_text(
                json.dumps({"expected_total": 1, "source_hashes": {"src/a.ts": "a" * 64}}),
                encoding="utf-8",
            )
            rows = {"src/a.ts": {"status": "unported", "source_lines": 2,
                                  "rust_mapping": "待定"}}
            progress = {"eligible": 1, "accepted": 0, "remaining": 1}
            with (patch("source_parity_gate.read_matrix", return_value=rows),
                  patch("source_parity_gate.current_counts", return_value=progress),
                  patch("source_parity_gate.check_evidence", return_value={}),
                  patch("source_parity_gate.load_contracts", return_value=({}, []))):
                output = repo / "audit"
                self.assertEqual(run(Namespace(repo=repo, output=output,
                                               command="report", source_archive=None, spec_archive=None)), 0)
                report = json.loads((output / "SOURCE_PARITY.json").read_text())
                self.assertFalse(report["pass"])
                self.assertEqual(report["contracts"]["missing"], 1)
                self.assertEqual(report["mode"], "report")
                self.assertEqual(run(Namespace(repo=repo, output=output,
                                               command="gate", source_archive=None, spec_archive=None)), 1)
                gated = json.loads((output / "SOURCE_PARITY.json").read_text())
                self.assertFalse(gated["pass"])
                self.assertIn("缺少原版源码 ZIP，拒绝放行", gated["errors"])

    def test_duplicate_archive_path_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            z=Path(tmp)/"a.zip"
            import warnings
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                with ZipFile(z,"w") as f:
                    f.writestr("source/src/a.ts","a")
                    f.writestr("source/src/a.ts","b")
            with self.assertRaises(AuditError):
                original_entries(z)


    def test_spec_manifest_source_mapping_and_duplicate_rejected(self):
        import io
        import csv
        with tempfile.TemporaryDirectory() as tmp:
            file = Path(tmp)/"spec.zip"
            code = b"const a=1\n"
            digest = hashlib.sha256(code).hexdigest()
            mapping = ("模块\t源文件\t源文件行数\t可执行单元\t静态操作节点\t唯一标注行数\t源文件sha256\t对应详细文档\n"
                       "M\ta.ts\t1\t1\t1\t1\t"+digest+"\t逐文件逐行/M/a.ts.md\n")
            docs = {
                "核验数据/文件与模块双向映射.tsv":mapping.encode(),
                "逐文件逐行/M/a.ts.md":"## 一、函数与类型声明入口\n| — | — | 无此类顶层声明 | — |\n## 二、物理源码逐行对照\n".encode("utf8"),
            }
            manifest = "".join(hashlib.sha256(data).hexdigest()+"  "+name+"\n"
                               for name,data in docs.items()).encode()
            with ZipFile(file,"w") as archive:
                for name,data in docs.items():
                    archive.writestr(name,data)
                archive.writestr("MANIFEST.sha256",manifest)
            baseline = {"source_hashes":{"src/a.ts":digest}}
            errors,summary=verify_spec_archive(file,baseline)
            self.assertEqual(errors,[])
            self.assertEqual(summary["mapped_firstparty"],1)
            self.assertEqual(summary["file_level_only"],1)
            self.assertEqual(summary["function_queue"][0]["kind"],"FileLevelDeclarationOrConstant")
            baseline["source_hashes"]["src/a.ts"]="0"*64
            errors,_=verify_spec_archive(file,baseline)
            self.assertTrue(any("不同版本源码" in e for e in errors))


    def test_all_source_lines_preserve_unicode_nbsp(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/"source.zip"
            spec = Path(tmp)/"spec.zip"
            text = "const x = '\u00a0';\n"
            with ZipFile(source, "w") as orig:
                orig.writestr("claude-code-2.1.88-main/source/src/a.ts", text.encode("utf8"))
            mapping = ("模块\t源文件\t源文件行数\t可执行单元\t静态操作节点\t唯一标注行数\t源文件sha256\t对应详细文档\n"
                       "M\ta.ts\t1\t1\t1\t1\t"+hashlib.sha256(text.encode()).hexdigest()+
                       "\t逐文件逐行/M/a.ts.md\n")
            doc = ("# a.ts\n## 三、物理源码逐行对照\n"
                   "| 行号 | 代码原文 | 作用域 | 语义 |\n"
                   "| 1 | <code>const&nbsp;x&nbsp;=&nbsp;'\u00a0';</code> | 文件级 | 常量 |\n")
            with ZipFile(spec,"w") as zipped:
                zipped.writestr("核验数据/文件与模块双向映射.tsv",mapping.encode())
                zipped.writestr("逐文件逐行/M/a.ts.md",doc.encode())
            errors,stat = verify_source_lines(source,spec)
            self.assertEqual(errors, [])
            self.assertEqual(stat["physical_lines"],1)
            bad = doc.replace("\u00a0", "&nbsp;")
            with ZipFile(spec,"w") as zipped:
                zipped.writestr("核验数据/文件与模块双向映射.tsv",mapping.encode())
                zipped.writestr("逐文件逐行/M/a.ts.md",bad.encode())
            errors,_ = verify_source_lines(source,spec)
            self.assertTrue(any("源码逐行原文不一致" in e for e in errors))


    def test_declaration_index_keeps_source_provenance_without_faking_acceptance(self):
        from source_parity_gate import source_declarations, index_contracts
        from unittest.mock import patch
        import json
        code = b"export function process(input: string) {\n return input;\n}\n"
        found = source_declarations(code)
        self.assertEqual([entry["id"] for entry in found], ["function:process@1"])
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            (repo / "docs").mkdir()
            (repo / "docs/PORT_BASELINE.json").write_text(json.dumps({
                "expected_total": 1,
                "source_hashes": {"src/a.ts": hashlib.sha256(code).hexdigest()}
            }), encoding="utf-8")
            (repo / "docs/SOURCE_PARITY_CONTRACTS.jsonl").write_text("", encoding="utf-8")
            archive = repo / "original.zip"
            with ZipFile(archive, "w") as original:
                original.writestr("original/source/src/a.ts", code)
            row = {"src/a.ts": {"status": "unported", "source_lines": 4,
                                  "source_bytes": len(code), "rust_mapping": "src/a.rs"}}
            with patch("source_parity_gate.read_matrix", return_value=row):
                stats = index_contracts(repo, archive, repo / "index.jsonl", write=True)
                self.assertEqual(stats["indexed"], 1)
                record = json.loads((repo / "docs/SOURCE_PARITY_CONTRACTS.jsonl").read_text())
                self.assertEqual(record["source_declarations"][0]["line"], 1)
                self.assertEqual(record["review_status"], "pending")
                self.assertEqual(record["functions"], [])
                # Re-indexing is idempotent: no new approvals or duplicates.
                index_contracts(repo, archive, repo / "index.jsonl", write=True)
                self.assertEqual(len((repo / "docs/SOURCE_PARITY_CONTRACTS.jsonl").read_text().splitlines()), 1)


    def test_working_directory_source_hash_is_verified(self):
        from source_parity_gate import pack_workspace_original, original_entries
        with tempfile.TemporaryDirectory() as directory:
            top = Path(directory)
            repo, source = top/"public-repo", top/"private-original"
            repo.mkdir()
            (source/"src").mkdir(parents=True)
            (source/"vendor").mkdir()
            code = b"export function example(){return 5;}\n"
            (source/"src/example.ts").write_bytes(code)
            (source/"vendor/adapter.ts").write_bytes(b"export const n=1;\n")
            archive = top/"original.zip"
            self.assertEqual(pack_workspace_original(source,repo,archive),2)
            original = original_entries(archive)
            self.assertEqual(original["src/example.ts"][0],hashlib.sha256(code).hexdigest())
            self.assertIn("vendor/adapter.ts",original)
            with self.assertRaises(AuditError):
                pack_workspace_original(repo,repo,top/"bad.zip")
            (source/"src/shortcut.ts").symlink_to(source/"src/example.ts")
            with self.assertRaises(AuditError):
                pack_workspace_original(source,repo,top/"bad.zip")

if __name__=="__main__":
    unittest.main()
