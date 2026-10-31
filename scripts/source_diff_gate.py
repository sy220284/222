#!/usr/bin/env python3
"""On-demand real original/Rust differential check, scoped to reviewed functions."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile

from port_workbench import AuditError, read_matrix
from runtime_parity_gate import _load_cases, run_dynamic_parity
from source_function_index import build_function_index
from source_parity_gate import (
    pack_workspace_original, verify_original, verify_source_lines,
    verify_spec_archive,
)
from source_test_gate import _jsonl, REGISTRY, verify_original as replay_goldens


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--repo",type=Path,default=Path("."))
    p.add_argument("--source-root",type=Path,required=True)
    p.add_argument("--spec-archive",type=Path,required=True)
    p.add_argument("--cases",type=Path,default=Path("tests/source_parity/cases"))
    p.add_argument("--output",type=Path,default=Path(".port-audit"))
    args=p.parse_args(argv)
    repo, root, output=args.repo.resolve(),args.source_root.resolve(),args.output.resolve()
    output.mkdir(parents=True,exist_ok=True)
    try:
        baseline=json.loads((repo/"docs/PORT_BASELINE.json").read_text(encoding="utf-8"))
        hashes=baseline["source_hashes"]
        matrix=read_matrix(repo)
        if set(matrix)!=set(hashes):
            raise AuditError("原版冻结清单与台账路径不一致")
        with tempfile.TemporaryDirectory(prefix="source-diff-") as tmp:
            archive=Path(tmp)/"source.zip"
            pack_workspace_original(root,repo,archive)
            errors=verify_original(archive,matrix,baseline)
            if errors:
                raise AuditError("原版工作区哈希不一致: " + errors[0])
            spec=args.spec_archive.resolve()
            if hashlib.sha256(spec.read_bytes()).hexdigest()!=(
                "3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93"
            ):
                raise AuditError("本地逐行规格 ZIP 哈希与冻结版本不一致")
            spec_errors,_=verify_spec_archive(spec,baseline)
            if spec_errors:
                raise AuditError("原版规格缺失或被改变: " + spec_errors[0])
            lines_errors,_=verify_source_lines(archive,spec)
            if lines_errors:
                raise AuditError("原版逐行规格与工作区源码不符: " + lines_errors[0])
            index,semantic_errors,_=build_function_index(spec,hashes)
            if semantic_errors:
                raise AuditError("原版 AST 索引不一致: " + semantic_errors[0])
            registrations=_jsonl(repo/REGISTRY)
            golden_errors=replay_goldens(repo,root,registrations,hashes)
            if golden_errors:
                raise AuditError("原版黄金结果与真实运行差异: " + golden_errors[0])
            cases,problems=_load_cases(args.cases)
            if problems:
                raise AuditError("差异场景无效: " + "; ".join(problems))
            registered_paths={r["path"] for r in registrations}
            by_id={n["source_id"]:path for path,nodes in index.items() for n in nodes}
            chosen={}
            for case in cases:
                for sid in case["source_ids"]:
                    path=by_id.get(sid)
                    if path is None or path not in registered_paths:
                        raise AuditError("场景引用了未核验原版函数: " + sid)
                    if matrix[path]["status"]=="excluded":
                        raise AuditError("差异测试不得包含排除路径: " + path)
                    registration=next(r for r in registrations if r["path"]==path)
                    if sid not in registration.get("source_ids",[]):
                        raise AuditError("测试未登记对应原版函数 ID: " + sid)
                    chosen[sid]=path
            selected={}
            required_families={}
            for sid,path in chosen.items():
                selected.setdefault(path,[]).append(
                    next(n for n in index[path] if n["source_id"]==sid)
                )
                registered=next(r for r in registrations if r["path"]==path)
                required_families[sid]=set(registered.get("differential_kinds",[]))
            contracts={path:{"functions":[{"source_id":node["source_id"]}
                            for node in nodes]} for path,nodes in selected.items()}
            result=run_dynamic_parity(
                repo,root,args.cases,selected,set(selected),output,
                contracts=contracts,required_families=required_families
            )
            after=Path(tmp)/"after.zip"
            pack_workspace_original(root,repo,after)
            after_errors=verify_original(after,matrix,baseline)
            if after_errors:
                result["errors"].extend("原版实际运行改变了工作区: "+e for e in after_errors)
                result["pass"]=False
            result["original_golden_replayed"]=len(registrations)
            result["source_hash_checked"]=len(hashes)
            result["differential_source_paths"]=len(selected)
            (output/"DYNAMIC_PARITY.json").write_text(
                json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
            print(json.dumps({k:v for k,v in result.items()
                if k not in ("missing_examples","mismatches","errors")},ensure_ascii=False))
            return 0 if result["pass"] else 1
    except (AuditError,OSError,ValueError,KeyError,TypeError) as exc:
        print("原版动态差异验收失败: "+str(exc),file=sys.stderr)
        (output/"DYNAMIC_PARITY.json").write_text(json.dumps({
            "schema":"source-rust-dynamic-parity/v1","pass":False,"error":str(exc)
        },ensure_ascii=False)+"\n",encoding="utf-8")
        return 1


if __name__=="__main__":
    sys.exit(main())
