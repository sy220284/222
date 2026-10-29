# Issue #3：冻结 362 项源码级等价验收登记表

## 2026-10-29：权限规则来源与持久化主链补齐（仍未验收）

本轮直接读取用户原始 `claude-code-2.1.88-main(1).zip` 中 `src/utils/permissions/permissionsLoader.ts`（1–296 行）、`src/utils/settings/constants.ts`（8–22、159–167 行）、`src/utils/settings/settings.ts`（274–309、416–535 行）、`src/utils/permissions/permissionRuleParser.ts`。新增 `src/permission_settings.rs`，并在 `src/main.rs` 的真实 Agent 初始化入口接入来源规则加载；来源顺序为用户、项目、项目本地、现有配置规则、托管策略，且 `allowManagedPermissionRulesOnly` 启用时只接受托管策略并阻止持久化新增授权。新增加/删规则时保留其他设置字段、兼容旧工具名，跨进程文件锁与原子替换保护并发，拒绝损坏 JSON 覆盖与项目设置符号链接越界。新增多来源优先级、托管策略独占、规则增删、损坏文件、16 线程并发、符号链接、真实 Shell/文件系统副作用测试。厂商绑定目录改为 `.atlas`，托管策略通过 `ATLAS_POLICY_SETTINGS` 指定。

**尚未完成等价**：原版启用来源开关、MDM/注册表/远程托管策略、动态缓存失效、设置 UI 与审批更新、全部 schema 校验及重载时序；`permissionsLoader.ts` 和关联冻结路径继续保持未验收，不勾选、不增加台账已验收数。GitHub CI 结果以对应提交实际运行记录为准。


### 2026-10-29：PowerShell 原版工具名贯通 Agent／权限／前后台进程（仍未验收）

原版对照：`src/tools/PowerShellTool/PowerShellTool.tsx`（工具名 `PowerShell`、`command`、毫秒 `timeout`、`run_in_background`），`src/tools/PowerShellTool/toolName.ts`，`PowerShellTool/powershellPermissions.ts`。此前 Rust 只把默认 shell 设为 PowerShell 时执行通用 `run_command`，但原版 `PowerShell` 工具名没有进入模型工具列表，也没有接入执行器，存在真实调用链缺口。

现已在 PowerShell 会话中发布原版 `PowerShell` 模型工具定义，保留通用 `run_command`；`ToolExecutor` 将显式 PowerShell 工具独立路由到 PowerShell 可执行程序和对应权限规则、Shell 生命周期事件及毫秒超时；后台任务增加逐次 shell 参数，不再依靠修改共享默认 shell 选择，避免并发 Bash/PowerShell 混跑。新增执行入口（无 PowerShell 安装时明确报错，绝不回退 Bash）、权限拒绝、后台进程覆盖默认值隔离测试。

修改：`src/agent.rs`、`src/tools.rs`、`src/background.rs`；提交 `708f445`、`6c12ec0`、`342b5b3`、`9018b6d`、`06bdbce`、`a4637c8`、`ad8b571`。**不勾选任何冻结项**：PowerShell 完整 AST、权限 UI、沙箱、取消／重启时序、进程组与图像输出等原版行为仍缺。


### 2026-10-29：PowerShell 复合命令权限主链补证（未验收）

- 原版定位：`src/tools/PowerShellTool/powershellPermissions.ts` 的 `filterRulesByContentsMatchingInput`（163–326 行）、`getSubCommandsForPermissionCheck`（527 行起）和解析后 deny > ask > allow 汇总（877 行起）；邻接 `src/utils/powershell/parser.ts` 的语句／管道 AST。
- Rust 实际修改：`src/powershell_permission_matching.rs` 在规则判定前按 PowerShell 引号与注释边界拆分分号、管道、换行、`&&`、`||`；对每个子命令独立执行别名、大小写、拒绝／询问／允许匹配，拒绝优先。反引号转义、脚本块、子表达式、重定向等无法凭文本安全判定的语法要求人工审批，不再由首个命令的前缀允许规则放行。对明确整条命令授权仍保留精确匹配入口。
- 回归：`powershell_permission_matching::tests::compound_scripts_cannot_bypass_later_denies_or_unknown_statements`、`quotes_comments_and_dynamic_script_blocks_do_not_inherit_prefix_allow`，以及 `tools::tests::powershell_compound_permission_checks_reach_actual_tool_dispatch_gate`，覆盖真实工具授权入口、拒绝优先、管道、分号、换行、注释、嵌套表达式及 Bash/PowerShell 隔离。
- 证据提交：`e1131d8`、`daffb48`。**仍未通过完整源文件验收**：原版 PowerShell AST、路径安全、沙箱、用户审批 UI 和取消时序尚待逐函数／跨模块对照；Issue #3 冻结清单不得勾选。

## 真实进度（不以静态映射冒充验收）

