import csv, hashlib, io, sys, tempfile, unittest
from pathlib import Path
from zipfile import ZipFile
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/"scripts"))
from source_function_index import build_function_index, validate_contract_coverage

class SourceIndexTests(unittest.TestCase):
    def test_same_line_callbacks_are_unique_and_required(self):
        with tempfile.TemporaryDirectory() as d:
            archive=Path(d)/"spec.zip"; sha=hashlib.sha256(b"two callbacks").hexdigest()
            mapping=("模块\t源文件\t源文件行数\t可执行单元\t静态操作节点\t唯一标注行数\t源文件sha256\t对应详细文档\n"
                     f"M\ta.ts\t1\t2\t2\t1\t{sha}\t逐文件逐行/M/a.ts.md\n")
            fields=["模块","文件","函数或方法","AST类别","开始行","结束行",
                    "父级作用域","参数","返回类型声明","是否async","是否generator",
                    "步骤数","分支节点数","显式返回数","抛错数","捕获数","await数",
                    "状态写入数","原始源文件sha256"]
            table=io.StringIO(); writer=csv.DictWriter(table,fieldnames=fields,delimiter="\t");writer.writeheader()
            for param in ("a","b"):
                row={k:"" for k in fields}
                row.update({"文件":"a.ts","AST类别":"ArrowFunction","函数或方法":"回调@1",
                            "开始行":"1","结束行":"1","父级作用域":"文件级",
                            "参数":param,"是否async":"False","是否generator":"False",
                            "原始源文件sha256":sha})
                for key in fields[11:18]:row[key]="0"
                writer.writerow(row)
            with ZipFile(archive,"w") as z:
                z.writestr("核验数据/全函数与方法核查.tsv",table.getvalue())
                z.writestr("核验数据/文件与模块双向映射.tsv",mapping)
            index,errors,stats=build_function_index(archive,{"src/a.ts":sha})
            self.assertEqual(errors,[]);self.assertEqual(stats["ast_functions"],2)
            self.assertEqual(stats["shared_position_groups"],1)
            self.assertNotEqual(index["src/a.ts"][0]["source_id"],index["src/a.ts"][1]["source_id"])
            self.assertEqual(validate_contract_coverage(index,{},{"src/a.ts"})[0]["missing"],2)
            records={"src/a.ts":{"functions":[{"source_id":a["source_id"]} for a in index["src/a.ts"]]}}
            self.assertEqual(validate_contract_coverage(index,records,{"src/a.ts"}),[])

if __name__ == "__main__":unittest.main()
