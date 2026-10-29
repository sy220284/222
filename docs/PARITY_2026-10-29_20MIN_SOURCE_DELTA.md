# 2026-10-29 · 20 分任务：原版业务链补齐续接记录

## 范围和计数

本记录只计入 **10 个互不重复、可由用户直接调用的业务能力**。同一能力在 CLI、SDK 的不同入口和同一能力的内部辅助函数不重复计数。当前距离本任务单轮 100 项目标仍差 **90 项**，本轮不得标记达标，也不得把下述功能称为完整源码等价。

原始依据为用户资料库原始 `claude-code-2.1.88-main(1).zip` 中的 `source/src/`，没有使用提示词摘录包；Rust 修改仅在 `feat/full-rust-parity`、Draft PR #2。没有运行 cargo、Actions 或行为测试；以下全部为 **已提交、待验收**。Issue #3 固定 362 项保持开放，`src/generated_ports` 不计数。

| # | 用户可感知业务能力 | 原版路径与入口 | 本轮 Rust 可达调用链 | 关键提交 | 尚未等价 |
|---:|---|---|---|---|---|
| 1 | 查看当前上下文占用和分类（CLI /context、SDK get_context_usage 共计一项） | `src/commands/context/context-noninteractive.ts::collectContextData`、`src/utils/analyzeContext.ts`、`src/cli/print.ts` | `main.rs` CLI/SDK → `Agent::context_usage` → `context_usage::Snapshot`；模型请求真实工具清单、消息块分类、最近 API 用量 | `5190675`、`24e5e86`、`e6e818a`、`2ea19eb`、`8e5b9a5`、`8224986` | 厂商精确 tokenizer、上下文折叠、动态压缩缓冲区、完整网格和技能/记忆细分；未知窗口明确为 unknown |
| 2 | 复制最近助手回复或指定代码块（/copy） | `src/commands/copy/copy.tsx::collectRecentAssistantTexts`、`copyOrWriteToFile` | `main.rs` → `Agent::copy_response` → `copy_response::copy`；20 条倒序、代码围栏、OSC52、独立私有临时文件 | `bf98b6b`、`eb935c6`、`8e5c2da`、`93925af`、`296abf5` | 原版 JSX 选择器、全部 Markdown lexer 情形、终端实际剪贴板回执 |
| 3 | SDK 获取设置来源与合并结果（get_settings） | `src/cli/print.ts` 的 `get_settings`、`src/utils/settings/settings.ts::getSettingsWithSources` | `main.rs::sdk_nested_control` → `SettingsPaths::settings_snapshot`；用户/项目/本地/托管策略优先级 | `72d1a2c`、`5125ea3` | flagSettings 动态层、全设置 schema、环境覆盖的 effort |
| 4 | 按来源查看已生效权限规则（/permissions） | `src/commands/permissions/permissions.tsx`、`src/components/permissions/rules/PermissionRuleList.tsx` | `main.rs` → `SettingsPaths::render_permission_rules`；按来源显示 deny/ask/allow、默认模式、目录，不泄露无关设置 | `b472a8c`、`5125ea3` | 原版交互增删界面、拒绝重试交互；现有 /permission add/delete 仍是独立旧入口 |
| 5 | 查看工作区 Git 差异（/diff） | `src/commands/diff/diff.tsx`、`src/components/diff/DiffDialog.tsx` | `main.rs` → `diff_view::render`；固定 argv 读取状态、已暂存和未暂存 diff | `faf3e42`、`f9c8652` | 原版逐轮 diff 视图、交互选择与未跟踪文件内容预览 |
| 6 | 列出有权限访问的项目/插件技能（/skills） | `src/commands/skills/skills.tsx`、`src/components/skills/SkillsMenu.tsx` | `main.rs` → `Agent::list_skills` → `ToolExecutor::list_skills`；复用读取权限和插件边界检查 | `8b23eac`、`c81cfd6`、`a166227` | 原版技能交互菜单、技能预览和执行导航 |
| 7 | 列出项目与用户自定义 Agent（/agents） | `src/commands/agents/index.ts`、`src/commands/agents/agents.tsx` | `main.rs` → `Agent::list_agents` → `agent_definition::existing_agent_types`；项目目录越界校验 | `28fd3ed`、`43e3fff` | 原版创建、编辑、删除、工具和模型选择 UI |
| 8 | 查看 MCP 服务连接状态（/mcp status） | `src/commands/mcp/mcp.tsx`、`src/services/mcp/MCPConnectionManager.tsx` | `main.rs` → `Agent::mcp_statuses` → `McpManager::statuses` | `74d1e64` | 原版交互启用/禁用、逐服务配置编辑 |
| 9 | 对失败的 MCP 服务执行重新连接（/mcp reconnect） | `src/commands/mcp/mcp.tsx`、`src/services/mcp/client.ts` | `main.rs` → `Agent::reconnect_failed_mcp` → `McpManager::reconnect_failed`，重新获取状态并使上下文快照失效 | `3438d50`、`74d1e64` | 指定服务重连、交互授权与完整传输恢复 UI |
| 10 | 查看当前会话的 Hook 事件、匹配器与超时（/hooks） | `src/commands/hooks/hooks.tsx`、`src/utils/hooks.ts` | `main.rs` → `Agent::hooks_report`；仅显示事件与配置，不输出可能含凭据的命令文本 | `5ff5764`、`02cd139` | 原版交互添加、编辑、删除、测试与完整 Hook 类型配置 UI |

## 本轮静态复核与续接点

对最新分支读取 `main.rs`、`agent.rs`、`context_usage.rs`、`copy_response.rs`、`diff_view.rs`、`permission_settings.rs`、`tools.rs`、`mcp.rs`，核对 29 项入口、方法、模块引用与安全接线，**29/29 静态检查通过**（此检查早于 /hooks 新接线；后续仍需再次静态核对）；这不构成编译或行为验收。同期其他任务在相同分支持续提交，本轮逐文件按最新 blob SHA 更新，未覆盖并行修改。

**后续必须先补齐 100 项真实问题清单并逐项源码确认**，优先围绕 SDK `interrupt` / `rewind_files` / `seed_read_state` / `reload_plugins` / `mcp_toggle`，以及 CLI `/hooks` / `/memory` / `/rewind` / `/config` / `/sandbox` 等尚未完成的真实调用链逐一查缺。这里只列下一轮待核查候选，**不把候选或原版命令名计为本轮已完成**。