- 固定范围：Issue #3 所列 **362 个唯一原文件**，冻结参照提交 `34292c9033a461d516ccebb9f559c79cce5c394e`；当前 PR #2 HEAD `8e53b87187744ec3090ace5602e7ee9a9f4efeb4`。
- 本轮对台账 **362/362** 条逐一核对源路径、原文件行数、Rust 映射、分类；**362 条均为部分实现，0 条可据此验收关闭**。没有原源码逐函数及实际调用链证据的条目保持开放。
- 原始对照来源：用户提供的 Claude Code 2.1.88 源码包；只引用其源路径与函数，不上传原包。**本报告的逐行清单属于范围和代码映射审计，不代表已经对 362 个文件逐函数执行/验收。**
- GitHub 本仓库当前仅有 PR #1 与 Draft PR #2；**PR #3 不存在**，因此没有可以对比、移植或合并的 PR #3 代码。Issue #3 是验收任务，所有修复只能进入 PR #2。
- 已确认的源码级缺口：`src/utils/permissions/shellRuleMatching.ts` 除匹配外还导出 `suggestionForExactCommand` 与 `suggestionForPrefix`（原文件 189–228 行），Rust 原先没有；提交 `8e53b871` 已补齐返回的 `addRules`、`allow`、`localSettings` 字段并加入边界回归。**该源文件仍未完成整个 Shell 权限调用链最终等价。**

## 冻结路径逐项交叉审计

