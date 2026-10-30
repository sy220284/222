> **2026-10-30 最新授权范围（覆盖下文历史快照）**：Claude Code 2.1.88 原始 1,906 条路径，其中 22 条原始排除，加上 177 条用户确认的远程、云端及原厂专属排除，共 **199 条排除**，其余 **1,707 条保留**；模型通过可配置 API 接入，继续实现独立本地 Agent、普通 SDK、MCP、Hooks、任务、会话及文件工具，不再实施本地模型推理、远程 Bridge/SDK、官方订阅权益、云端同步或厂商遥测。排除名单以 `docs/REMOVED_SCOPE.json` 为准；实时状态请执行 `python3 scripts/port_workbench.py counts --repo .`。

> **历史与执行的区别**：本文较早记录的 22/99/186 排除数、1,884/1,807/1,720 待迁移规模，以及 PR #2、远程 SDK/Bridge 待办均为历史资料，不再作为执行指令。当前工作提交至 PR #6，Issue #3 中 24 条取消开发、余 338 条继续闭环。

# 222 原版源码驱动的 Rust 全量迁移工作台

## 目的及边界

统一 `1,906` 条原路径的原版源码、函数出口、静态依赖、Rust 实现映射和验证证据，避免重复拉取、逐轮挑小文件以及将部分实现误记为完成。

**当前实施目标**：排除用户明确取消的 199 条原路径后，其余 **1,707** 条按完整功能、真实调用链、权限、异常及恢复逐项实现；推理只通过模型 API，原厂远程与云端功能不再跟进。已有 `docs/FILE_PORT_MATRIX.md` 仍是唯一状态台账，`docs/PORT_STATUS.md` 仍保留人工审计及技术差异。

工作台从当前矩阵实时计算未迁移与部分实现数量，组织成共享依赖业务域。自动生成的静态依赖仅帮助定位上下游，**不会自动宣称任何文件已完成**。

## 使用方式（不创建定时任务，不启用子代理）

首先在持久目录中准备两个独立源码树：

- Rust 仓库：`git clone -b fix/manual-source-parity-20261030 https://github.com/sy220284/222.git`，使用最新 PR HEAD；修改前与提交前重新确认分支提交和并行改动。
- 用户授权原版源码：解压 `claude-code-2.1.88-main(1).zip`，将其包含 `src/`、`vendor/` 的根目录作为 `ORIGINAL_ROOT`。原版不提交进公开 GitHub，不作为运行时捷径。

执行：

```sh
python3 scripts/port_workbench.py check --repo .
python3 scripts/port_workbench.py scan --repo . \
  --source "$ORIGINAL_ROOT" --output .port-workbench
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
```

扫描必须核对**所有**原路径及原版源码字节数/行数；发现缺文件、不同版本或台账数据不符立即报错，不能跳过。生成：

- `source-port-inventory.json`：逐路径 SHA-256、原始函数/类型出口（启发式）、TS 静态导入、依赖者、Rust 路径映射及差异。
- `port-work-plan.json`：按共享依赖和影响面排列的可机器处理工作单，不会生成代码空壳。
- `PORT_WORK_PLAN.md`：按业务域汇总缺口、关联入口、影响路径和待检查的相邻模块。

这些文件生成到本地 `.port-workbench/`，目录应加入 `.gitignore`。扫描数据是当前 HEAD 和本地原版的可再生结果，禁止用于伪造最终验收。

## 真正的开发循环

1. 从 `port-work-plan.json` 选取相同状态源、共享依赖和调用方关联的完整功能链；同时读取实际原版文件和相邻上下游。
2. 明确此链所有应保留函数、事件顺序、权限/副作用、失败、取消及恢复。已有 Rust 语义正确的部分直接复用；复合缺口优先修复共同底层。
3. 实现完整 Rust 功能；统一 CLI/TUI/SDK/MCP/Agent/远程实际生产入口，删除重复状态；禁止仅做静态 AST 转写、元数据、空壳和未接入模块。
4. 以原版输入/输出/副作用为基准做正常、失败、并发、权限、取消和重启测试；执行 `cargo fmt --all -- --check`、`cargo check --locked --all-targets`、`cargo test --locked --all-targets`。复杂链还需实际子进程或网络端到端测试。
5. 将真实代码、测试和台账作为一个语义完整提交；写入前按最新 HEAD 合并并行变更；写入后核对文件差异。缺口仍存在时留在“部分实现”，不核销。
6. 每次继续运行 `scan`，只复查变更影响到的调用链，不重复研究已锁定且未变化的路径。

## 完成验收门禁

`docs/PORT_BASELINE.json` 冻结工作台上线时的 1,906 路径状态及其原版 SHA-256（首次生成后不得覆盖）。`docs/PORT_EVIDENCE.jsonl` 记录新的“已验收”升级证据，每行必须包括：

```json
{"path":"src/example.ts","source_sha256":"原版 SHA256","rust_entrypoints":["src/agent.rs"],"test_names":["tests::behavior"],"tested_commit":"40位提交 SHA","failure_and_recovery_verified":true,"remaining_gaps":[]}
```

原路径新增标记“已验收”而无证据、存在未解决差异、Rust 入口文件缺失，或未经同意增加排除路径，`check` 将失败。**证明文件只负责防止明显虚报；录入文字本身不能替代代码级验收**。既有 29 个已验收项的历史证据仍需逐条复查，本工具不会替它们自动背书。

本地可独立运行 `check`；CI 中执行本命令及工作台测试，避免并行任务将台账破坏。源码树不进 CI，故源码静态全量核对在本地 `scan` 阶段完成，上传 GitHub 之前必须人工执行并检查。

### 增量影响分析

Git 提交中变更了一个公共 Rust 文件时，可以先执行：

```sh
python3 scripts/port_workbench.py impact --repo . \
  --source "$ORIGINAL_ROOT" --output .port-workbench \
  --changed src/agent.rs --changed src/tools.rs
```

`change-impact.json` 会列出该 Rust 文件负责的原路径及其依赖者的传递闭包，只复核受影响功能链。对动态加载、配置注入、跨进程协议和运行时注册保留人工复查，不能把静态依赖图视为完整覆盖。
