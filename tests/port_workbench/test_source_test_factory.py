"""Keep full-source worklist and real test acceptance separate."""
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2]/"scripts"))
from source_test_factory import build_worklist

class SourceTestFactoryTest(unittest.TestCase):
    def setUp(self):
        self.rows = {
            "src/a.ts": {"status":"verified","rust_mapping":"src/a.rs"},
            "src/b.ts": {"status":"unported","rust_mapping":""},
            "src/c.ts": {"status":"excluded","rust_mapping":""},
        }
        self.hashes = {p:p for p in self.rows}
        self.registration = [{"path":"src/a.ts","source_sha256":"src/a.ts",
            "source_ids":["src/a.ts:FunctionDeclaration:1-2:occurrence-1"],
            "tests":[{"name":"a::tests::test"}],"golden":"tests/a.json"}]

    def test_every_non_excluded_path_is_a_real_task(self):
        tasks,stats=build_worklist(self.rows,self.hashes,self.registration)
        self.assertEqual(stats["required_paths"],2)
        self.assertEqual(stats["missing_golden_paths"],1)
        self.assertFalse(stats["full_ast_verified"])
        self.assertFalse(stats["all_function_tests_built"])
        self.assertEqual({task["path"] for task in tasks},{"src/a.ts","src/b.ts"})
        self.assertTrue(all(not task["accepted_by_factory"] for task in tasks))

    def test_complete_ast_index_identifies_exact_missing_functions(self):
        index={
            "src/a.ts":[{"source_id":"src/a.ts:FunctionDeclaration:1-2:occurrence-1",
                        "kind":"FunctionDeclaration","source_lines":[1,2]},
                        {"source_id":"src/a.ts:ArrowFunction:3-3:occurrence-1",
                        "kind":"ArrowFunction","source_lines":[3,3]}],
            "src/b.ts":[{"source_id":"src/b.ts:FileLevel:1",
                        "kind":"FileLevel","source_lines":[1,4]}],
        }
        tasks,stats=build_worklist(self.rows,self.hashes,self.registration,index)
        self.assertTrue(stats["full_ast_verified"])
        self.assertEqual(stats["indexed_ast_units"],3)
        self.assertEqual(stats["ast_units_without_golden"],2)
        a=next(t for t in tasks if t["path"]=="src/a.ts")
        self.assertEqual(a["units_without_golden"],1)

    def test_fake_source_id_is_rejected(self):
        self.registration[0]["source_ids"]=["src/a.ts:fake"]
        with self.assertRaises(Exception):
            build_worklist(self.rows,self.hashes,self.registration,{"src/a.ts":[
                {"source_id":"src/a.ts:real"}],"src/b.ts":[{"source_id":"src/b.ts:file"}]})

    def test_duplicate_and_unknown_paths_are_not_accepted(self):
        with self.assertRaises(Exception):
            build_worklist(self.rows,self.hashes,self.registration*2)
        with self.assertRaises(Exception):
            build_worklist(self.rows,self.hashes,[{"path":"src/c.ts",
                "source_sha256":"src/c.ts"}])

if __name__=="__main__":
    unittest.main()
