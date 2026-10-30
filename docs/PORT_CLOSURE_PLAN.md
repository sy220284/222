> **2026-10-30 云端/远程第二轮代码核查：** 新增确认 87 条分散的云端专属路径；排除总数更新为 186（原厂遥测22 + 用户新增164），剩余非排除 1720 条；用户授权清单见 `docs/REMOVED_SCOPE.json`。后续旧数值仅供历史追溯，已移除功能不再跟进。

> **2026-10-30 用户最新授权：** 只接入模型 API、移除本地模型、远程 Bridge/SDK 与云端专属功能；已批准新增 77 条排除（总计 99），保留 1807 条源码路径继续完整验收。授权名单以 docs/REMOVED_SCOPE.json 为准；下文旧统计和远程计划仅作历史参考。

# 222 Rust 迁移完整闭环执行计划（基于工作台全量扫描）

> 执行目标：2.1.88 原版 1,906 条路径，22 条用户明确排除，**1,884 条非排除路径逐项完成 Rust 等价实现、实际入口接线及验收**。本计划是执行顺序，不将整体交付拆减为低标准的小任务。
>
> 2026-10-29 快照：797 条未迁移、1,058 条部分实现/部分覆盖、29 条独立已验收、22 条排除。并行提交随时改变数字，开始开发和提交前一律通过 `scripts/port_workbench.py scan` 和最新 `docs/FILE_PORT_MATRIX.md` 动态重算；最新远端已出现 792/1,063 的变化。

## 1. 唯一范围、证据和状态事实源

- 原版唯一基准：用户提供的 `claude-code-2.1.88-main(1).zip`，不提交到 GitHub。`docs/PORT_BASELINE.json` 冻结每条源文件 SHA-256；原版源码路径、函数、类型、状态和异常用于行为对照。
- 最新代码：`sy220284/222` 的 `feat/full-rust-parity`，草稿 PR #2。`docs/FILE_PORT_MATRIX.md` 是逐条路径状态事实源，`docs/PORT_STATUS.md` 是技术差异说明，`docs/PORT_EVIDENCE.jsonl` 是新增已验收路径的可追溯证据。
- 内部工作清单为完整功能链，保留路径一对多及多对一映射；**不以 Rust 文件数、生成 AST、编译绿灯或部分入口连通作为验收**。所有部分实现项（含 Issue #3 冻结 362 项）继续留在总范围，不能另开已完成口径。
- 删除厂商账号、专属服务和行为/环境遥测；本地或多提供商协议保留同类通用功能，未经批准不删除通用能力。

## 2. 扫描排序与依赖主链

以工作台报告 `/port-audit-output/PORT_WORK_PLAN.md` 的反向依赖为排序依据，以源码级调查消除静态扫描漏项。优先级按：影响生产入口/安全与状态一致性 > 被多个业务调用方复用的能力 > 完整功能链中阻塞下游的依赖 > 单独 UI 细节。动态加载、插件钩子、SDK 入口以及运行时配置必须额外做真实执行确认。

| 顺序 | 主链范围 | 关键原版路径／已知阻塞 | 必须形成的运行态结果 |
|---|---|---|---|
| A | 基础运行时、文件、路径、配置 | `src/utils/path.ts`、`envUtils.ts`、`execFileNoThrow.ts`、`fsOperations.ts`、`settings/settings.ts`、`src/utils/auth.ts` | 单一配置真源；跨平台路径与 FS 安全；子进程退出/输出/超时/取消；有效错误返回，工作区审批仍有效 |
| B | Agent、模型路由、工具权限 | `src/QueryEngine.ts`、`src/utils/model/model.ts`、`src/tools/AgentTool/loadAgentsDir.ts`、权限文件与 Bash/File 工具 | CLI、TUI 与 SDK 同一多模型执行链；工具权限、Hook、MCP、取消与恢复在实际调用中生效 |
| C | 命令、MCP、插件及任务 | `src/commands.ts`、`src/types/command.ts`、`src/services/mcp/types.ts`、任务图和插件管理 | 用户输入到注册、执行、工具审批与持久化回执全通，正常/拒绝/失败/恢复均可测试 |
| D | 终端 UI 与消息交互 | `src/ink.ts`、`src/hooks/useTerminalSize.ts`、`src/components/permissions/PermissionRequest.tsx`、消息及输入组件 | 真正的终端渲染与键盘事件、窗口调整、历史、弹窗、权限和取消，不是静态 JSX 转写 |
| E | SDK、桥接、团队和远程 | `src/bridge/*`、`src/cli/transports/*`、`src/utils/swarm/*`、团队/会话存储 | 远程会话/鉴权/断线重连/批次上传/附件与取消、SDK 协议及队列重放；来源、身份和副作用幂等 |
| F | 全量关联复核 | `src/utils/*`、`src/components/*`、`src/hooks/*`、`src/ink/*`、vendor 4 条 | 所有非排除路径逐一完成入口和行为验证；补齐单点细节和真实横向调用方 |