| # | 2.1.88 原始源文件 | 行数 | PR #2 当前 Rust 映射 | 当前登记 | 文件级验收 |
|---:|---|---:|---|---|---|
| 1 | `src/utils/permissions/autoModeState.ts` | 40 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 2 | `src/utils/permissions/bashClassifier.ts` | 62 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 3 | `src/utils/permissions/bypassPermissionsKillswitch.ts` | 156 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 4 | `src/utils/permissions/classifierDecision.ts` | 99 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 5 | `src/utils/permissions/classifierShared.ts` | 40 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 6 | `src/utils/permissions/dangerousPatterns.ts` | 81 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 7 | `src/utils/permissions/denialTracking.ts` | 46 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 8 | `src/utils/permissions/filesystem.ts` | 1778 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 9 | `src/utils/permissions/getNextPermissionMode.ts` | 102 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 10 | `src/utils/permissions/pathValidation.ts` | 486 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 11 | `src/utils/permissions/permissionExplainer.ts` | 251 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 12 | `src/utils/permissions/PermissionMode.ts` | 142 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 13 | `src/utils/permissions/PermissionPromptToolResultSchema.ts` | 128 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 14 | `src/utils/permissions/PermissionResult.ts` | 36 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 15 | `src/utils/permissions/PermissionRule.ts` | 41 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 16 | `src/utils/permissions/permissionRuleParser.ts` | 199 | `src/permission_rules.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 17 | `src/utils/permissions/permissions.ts` | 1487 | `src/permission_rules.rs / src/tools.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 18 | `src/utils/permissions/permissionSetup.ts` | 1533 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 19 | `src/utils/permissions/permissionsLoader.ts` | 297 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 20 | `src/utils/permissions/PermissionUpdate.ts` | 390 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 21 | `src/utils/permissions/PermissionUpdateSchema.ts` | 79 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 22 | `src/utils/permissions/shadowedRuleDetection.ts` | 235 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 23 | `src/utils/permissions/shellRuleMatching.ts` | 229 | `src/permission_rules.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 24 | `src/utils/permissions/yoloClassifier.ts` | 1496 | `src/tools.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 25 | `src/services/mcp/auth.ts` | 2466 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 26 | `src/services/mcp/channelAllowlist.ts` | 77 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 27 | `src/services/mcp/channelNotification.ts` | 317 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 28 | `src/services/mcp/channelPermissions.ts` | 241 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 29 | `src/services/mcp/claudeai.ts` | 165 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 30 | `src/services/mcp/client.ts` | 3349 | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 31 | `src/services/mcp/config.ts` | 1579 | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 32 | `src/services/mcp/elicitationHandler.ts` | 314 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 33 | `src/services/mcp/headersHelper.ts` | 139 | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 34 | `src/services/mcp/InProcessTransport.ts` | 64 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 35 | `src/services/mcp/MCPConnectionManager.tsx` | 73 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 36 | `src/services/mcp/mcpStringUtils.ts` | 107 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 37 | `src/services/mcp/normalization.ts` | 24 | `src/runtime_contracts.rs / src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 38 | `src/services/mcp/oauthPort.ts` | 79 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 39 | `src/services/mcp/officialRegistry.ts` | 73 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 40 | `src/services/mcp/SdkControlTransport.ts` | 137 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 41 | `src/services/mcp/types.ts` | 259 | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 42 | `src/services/mcp/useManageMCPConnections.ts` | 1142 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 43 | `src/services/mcp/utils.ts` | 576 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 44 | `src/services/mcp/vscodeSdkMcp.ts` | 113 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 45 | `src/services/mcp/xaa.ts` | 512 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 46 | `src/services/mcp/xaaIdpLogin.ts` | 488 | `src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 47 | `src/skills/bundled/batch.ts` | 125 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 48 | `src/skills/bundled/claudeApi.ts` | 197 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 49 | `src/skills/bundled/claudeApiContent.ts` | 76 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 50 | `src/skills/bundled/claudeInChrome.ts` | 35 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 51 | `src/skills/bundled/debug.ts` | 104 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 52 | `src/skills/bundled/index.ts` | 80 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 53 | `src/skills/bundled/keybindings.ts` | 340 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 54 | `src/skills/bundled/loop.ts` | 93 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 55 | `src/skills/bundled/loremIpsum.ts` | 283 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 56 | `src/skills/bundled/remember.ts` | 83 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 57 | `src/skills/bundled/scheduleRemoteAgents.ts` | 448 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 58 | `src/skills/bundled/simplify.ts` | 70 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 59 | `src/skills/bundled/skillify.ts` | 198 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 60 | `src/skills/bundled/stuck.ts` | 80 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 61 | `src/skills/bundled/updateConfig.ts` | 476 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 62 | `src/skills/bundled/verify.ts` | 31 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 63 | `src/skills/bundled/verifyContent.ts` | 14 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 64 | `src/skills/bundledSkills.ts` | 221 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 65 | `src/skills/loadSkillsDir.ts` | 1087 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 66 | `src/skills/mcpSkillBuilders.ts` | 45 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 67 | `src/tools/AgentTool/agentColorManager.ts` | 67 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 68 | `src/tools/AgentTool/agentDisplay.ts` | 105 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 69 | `src/tools/AgentTool/agentMemory.ts` | 178 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 70 | `src/tools/AgentTool/agentMemorySnapshot.ts` | 198 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 71 | `src/tools/AgentTool/AgentTool.tsx` | 1398 | `src/agent.rs` / `src/worktree.rs` / `src/permission_rules.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 72 | `src/tools/AgentTool/agentToolUtils.ts` | 687 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 73 | `src/tools/AgentTool/built-in/claudeCodeGuideAgent.ts` | 206 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 74 | `src/tools/AgentTool/built-in/exploreAgent.ts` | 84 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 75 | `src/tools/AgentTool/built-in/generalPurposeAgent.ts` | 35 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 76 | `src/tools/AgentTool/built-in/planAgent.ts` | 93 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 77 | `src/tools/AgentTool/built-in/statuslineSetup.ts` | 145 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 78 | `src/tools/AgentTool/built-in/verificationAgent.ts` | 153 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 79 | `src/tools/AgentTool/builtInAgents.ts` | 73 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 80 | `src/tools/AgentTool/constants.ts` | 13 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 81 | `src/tools/AgentTool/forkSubagent.ts` | 211 | `src/agent.rs` / `src/worktree.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 82 | `src/tools/AgentTool/loadAgentsDir.ts` | 756 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 83 | `src/tools/AgentTool/prompt.ts` | 288 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 84 | `src/tools/AgentTool/resumeAgent.ts` | 266 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 85 | `src/tools/AgentTool/runAgent.ts` | 974 | `src/agent.rs / src/worktree.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 86 | `src/tools/AgentTool/UI.tsx` | 872 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 87 | `src/services/api/adminRequests.ts` | 120 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 88 | `src/services/api/bootstrap.ts` | 142 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 89 | `src/services/api/claude.ts` | 3420 | `src/provider.rs / src/sse.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 90 | `src/services/api/client.ts` | 390 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 91 | `src/services/api/dumpPrompts.ts` | 227 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 92 | `src/services/api/errors.ts` | 1208 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 93 | `src/services/api/errorUtils.ts` | 261 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 94 | `src/services/api/filesApi.ts` | 749 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 95 | `src/services/api/firstTokenDate.ts` | 61 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 96 | `src/services/api/grove.ts` | 358 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 97 | `src/services/api/logging.ts` | 789 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 98 | `src/services/api/metricsOptOut.ts` | 160 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 99 | `src/services/api/overageCreditGrant.ts` | 138 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 100 | `src/services/api/promptCacheBreakDetection.ts` | 728 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 101 | `src/services/api/referral.ts` | 282 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 102 | `src/services/api/sessionIngress.ts` | 515 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 103 | `src/services/api/ultrareviewQuota.ts` | 39 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 104 | `src/services/api/usage.ts` | 64 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 105 | `src/services/api/withRetry.ts` | 823 | `src/provider.rs / src/sse.rs / src/gemini_sse.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 106 | `src/hooks/useMailboxBridge.ts` | 22 | `src/in_memory_mailbox.rs / src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 107 | `src/utils/hooks/apiQueryHookHelper.ts` | 142 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 108 | `src/utils/hooks/AsyncHookRegistry.ts` | 310 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 109 | `src/utils/hooks/execAgentHook.ts` | 340 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 110 | `src/utils/hooks/execHttpHook.ts` | 243 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 111 | `src/utils/hooks/execPromptHook.ts` | 212 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 112 | `src/utils/hooks/fileChangedWatcher.ts` | 192 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 113 | `src/utils/hooks/hookEvents.ts` | 193 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 114 | `src/utils/hooks/hookHelpers.ts` | 84 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 115 | `src/utils/hooks/hooksConfigManager.ts` | 401 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 116 | `src/utils/hooks/hooksConfigSnapshot.ts` | 134 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 117 | `src/utils/hooks/hooksSettings.ts` | 272 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 118 | `src/utils/hooks/postSamplingHooks.ts` | 71 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 119 | `src/utils/hooks/registerFrontmatterHooks.ts` | 68 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 120 | `src/utils/hooks/registerSkillHooks.ts` | 65 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 121 | `src/utils/hooks/sessionHooks.ts` | 448 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 122 | `src/utils/hooks/skillImprovement.ts` | 268 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 123 | `src/utils/hooks/ssrfGuard.ts` | 295 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 124 | `src/tools/BashTool/bashCommandHelpers.ts` | 266 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 125 | `src/tools/BashTool/bashPermissions.ts` | 2622 | `src/tools.rs / src/permission_rules.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 126 | `src/tools/BashTool/bashSecurity.ts` | 2593 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 127 | `src/tools/BashTool/BashTool.tsx` | 1144 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 128 | `src/tools/BashTool/BashToolResultMessage.tsx` | 191 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 129 | `src/tools/BashTool/commandSemantics.ts` | 141 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 130 | `src/tools/BashTool/destructiveCommandWarning.ts` | 103 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 131 | `src/tools/BashTool/modeValidation.ts` | 116 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 132 | `src/tools/BashTool/pathValidation.ts` | 1304 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 133 | `src/tools/BashTool/prompt.ts` | 370 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 134 | `src/tools/BashTool/readOnlyValidation.ts` | 1991 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 135 | `src/tools/BashTool/sedEditParser.ts` | 323 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 136 | `src/tools/BashTool/sedValidation.ts` | 685 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 137 | `src/tools/BashTool/shouldUseSandbox.ts` | 154 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 138 | `src/tools/BashTool/UI.tsx` | 185 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 139 | `src/tools/BashTool/utils.ts` | 224 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 140 | `src/tools/PowerShellTool/clmTypes.ts` | 212 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 141 | `src/tools/PowerShellTool/commandSemantics.ts` | 143 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 142 | `src/tools/PowerShellTool/commonParameters.ts` | 31 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 143 | `src/tools/PowerShellTool/destructiveCommandWarning.ts` | 110 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 144 | `src/tools/PowerShellTool/gitSafety.ts` | 177 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 145 | `src/tools/PowerShellTool/modeValidation.ts` | 405 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 146 | `src/tools/PowerShellTool/pathValidation.ts` | 2050 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 147 | `src/tools/PowerShellTool/powershellPermissions.ts` | 1649 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 148 | `src/tools/PowerShellTool/powershellSecurity.ts` | 1091 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 149 | `src/tools/PowerShellTool/PowerShellTool.tsx` | 1001 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 150 | `src/tools/PowerShellTool/prompt.ts` | 146 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 151 | `src/tools/PowerShellTool/readOnlyValidation.ts` | 1824 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 152 | `src/tools/PowerShellTool/UI.tsx` | 131 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 153 | `src/services/compact/apiMicrocompact.ts` | 154 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 154 | `src/services/compact/autoCompact.ts` | 352 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 155 | `src/services/compact/compact.ts` | 1706 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 156 | `src/services/compact/compactWarningHook.ts` | 17 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 157 | `src/services/compact/compactWarningState.ts` | 19 | `src/runtime_contracts.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 158 | `src/services/compact/grouping.ts` | 64 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 159 | `src/services/compact/microCompact.ts` | 531 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 160 | `src/services/compact/postCompactCleanup.ts` | 78 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 161 | `src/services/compact/prompt.ts` | 375 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 162 | `src/services/compact/sessionMemoryCompact.ts` | 631 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 163 | `src/services/compact/timeBasedMCConfig.ts` | 44 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 164 | `src/tasks/DreamTask/DreamTask.ts` | 158 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 165 | `src/tasks/InProcessTeammateTask/InProcessTeammateTask.tsx` | 126 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 166 | `src/tasks/InProcessTeammateTask/types.ts` | 122 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 167 | `src/tasks/LocalAgentTask/LocalAgentTask.tsx` | 683 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 168 | `src/tasks/LocalMainSessionTask.ts` | 480 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 169 | `src/tasks/LocalShellTask/killShellTasks.ts` | 77 | `src/background.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 170 | `src/tasks/LocalShellTask/LocalShellTask.tsx` | 523 | `src/background.rs / src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 171 | `src/tasks/pillLabel.ts` | 83 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 172 | `src/tasks/RemoteAgentTask/RemoteAgentTask.tsx` | 856 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 173 | `src/tasks/stopTask.ts` | 101 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 174 | `src/tasks/types.ts` | 47 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 175 | `src/entrypoints/agentSdkTypes.ts` | 444 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 176 | `src/entrypoints/cli.tsx` | 303 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 177 | `src/entrypoints/init.ts` | 341 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 178 | `src/entrypoints/mcp.ts` | 197 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 179 | `src/entrypoints/sandboxTypes.ts` | 157 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 180 | `src/entrypoints/sdk/controlSchemas.ts` | 664 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 181 | `src/entrypoints/sdk/coreSchemas.ts` | 1890 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 182 | `src/entrypoints/sdk/coreTypes.ts` | 63 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 183 | `src/memdir/findRelevantMemories.ts` | 142 | `src/memory.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 184 | `src/memdir/memdir.ts` | 508 | `src/memory.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 185 | `src/memdir/memoryScan.ts` | 95 | `src/memory.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 186 | `src/memdir/memoryTypes.ts` | 272 | `src/memory.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 187 | `src/memdir/paths.ts` | 279 | `src/memory.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 188 | `src/memdir/teamMemPaths.ts` | 293 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 189 | `src/memdir/teamMemPrompts.ts` | 101 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 190 | `src/tools/FileEditTool/constants.ts` | 12 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 191 | `src/tools/FileEditTool/FileEditTool.ts` | 626 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 192 | `src/tools/FileEditTool/prompt.ts` | 29 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 193 | `src/tools/FileEditTool/types.ts` | 86 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 194 | `src/tools/FileEditTool/UI.tsx` | 289 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 195 | `src/tools/FileEditTool/utils.ts` | 776 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 196 | `src/tools/LSPTool/formatters.ts` | 593 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 197 | `src/tools/LSPTool/LSPTool.ts` | 861 | `src/lsp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 198 | `src/tools/LSPTool/prompt.ts` | 22 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 199 | `src/tools/LSPTool/schemas.ts` | 216 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 200 | `src/tools/LSPTool/symbolContext.ts` | 91 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 201 | `src/tools/LSPTool/UI.tsx` | 228 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 202 | `src/tools/BriefTool/attachments.ts` | 111 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 203 | `src/tools/BriefTool/BriefTool.ts` | 205 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 204 | `src/tools/BriefTool/prompt.ts` | 23 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 205 | `src/tools/BriefTool/UI.tsx` | 101 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 206 | `src/tools/BriefTool/upload.ts` | 175 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 207 | `src/tools/FileReadTool/FileReadTool.ts` | 1184 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 208 | `src/tools/FileReadTool/imageProcessor.ts` | 95 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 209 | `src/tools/FileReadTool/limits.ts` | 93 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 210 | `src/tools/FileReadTool/prompt.ts` | 50 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 211 | `src/tools/FileReadTool/UI.tsx` | 185 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 212 | `src/tools/ScheduleCronTool/CronCreateTool.ts` | 158 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 213 | `src/tools/ScheduleCronTool/CronDeleteTool.ts` | 96 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 214 | `src/tools/ScheduleCronTool/CronListTool.ts` | 98 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 215 | `src/tools/ScheduleCronTool/prompt.ts` | 136 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 216 | `src/tools/ScheduleCronTool/UI.tsx` | 60 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 217 | `src/tools/WebFetchTool/preapproved.ts` | 167 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 218 | `src/tools/WebFetchTool/prompt.ts` | 47 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 219 | `src/tools/WebFetchTool/UI.tsx` | 72 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 220 | `src/tools/WebFetchTool/utils.ts` | 531 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 221 | `src/tools/WebFetchTool/WebFetchTool.ts` | 319 | `src/webfetch.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 222 | `src/commands/login/index.ts` | 15 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 223 | `src/commands/login/login.tsx` | 104 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 224 | `src/commands/logout/index.ts` | 11 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 225 | `src/commands/logout/logout.tsx` | 82 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 226 | `src/tools/ConfigTool/ConfigTool.ts` | 468 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 227 | `src/tools/ConfigTool/prompt.ts` | 94 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 228 | `src/tools/ConfigTool/supportedSettings.ts` | 212 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 229 | `src/tools/ConfigTool/UI.tsx` | 38 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 230 | `src/tools/MCPTool/classifyForCollapse.ts` | 605 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 231 | `src/tools/MCPTool/MCPTool.ts` | 78 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 232 | `src/tools/MCPTool/prompt.ts` | 4 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 233 | `src/tools/MCPTool/UI.tsx` | 403 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 234 | `src/cli/ndjsonSafeStringify.ts` | 33 | `src/ndjson.rs / src/main.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 235 | `src/cli/print.ts` | 5595 | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 236 | `src/cli/structuredIO.ts` | 860 | `src/main.rs / src/agent.rs / src/sse.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 237 | `src/services/lsp/LSPClient.ts` | 448 | `src/lsp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 238 | `src/services/lsp/LSPServerManager.ts` | 421 | `src/lsp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 239 | `src/services/lsp/manager.ts` | 290 | `src/lsp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 240 | `src/services/SessionMemory/prompts.ts` | 325 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 241 | `src/services/SessionMemory/sessionMemory.ts` | 496 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 242 | `src/services/SessionMemory/sessionMemoryUtils.ts` | 208 | `src/agent.rs / src/session.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 243 | `src/tools/EnterPlanModeTool/EnterPlanModeTool.ts` | 127 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 244 | `src/tools/EnterPlanModeTool/prompt.ts` | 171 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 245 | `src/tools/EnterPlanModeTool/UI.tsx` | 33 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 246 | `src/tools/EnterWorktreeTool/EnterWorktreeTool.ts` | 128 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 247 | `src/tools/EnterWorktreeTool/prompt.ts` | 31 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 248 | `src/tools/EnterWorktreeTool/UI.tsx` | 20 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 249 | `src/tools/ExitPlanModeTool/ExitPlanModeV2Tool.ts` | 494 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 250 | `src/tools/ExitPlanModeTool/prompt.ts` | 30 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 251 | `src/tools/ExitPlanModeTool/UI.tsx` | 82 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 252 | `src/tools/ExitWorktreeTool/ExitWorktreeTool.ts` | 330 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 253 | `src/tools/ExitWorktreeTool/prompt.ts` | 33 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 254 | `src/tools/ExitWorktreeTool/UI.tsx` | 25 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 255 | `src/tools/FileWriteTool/FileWriteTool.ts` | 435 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 256 | `src/tools/FileWriteTool/prompt.ts` | 19 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 257 | `src/tools/FileWriteTool/UI.tsx` | 405 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 258 | `src/tools/GlobTool/GlobTool.ts` | 199 | `src/search.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 259 | `src/tools/GlobTool/prompt.ts` | 8 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 260 | `src/tools/GlobTool/UI.tsx` | 63 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 261 | `src/tools/GrepTool/GrepTool.ts` | 578 | `src/search.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 262 | `src/tools/GrepTool/prompt.ts` | 19 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 263 | `src/tools/GrepTool/UI.tsx` | 201 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 264 | `src/tools/ListMcpResourcesTool/ListMcpResourcesTool.ts` | 124 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 265 | `src/tools/ListMcpResourcesTool/prompt.ts` | 21 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 266 | `src/tools/ListMcpResourcesTool/UI.tsx` | 29 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 267 | `src/tools/NotebookEditTool/NotebookEditTool.ts` | 491 | `src/notebook.rs / src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 268 | `src/tools/NotebookEditTool/prompt.ts` | 4 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 269 | `src/tools/NotebookEditTool/UI.tsx` | 93 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 270 | `src/tools/ReadMcpResourceTool/prompt.ts` | 17 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 271 | `src/tools/ReadMcpResourceTool/ReadMcpResourceTool.ts` | 159 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 272 | `src/tools/ReadMcpResourceTool/UI.tsx` | 37 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 273 | `src/tools/RemoteTriggerTool/prompt.ts` | 16 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 274 | `src/tools/RemoteTriggerTool/RemoteTriggerTool.ts` | 162 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 275 | `src/tools/RemoteTriggerTool/UI.tsx` | 17 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 276 | `src/tools/SendMessageTool/prompt.ts` | 50 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 277 | `src/tools/SendMessageTool/SendMessageTool.ts` | 918 | `src/team_registry.rs / src/team_mailbox.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 278 | `src/tools/SendMessageTool/UI.tsx` | 31 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 279 | `src/tools/SkillTool/prompt.ts` | 242 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 280 | `src/tools/SkillTool/SkillTool.ts` | 1109 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 281 | `src/tools/SkillTool/UI.tsx` | 128 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 282 | `src/tools/TaskStopTool/prompt.ts` | 9 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 283 | `src/tools/TaskStopTool/TaskStopTool.ts` | 132 | `src/background.rs / src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 284 | `src/tools/TaskStopTool/UI.tsx` | 41 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 285 | `src/tools/TeamCreateTool/prompt.ts` | 114 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 286 | `src/tools/TeamCreateTool/TeamCreateTool.ts` | 241 | `src/team_registry.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 287 | `src/tools/TeamCreateTool/UI.tsx` | 6 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 288 | `src/tools/TeamDeleteTool/prompt.ts` | 17 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 289 | `src/tools/TeamDeleteTool/TeamDeleteTool.ts` | 140 | `src/team_registry.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 290 | `src/tools/TeamDeleteTool/UI.tsx` | 20 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 291 | `src/tools/TodoWriteTool/constants.ts` | 2 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 292 | `src/tools/TodoWriteTool/prompt.ts` | 185 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 293 | `src/tools/TodoWriteTool/TodoWriteTool.ts` | 116 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 294 | `src/tools/WebSearchTool/prompt.ts` | 35 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 295 | `src/tools/WebSearchTool/UI.tsx` | 101 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 296 | `src/tools/WebSearchTool/WebSearchTool.ts` | 436 | `src/websearch.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 297 | `src/utils/plugins/loadPluginHooks.ts` | 288 | `src/plugins.rs / src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 298 | `src/utils/plugins/mcpPluginIntegration.ts` | 635 | `src/plugins.rs / src/mcp.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 299 | `src/utils/plugins/pluginLoader.ts` | 3303 | `src/plugins.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 300 | `src/utils/task/diskOutput.ts` | 452 | `src/background.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 301 | `src/utils/task/framework.ts` | 309 | `src/background.rs / src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 302 | `src/utils/task/outputFormatting.ts` | 39 | `src/task_output_format.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 303 | `src/tools/AskUserQuestionTool/AskUserQuestionTool.tsx` | 266 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 304 | `src/tools/AskUserQuestionTool/prompt.ts` | 45 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 305 | `src/tools/REPLTool/constants.ts` | 47 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 306 | `src/tools/REPLTool/primitiveTools.ts` | 40 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 307 | `src/tools/shared/gitOperationTracking.ts` | 278 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 308 | `src/tools/shared/spawnMultiAgent.ts` | 1094 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 309 | `src/tools/TaskCreateTool/prompt.ts` | 57 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 310 | `src/tools/TaskCreateTool/TaskCreateTool.ts` | 139 | `src/task_graph.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 311 | `src/tools/TaskGetTool/prompt.ts` | 25 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 312 | `src/tools/TaskGetTool/TaskGetTool.ts` | 129 | `src/task_graph.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 313 | `src/tools/TaskListTool/prompt.ts` | 50 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 314 | `src/tools/TaskListTool/TaskListTool.ts` | 117 | `src/task_graph.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 315 | `src/tools/TaskUpdateTool/prompt.ts` | 78 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 316 | `src/tools/TaskUpdateTool/TaskUpdateTool.ts` | 407 | `src/task_graph.rs / src/agent.rs / src/team_mailbox.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 317 | `src/tools/ToolSearchTool/prompt.ts` | 122 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 318 | `src/tools/ToolSearchTool/ToolSearchTool.ts` | 472 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 319 | `src/context/mailbox.tsx` | 38 | `src/agent.rs / src/in_memory_mailbox.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 320 | `src/cost-tracker.ts` | 324 | `src/protocol.rs` / `src/session.rs` / `src/cost.rs` / `src/agent.rs` / `src/tool_metrics… | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 321 | `src/main.tsx` | 4684 | `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 322 | `src/query.ts` | 1730 | `src/agent.rs / src/sse.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 323 | `src/query/stopHooks.ts` | 474 | `src/hooks.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 324 | `src/QueryEngine.ts` | 1296 | `src/provider.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 325 | `src/services/claudeAiLimits.ts` | 516 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 326 | `src/services/claudeAiLimitsHook.ts` | 24 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 327 | `src/services/rateLimitMessages.ts` | 345 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 328 | `src/services/rateLimitMocking.ts` | 145 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 329 | `src/services/tools/toolHooks.ts` | 651 | `src/hooks.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 330 | `src/state/store.ts` | 35 | `src/reactive_store.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 331 | `src/Task.ts` | 126 | `src/task.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 332 | `src/tools/McpAuthTool/McpAuthTool.ts` | 216 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 333 | `src/tools/SleepTool/prompt.ts` | 18 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 334 | `src/tools/SyntheticOutputTool/SyntheticOutputTool.ts` | 164 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 335 | `src/tools/TaskOutputTool/TaskOutputTool.tsx` | 584 | `src/background.rs / src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 336 | `src/tools/testing/TestingPermissionTool.tsx` | 74 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 337 | `src/tools/utils.ts` | 41 | `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 338 | `src/utils/argumentSubstitution.ts` | 146 | `src/skill_arguments.rs` / `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 339 | `src/utils/array.ts` | 14 | `src/array_utils.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 340 | `src/utils/bash/shellQuote.ts` | 305 | `src/shell_quote.rs` / `src/skill_arguments.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 341 | `src/utils/cleanupRegistry.ts` | 26 | `src/cleanup_registry.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 342 | `src/utils/controlMessageCompat.ts` | 33 | `src/control_message_compat.rs / src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 343 | `src/utils/diff.ts` | 178 | `src/tool_metrics.rs` / `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 344 | `src/utils/envValidation.ts` | 39 | `src/env_validation.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 345 | `src/utils/extraUsage.ts` | 24 | `src/provider.rs / src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 346 | `src/utils/fileHistory.ts` | 1116 | `src/history.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 347 | `src/utils/getWorktreePaths.ts` | 71 | `src/worktree.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 348 | `src/utils/getWorktreePathsPortable.ts` | 28 | `src/worktree.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 349 | `src/utils/hooks.ts` | 5023 | `src/hooks.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 350 | `src/utils/mailbox.ts` | 74 | `src/in_memory_mailbox.rs / src/event_signal.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 351 | `src/utils/messageQueueManager.ts` | 548 | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 352 | `src/utils/modelCost.ts` | 232 | `src/cost.rs` / `src/config.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 353 | `src/utils/objectGroupBy.ts` | 19 | `src/collections.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 354 | `src/utils/QueryGuard.ts` | 122 | `src/query_guard.rs` / `src/sdk_queue.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 355 | `src/utils/semanticBoolean.ts` | 30 | `src/semantic_inputs.rs` / `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 356 | `src/utils/semanticNumber.ts` | 37 | `src/semantic_inputs.rs` / `src/tools.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 357 | `src/utils/set.ts` | 54 | `src/collections.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 358 | `src/utils/signal.ts` | 44 | `src/event_signal.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 359 | `src/utils/swarm/teamHelpers.ts` | 684 | `src/team_registry.rs / src/agent.rs / src/task_graph.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 360 | `src/utils/tasks.ts` | 863 | `src/task_graph.rs / src/task_events.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 361 | `src/utils/teammateMailbox.ts` | 1184 | `src/team_mailbox.rs / src/team_registry.rs / src/agent.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |
| 362 | `src/utils/worktree.ts` | 1520 | `src/worktree.rs` | 部分实现 | **未通过**：函数与跨模块等价证据不足 |

## 逐函数验收门槛与尚未关闭的主链

每行必须补齐源码定位、原版函数/事件、Rust 真实执行入口、正常路径、输入验证、错误和取消、权限/并发时序、重启持久化、UI/SDK/远程调用及与原版结果逐项对照的测试；提交与 CI 可复查后才更新原始矩阵并勾选 Issue。空值、模拟返回、常量替代实现不得作为验收。

当前原始 362 项涉及权限与 Bash/PowerShell、MCP OAuth 与多传输、模型/Agent、Hook、任务图和团队、终端 UI、会话记忆、SDK/流式、LSP、插件等关联主链，均仍有需要逐项关闭的差异。不能仅据 Rust `cargo test` 通过声称对原版等价。后续严格只更新同一 PR #2。


## 2026-10-29：冻结路径 #349 Hook 权限询问与实际执行阻断

**原版源码证据：** 2.1.88 `src/utils/hooks.ts` 的 `executePreToolHooks` 聚合 `permissionDecision: ask`，并在约 2849–2875 行将该权限行为及修改后的输入交给工具执行链；`src/services/tools/toolHooks.ts` 约 322–432 行的 `resolveHookPermissionDecision` 将 `ask` 作为 `forceDecision` 传给用户授权回调，而不是当作普通的无决定 Hook。即使正常工具策略允许，显式 `ask` 也应要求用户确认。

**Rust 实际差异与修复：** 原 `src/hooks.rs::run_command` 只处理 `deny`，丢失 `ask`，`src/agent.rs::ask` 因而在 `--approval full` 下直接执行有副作用的工具。现将 `PreToolUse` 的 `permissionDecision` 从 Hook JSON 传入 `HookOutcome`，在 Agent 每条真实工具调用前强制确认；非交互运行返回与工具 ID 对应的错误回填，拒绝执行和事后 Hook，仍允许同轮其余工具及下一轮模型请求；交互运行需用户明确输入 y/yes。多个 Hook 的询问不会被后续允许决定覆盖，原有显式拒绝仍优先。

**真实调用链回归：** `tests/integration_hook_lifecycle.rs::pre_tool_hook_ask_cannot_be_bypassed_by_full_approval` 使用独立 CLI 进程、真实 Shell Hook、模拟模型 HTTP 两轮工具调用和临时文件系统，验证 `--approval full` 下 Hook `ask` 不产生文件副作用、下一轮模型收到 `must-ask` 的拒绝结果。代码提交 `a1091560`、`2c08cb75`，格式及相邻并行改动整合 `c9e3b7b5`。最终 CI 应以最新 HEAD 的 Actions 实际结论为准。

**未闭环：** 原版 `allow` 跳过一般询问但仍服从显式 deny/ask 的完整决策、复杂交互式权限回调、全部 Hook 类型/异步取消、多个 Hook 输入修改时序及重启行为尚待补全；冻结 #349 **仍为部分实现、未通过完整等价验收**，362 条均不勾选。


## 2026-10-29：冻结 #19 权限设置损坏文件的运行时恢复（仍未验收）

**原版逐行依据：** 2.1.88 `src/utils/settings/settings.ts` 的 `parseSettingsFileUncached`（201–230 行）在 JSON 解析异常时返回 `settings: null`；`getSettingsForSourceUncached`（约 340–357 行）继续加载其他来源；`src/utils/permissions/permissionsLoader.ts::loadAllPermissionRulesFromDisk` 按已启用来源逐个汇集规则。相反，编辑入口 `settings.ts::updateSettingsForSource`（416 行起）拒绝用新数据覆盖语法损坏的文件。

**Rust 真实缺口：** `src/permission_settings.rs::SettingsPaths::load` 原先直接调用严格 `read_json`，一个项目设置 JSON 语法损坏会中断整个 Agent 启动，其他用户／本地有效规则无法生效；但修改权限文件时严格拒绝覆盖损坏内容的保护必须保留。

**代码修复：** 增加仅供执行加载的 `read_json_for_execution`，只对确认为 JSON 语法错误的来源输出警告并跳过，其他来源继续加载；符号链接、文件类型与路径边界等安全错误仍向上传播。写入路径继续使用严格 `read_json`，保留损坏文件原内容并拒绝覆盖。原版全部设置 schema 验证、逐条非法规则过滤、启用来源配置、MDM/远端托管及动态缓存时序尚未完成。

**真实调用与验证：** `permission_settings::tests::malformed_source_json_does_not_abort_other_valid_permission_sources` 使用三个来源的真实磁盘设置、实际 `ToolExecutor::execute` 启动 Shell，验证有效来源允许的文件落盘、另一个来源明确拒绝的文件未生成、损坏的项目设置不能被 `add` 覆盖。代码提交 `b89c0f0442af2c6f8d6542d855a0d04217f3e868`，CI 以该提交对应 Actions 最终结果为准。

**验收状态：** 冻结 #19 及其关联源路径仍为部分实现；固定 362 条完整验收仍为 **0/362**。本次不调整全仓动态台账，也不以单项恢复测试勾选冻结条目。
