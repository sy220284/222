"""Regression checks for the compulsory original-vs-Rust runtime parity gate."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/"scripts"))
from runtime_parity_gate import run_dynamic_parity

PRODUCER = """
import json, os, sys
r = json.loads(sys.stdin.read())
value = r["input"]["value"]
if os.environ.get("PARITY_SIDE") == "rust" and r["input"].get("diverge"):
    value = value + 1
out = {
"schema":"source-rust-observation/v1",
"case_id":r["id"], "executed_source_ids":["src/f.ts:FunctionDeclaration:1-1:occurrence-1"],
"outcome":{"value":value}, "errors":[], "state":{"value":value},
"calls":["execute"], "permissions":[], "events":["begin","end"],
"side_effects":[], "persistence":{}, "cancellation":{"cancelled":False}
}
print(json.dumps(out))
"""


class DynamicParityTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.original = self.root/"original"
        self.repo = self.root/"repo"
        self.cases = self.root/"cases"
        for d in (self.original,self.repo,self.cases):
            d.mkdir()
        self.sid = "src/f.ts:FunctionDeclaration:1-1:occurrence-1"
        self.index = {"src/f.ts":[{"source_id":self.sid}]}
        (self.original/"produce.py").write_text(PRODUCER,encoding="utf8")
        (self.repo/"produce.py").write_text(PRODUCER,encoding="utf8")

    def case(self, name, kind, diverge=False):
        data = {
            "id":name,"kind":kind,"source_ids":[self.sid],
            "input":{"value":10,"diverge":diverge},"timeout_seconds":5,
            "original":[sys.executable,"{ORIGINAL}/produce.py"],
            "rust":[sys.executable,"{REPO}/produce.py"],
        }
        (self.cases/(name+".json")).write_text(
            json.dumps(data),encoding="utf8")

    def run_gate(self):
        return run_dynamic_parity(self.repo,self.original,self.cases,
                                  self.index,{"src/f.ts"},self.root/"out")

    def test_missing_scenarios_fail_closed(self):
        report = self.run_gate()
        self.assertFalse(report["pass"])
        self.assertEqual(report["missing_units"],1)
        self.assertTrue((self.root/"out/DYNAMIC_PARITY.json").is_file())

    def test_both_executables_match_and_cover_normal_and_edge(self):
        self.case("ok","normal")
        self.case("boundary","edge")
        report = self.run_gate()
        self.assertEqual(report["passed"],2)
        self.assertTrue(report["pass"])

    def test_real_differences_block(self):
        self.case("ok","normal")
        self.case("diff","edge",diverge=True)
        report = self.run_gate()
        self.assertFalse(report["pass"])
        self.assertEqual(len(report["mismatches"]),1)
        self.assertIn("outcome",report["mismatches"][0]["different_fields"])

    def test_unknown_function_cannot_claim_coverage(self):
        self.case("ok","normal")
        file = self.cases/"ok.json"
        record=json.loads(file.read_text())
        record["source_ids"]=["src/made-up.ts:fake:1"]
        file.write_text(json.dumps(record),encoding="utf8")
        report=self.run_gate()
        self.assertFalse(report["pass"])
        self.assertTrue(any("不存在" in e for e in report["errors"]))


if __name__ == "__main__":
    unittest.main()