## 3. 不得省略的工程闭环步骤

1. **冻结输入**：读取最新 HEAD、PR #2、Issue #3、矩阵与原版 SHA。锁定当前功能链的原版源码、关联类型、调用者、持久化协议和 Rust 现有对应入口。
2. **列出差异**：逐个原版函数及每个外部可观察状态建立 `源函数 → Rust 函数 → CLI/TUI/SDK/Agent/MCP/远程入口 → 测试用例` 映射；列出缺失行为，不得只复制类型或纯静态值。
3. **根因实现**：优先修复共享核心边界并让所有有效调用方收敛；保留唯一状态/权限所有者；实现超时、取消、并发、错误、重试边界、数据恢复与下游结果。不要套接不存在的函数、降低合法并发或使用固定回执。
4. **全面测试**：正常、错误、超时/取消、权限拒绝、并发/竞争、进程崩溃与重启、恢复、Windows/macOS/Linux 差异按实际适用覆盖。使用真实 CLI 子进程、临时目录/工作树、模拟 HTTP 模型、MCP 进程及远程重连驱动跨模块链。
5. **双向审计**：工作台 `impact --changed <Rust文件>` 确认反向依赖受影响源路径；对照源码检查未触达的原版分支及旁路入口。只修本轮新影响和相关公共边界，既不忽略缺口，也不反复全仓重看已验收无变化模块。
6. **更新证据**：所有剩余差异为零，且已运行测试、实际入口可达，才写 `docs/PORT_EVIDENCE.jsonl` 并提升为 `已验收`。否则保留 `部分实现`，技术差异明确记录。
7. **提交和远端复查**：在 Draft PR #2 按功能链原子提交，提交前读最新 HEAD 并做**语义合并**，禁止覆盖并行改动；在新 HEAD 重新检查 `cargo fmt`, `cargo check`, `cargo test`, Clippy 和安全 CI；修复失败并重试，证据与 HEAD 绑定。
8. **接续未完成范围**：验证通过即从最新缺口队列取下一个阻塞链；不会因一个模块或一次提交结束宣称整个迁移完成。中断时保留 HEAD、未完成项目、复现命令和测试结果便于续作。

## 4. 全量执行与恢复命令

```sh
python3 scripts/port_workbench.py scan --repo . --source "$ORIGINAL_ROOT" --output port-audit-output
python3 scripts/port_workbench.py impact --repo . --source "$ORIGINAL_ROOT" --changed src/workspace_dirs.rs --output port-audit-output
python3 scripts/port_workbench.py check --repo .
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
```

根据 `port-audit-output/PORT_WORK_PLAN.md` 按阻塞主链而非文件大小排序，结合 `source-index.json` 中的 `dependencies` 和 `exports` 对源码逐项复核。报告、源码哈希和失败日志作为工作产物保存，但**不得将用户提供的完整原版源码提交到公开仓库**。

## 5. 完成门槛与禁止操作

- 计数关系始终满足：`未迁移 + 部分实现 + 已验收 + 排除 = 1,906`，所有 1,884 非排除路径独立验收才满足最终目标。
- 功能主链需要从实际生产入口执行，包含权限、状态所有权、外部副作用、存储、恢复以及用户可见结果。模拟测试仅覆盖被模拟的契约。
- PR #2 保持 Draft；不自行合并 main、不关闭 Issue #3、不自行创建新的定时任务或子代理。已核实失败时定位根因并继续修复；没有运行过的测试不可写成已通过。
- 终止条件仅为 1,884 路径及完整跨模块链全部闭环且最终最新 HEAD 的 CI 验证成功；其它停止均应明确列出实际剩余路径。

## 6. 当前已落地的基础链修正

- 将 `src/utils/path.ts` 的路径展开、归一化、相对路径、安全穿越识别与目录判定移植为 Rust `src/path_support.rs`，并与 `src/workspace_dirs.rs` 中工作区授权边界相连。原版 `sanitizePath` 引用源与其它动态调用方尚需复核，先登记为部分实现。
- 纠正任务图只读查询对无效 ID 的行为：对外返回“Task not found”，内部更新、删除路径仍拒绝非法 ID；验证正常读取和路径穿越安全。
- 持续按最新 HEAD 和测试结果复查，在功能差异完成前不提前核销源码路径。
