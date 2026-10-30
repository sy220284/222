# 2026-10-29 · 20 分任务续接（二）：源码级真实功能补齐

## 范围与核验

仅修改 `feat/full-rust-parity`、Draft PR #2；原始对照包为资料库 `/claude-code-2.1.88-main(1).zip` 的 `source/src/`。本次逐行查看原版相关 TS/TSX 与现有 Rust 实际入口，新增 **5 个互不重复、用户可感知的业务功能补齐**。同一功能的 CLI、SDK、内部方法与多次修正只计一次；额外的 SDK keep_alive 修复并入 SDK 会话生命周期，不独立计数。未运行 cargo fmt/check/clippy/test、Actions CI 或行为测试；全部 **已提交、待验收**。Issue #3 冻结 362 条保持开放。

**本次目标：5/100，尚差 95 项。** 先行形成 100 个经原版/可达 Rust 双侧核验的真实缺口清单的 A 阶段仍未达标，不以候选命令、静态生成或文件数补足。以下是真实代码提交证据及续接点。

| 独立功能 | 原版真实调用链 | Rust 实际新增入口与改动 | 提交 SHA | 未等价差异 |
|---|---|---|---|---|
| 指定 MCP 服务独立重连（CLI + SDK 合计一项） | `src/commands/mcp/mcp.tsx:71-72`、`src/components/mcp/MCPReconnect.tsx:30-62`、`src/services/mcp/useManageMCPConnections.ts:1043-1070`、`src/cli/print.ts:3133-3205` | `/mcp reconnect SERVER_NAME` 与 SDK `mcp_reconnect.serverName` → `Agent::reconnect_named_mcp` → `McpManager::reconnect_named` → 既有事务式 `reconnect_server`，只更新指定服务工具路由、保持健康同伴并清除上下文缓存 | `052ec5d1a270ae7b2b5f8342a9443ad26a1f09cc`、`33396749557f84cd5fbf7edc78537c569043c40a`、`6a4f882249c2d293e25dd9b7517cc9ee4de1d2de`、`f8cf2eac5f354475121717f6553c2920752b0f22` | SDK 注入/动态服务、needs-auth、完整交互反馈、重连取消与竞态未核齐 |
| 多选问题同时选择预设选项与自定义 Other | `src/components/permissions/AskUserQuestionPermissionRequest/QuestionView.tsx:299-306`：`SelectMulti` 允许 `__other__` 与其他选项组合，追加自定义文本 | `ask_user_question::execute` 允许 `0,1,2`，验证自定义答案后将预设标签与自定义文本按原版顺序合并，仅单选显示/记录选项预览，保留自定义备注注解 | `336b9b9bf670ecad8df2576f809de3868b7e3950`、`c47659f899c19ed9eb6526c60785c179824a00c3`、`02a06e81decd0f929a64a3cf39b7a2cede16d0c4` | 完整 Ink 交互、外部编辑器、按键取消、SDK 回调尚未等价 |
| SDK 初始化一次性状态与首条用户消息隐式初始化 | `src/cli/print.ts:4355-4367`、`4059-4060` | SDK 流维护 `sdk_initialized`，重复 `initialize` 返回明确错误，首条合法用户消息隐式置位，初始化参数校验失败不占状态 | `f48767dc9897d58d6e9ab5b82e30001ba717515d`、`73d17828adddf62608bf82a6428989a27dcb4e5a` | SDK 初始化返回的动态 commands/agents 仍为空；完整初始化选项未支持 |
| SDK 正常结束会话 | `src/cli/print.ts:2850-2862`：`end_session` 回复后退出输入循环 | SDK `end_session` → 返回成功控制响应 → 退出 SDK 输入循环 → 调用外层现有 `Agent::finish` 生命周期清理 | `a94310ca73668f29c94f7dea99e43a026c7bdad9` | Rust SDK 仍同步等待模型调用，尚缺原版运行中异步 abort、在途 Hook/工具取消与输出收尾时序 |
| SDK 恢复默认模型 | `src/cli/print.ts:2933-2944`：`set_model("default")` 解析当前默认模型 | `sdk_nested_control("set_model")` 对 `default` 使用 `Config::select(None,None)` 恢复配置的默认提供商/模型，经过真实 `Agent::switch_model` | `4e61682a90f85e9a45b0318e185d57aadf83e020` | 原版模型切换消息、breadcrumb、会话模型元数据通知未完全核齐 |

另修复 SDK `keep_alive` 消息误报“Unsupported SDK event”：`src/cli/structuredIO.ts:344-346`、`src/cli/print.ts:4036-4038`，Rust SDK 静默忽略心跳；提交 `3d4a932a624b719edb57bb0f539c9ae53d638296`。归入 SDK 会话生命周期，**不另计功能**。

## 静态复核与下一准确断点

复查当前分支 `src/mcp.rs`、`src/agent.rs`、`src/main.rs`、`src/ask_user_question.rs` 的实际函数、CLI/SDK 调用接线，以上关键入口仍可见；没有行为验收或等价认证。原版完整源包已实际读取，且未使用提示词摘录包。

后续必须继续按同一原始源码双侧确认更多独立业务缺口，并排除其他并行任务已经补齐的项目。优先候选：SDK `mcp_toggle`（需项目级持久化+动态工具卸载/重载）、`interrupt`（需要并发 stdin/模型请求取消）、`seed_read_state`（真实读文件缓存）、`rewind_files`（文件快照恢复）、`reload_plugins`（插件运行时重载）；以及 CLI `/hooks` 增删改、`/memory`、`/config`、`/sandbox`。这些仅是**待核验候选**，没有列为本次 100 项问题清单或已完成数。

本次未修改 main、未新建 PR、未合并、未修改 Issue #3 冻结完成状态。
