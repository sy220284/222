# 222 原版源码驱动的 Rust 全量迁移工作台

## 唯一工作范围（2026-10-31，PR #6）

- 原版 Claude Code 2.1.88 的源路径固定为 **1,906** 条；原版 SHA-256 与起始状态在 docs/PORT_BASELINE.json 冻结，不随开发进度改写。
- 当前按用户授权 **排除 192 条，保留 1,714 条**。其中 22 条是原始排除，另外 170 条逐项登记在 docs/REMOVED_SCOPE.json；调整范围必须得到新的用户明确授权。
- 先前曾出现的 199、195 条排除和 1,707、1,711 条保留均为历史快照，当前禁止作为任务分派依据。
- 恢复的 UltraPlan、UltraReview 和 LocalTrigger 共 7 条源码路径仍处于“部分实现”，保留本地通用功能并持续核验。移除原厂远程、云端托管、本地模型推理及专属遥测，模型通过用户配置的 API 调用。严格区分已经授权排除的远程能力与需要保留的本地通用能力。
- **当前开发目标为 sy220284/222 的 PR #6，分支 fix/manual-source-parity-20261030**。Issue #3 原 362 项中 21 项已由用户授权取消，其余 341 项仍须闭环。

工作台使用现有文件作为唯一事实源，避免一份数据反复人工填报：

| 文件 | 作用 |
|---|---|
| docs/FILE_PORT_MATRIX.md | 原路径、当前状态、Rust 映射唯一台账 |
| docs/REMOVED_SCOPE.json | 170 条新增排除的逐项用户授权 |
| docs/PORT_BASELINE.json | 冻结 1,906 条原版哈希和首次状态 |
| docs/PORT_EVIDENCE.jsonl | 后续升级为“已验收”的测试与提交证据 |
| docs/PORT_STATUS.md | 人工核实的行为差异与历史审计 |
| scripts/port_workbench.py | 同一事实源生成计划、功能链、范围统计、增量影响及验收门禁 |

**任何生成报告都不反写状态矩阵、授权文件或冻结哈希。** 修改原版路径状态必须根据实际实现及有效验收证据；静态引用、Rust 文件存在和编译通过均不构成等价证明。

## 使用命令

准备仓库与用户授权的 2.1.88 原版源码：

~~~sh
git clone -b fix/manual-source-parity-20261030 https://github.com/sy220284/222.git
cd 222
export ORIGINAL_ROOT=/path/to/extracted/claude-code-2.1.88/source
~~~

普通开发及检查：

~~~sh
# 由逐文件台账直接计算当前范围；不需要手工维护数字
python3 scripts/port_workbench.py counts --repo .

# 严格检查批准的排除范围、既有证据与新增验收记录
python3 scripts/port_workbench.py check --repo .

# 无须原版 ZIP，也能生成全量逐路径功能链、领域进度及验收差异
python3 scripts/port_workbench.py dashboard --repo . --output .port-workbench

# 指定本次修改的 Rust 文件；生成直接映射复验提示
python3 scripts/port_workbench.py dashboard --repo . --output .port-workbench \
  --changed src/agent.rs --changed src/tools.rs

# 有原版源码时复验完整哈希、函数出口、静态依赖、受影响调用方
python3 scripts/port_workbench.py dashboard --repo . --source "$ORIGINAL_ROOT" \
  --output .port-workbench --changed src/agent.rs

# 完整扫描和优先级计划；检查全部原路径字节、行数与导入关系
python3 scripts/port_workbench.py scan --repo . \
  --source "$ORIGINAL_ROOT" --output .port-workbench

# 精确的原版静态反向依赖分析（需要原版源码）
python3 scripts/port_workbench.py impact --repo . --source "$ORIGINAL_ROOT" \
  --output .port-workbench --changed src/agent.rs

# 与 CI 一致的工作台回归测试
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
~~~

在本地有完整 Git 历史（含 HEAD^）时，可以使用 --changed-from-git 自动读取本次提交差异。GitHub PR6 的 CI 已采用该模式并将工作台报告上传为 port-workbench-dashboard 工件；CI 检出深度为两个提交。

## 报告的可核查内容

运行 dashboard 后生成两个文件：

- **PORT_DASHBOARD.json**：包含全部 1,906 条原路径，每条有状态、授权范围、原版哈希、Rust 映射、当前磁盘文件、测试记录、已知缺口、优先度；提供原版源码时还包含导出符号、静态依赖及依赖者。已验收旧基线中没有新版独立证据的路径会明确标成“需重新复核”。
- **PORT_DASHBOARD.md**：按单一真实范围显示总进度、各业务域缺口、映射/文件缺失、优先处理队列和改动需要复验的范围。

若提供 --changed，报告还会列出直接受影响原路径、需回归的已验收路径、未匹配文件；若提供 --source，则进一步沿原版静态导入关系计算上游调用方。对于动态调用、插件、SDK、跨进程协议和运行时注册，未匹配项必须人工复核，禁止假定不存在影响。

原有 scan 仍输出 source-port-inventory.json、port-work-plan.json、PORT_WORK_PLAN.md；reachability 和 stubs 专项检查也保留，避免破坏已有工作路径。

## 固定闭环动作

1. 检查用户最新需求、PR6 最新 HEAD、矩阵、已授权排除清单；对任何范围漂移先停止错误任务分派。
2. 运行 dashboard + scan，优先找多个模块共用的缺口，再选取完整的用户功能链。确认原版函数、参数、状态、权限、异常、取消与恢复，不用生成的 AST 或名称相似度冒充等价。
3. 对照原版实代码实现 Rust；从 CLI/TUI/SDK/Agent/MCP 等**仍保留的实际入口**调用到最终用户结果；清理重复状态与不可达代码。
4. 验证正常、失败、权限拒绝、取消、并发、重启恢复以及跨平台的实际行为；保留可追溯测试与当前提交证据。
5. 完整验收后才修改矩阵状态并更新 docs/PORT_EVIDENCE.jsonl；现存差异继续记为“部分实现”。运行 check、dashboard、影响分析和相关回归。
6. 推送到 PR6 后以**该次 HEAD** 对应的 CI 为验证依据，不沿用之前提交的通过结果。继续完成剩余真实缺口。

校验命令：cargo fmt --all -- --check、cargo clippy --locked --all-targets -- -D clippy::correctness -D clippy::suspicious、cargo check --locked --all-targets、cargo test --locked --all-targets。CI 对工作台进行强制校验并保存最新报告。

本工具不创建定时任务、不自动开启派生 Agent；提交/CI 触发工作台生成只属于一次已有构建，不代表后台任务持续运行。
