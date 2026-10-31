import hashlib
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from source_parity_gate import AuditError, original_entries, verify_original, verify_contracts

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
                                               command="report", source_archive=None)), 0)
                report = json.loads((output / "SOURCE_PARITY.json").read_text())
                self.assertFalse(report["pass"])
                self.assertEqual(report["contracts"]["missing"], 1)
                self.assertEqual(report["mode"], "report")
                self.assertEqual(run(Namespace(repo=repo, output=output,
                                               command="gate", source_archive=None)), 1)
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

if __name__=="__main__":
    unittest.main()
