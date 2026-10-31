"""Test the original-driven source/Rust registry cannot accept fake evidence."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]/"scripts"))
from source_test_gate import inspect, inventory_tests


class SourceTestGateTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo=Path(self.tmp.name)
        (self.repo/"src").mkdir()
        (self.repo/"tests").mkdir()
        (self.repo/"src/real.rs").write_text("pub fn real() {}",encoding="utf-8")
        self.sha="a"*64
        self.rows={"src/example.ts":{"status":"verified"}}
        self.hashes={"src/example.ts":self.sha}
        self.fixture={
            "schema":"source-golden/v1","source_path":"src/example.ts",
            "source_sha256":self.sha,"export":"parseExample",
            "cases":[{"id":"ordinary","input":"x","expected":"yes"},
                     {"id":"edge","input":"","expected":None}],
        }
        self.write_golden()
        self.entry={
            "path":"src/example.ts","source_sha256":self.sha,
            "profile":"pure","rust_entrypoints":["src/real.rs"],
            "golden":"tests/golden.json",
            "tests":[{"layer":"unit","name":"sample::tests::unit"},
                     {"layer":"regression","name":"golden_match"}],
        }
        self.old=[{"path":"src/example.ts","source_sha256":self.sha,
                   "rust_entrypoints":["src/real.rs"],
                   "test_names":["sample::tests::unit"]}]
        self.discovered={"sample::tests::unit","golden_match"}

    def write_golden(self):
        (self.repo/"tests/golden.json").write_text(
            json.dumps(self.fixture),encoding="utf-8")

    def result(self, full=False):
        return inspect(self.repo,self.rows,self.hashes,
                       [self.entry],self.old,self.discovered,full)

    def test_valid_evidence_has_real_golden_and_rust_tests(self):
        result=self.result()
        self.assertTrue(result["pass"],result["errors"])
        self.assertEqual(result["golden_cases"],2)

    def test_missing_discovered_rust_test_fails(self):
        self.discovered.remove("golden_match")
        self.assertTrue(any("实际 --list" in e for e in self.result()["errors"]))

    def test_source_sha_mismatch_fails(self):
        self.entry["source_sha256"]="b"*64
        self.assertFalse(self.result()["pass"])

    def test_golden_cannot_lose_edge_case(self):
        self.fixture["cases"]=self.fixture["cases"][:1]
        self.write_golden()
        self.assertFalse(self.result()["pass"])

    def test_rust_path_cannot_escape_repo(self):
        self.entry["rust_entrypoints"]=["../../unsafe.rs"]
        self.assertTrue(any("不得逃逸" in e for e in self.result()["errors"]))

    def test_full_gate_cannot_pass_unported_functions(self):
        self.rows["src/missing.ts"]={"status":"unported"}
        self.hashes["src/missing.ts"]="b"*64
        result=self.result(full=True)
        self.assertFalse(result["pass"])
        self.assertEqual(result["missing_source_tests"],1)
        self.assertEqual(result["remaining_port_paths"],1)

    def test_legacy_evidence_cannot_silently_disappear(self):
        self.old=[]
        self.assertTrue(any("缺少历史" in e for e in self.result()["errors"]))

    def test_cargo_inventory_parses_only_real_test_lines(self):
        file=self.repo/"list.txt"
        file.write_text("sample::tests::unit: test\ngolden_match: test\n2 tests\n")
        self.assertEqual(inventory_tests(file),self.discovered)
        file.write_text("some documentation says tests exist")
        with self.assertRaises(Exception):
            inventory_tests(file)


if __name__=="__main__":
    unittest.main()
