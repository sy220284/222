# 01—10 号产品规格逐函数源码复核与落地队列

基准：用户上传的《产品设计逻辑.zip》、对应《说明书.zip》、原版 2.1.88 `source/src`，及 PR #6 `docs/FILE_PORT_MATRIX.md`。本文件在用户指定的十份文档范围内登记 **233 条原版源码归属路径**与 **90 个关键函数锚点**，所有函数先按真实代码/当前 HEAD CI 验收，禁止因 Rust 文件存在或一次单测通过而改为全等价。

## 逐文档状态与本轮执行

| 文档编号 | 源文件路径 | 部分实现登记 | 未迁移 | 已批准排除 | 本轮真实代码变动及剩余问题 |
|---|---:|---:|---:|---:|---|
| 01 | 30 | 16 | 6 | 8 | `src/main.rs` 的 `-p/--print`、`--output-format`、无配置依赖 version；`tests/integration_cli.rs`、`tests/version_cli.rs`；完整文档均待验收 |
| 02 | 10 | 6 | 4 | 0 | `src/sdk_queue.rs` 本地 `/help` 处理与提示队列回归；完整文档均待验收 |
| 03 | 15 | 13 | 1 | 0 | `src/agent.rs` Stop Hook 阻止继续时保存停止原因；`src/main.rs` 结果传播；完整文档均待验收 |
| 04 | 26 | 12 | 1 | 12 | `src/provider.rs` 修复 Retry-After 原版无 6 小时截断，增边界测试；完整文档均待验收 |
| 05 | 22 | 12 | 10 | 0 | 预算与模型选择沿既有 `src/query_token_budget.rs`、`src/config.rs`，后续需逐函数验证；完整文档均待验收 |
| 06 | 17 | 12 | 5 | 0 | 系统提示与项目指令沿既有 `src/conversation_context.rs`、`src/prompt_runtime.rs`，仍有附件和作用域差异；完整文档均待验收 |
| 07 | 14 | 12 | 2 | 0 | 压缩路径沿既有 `src/agent.rs` 和 `src/session.rs`；缓存编辑微压缩、边界重链仍缺；完整文档均待验收 |
| 08 | 36 | 29 | 3 | 0 | `src/tool_pool_merge.rs` 简化模式仅 Bash/Read/Edit，Agent 与执行验证入口同步约束；完整文档均待验收 |
| 09 | 39 | 33 | 3 | 0 | `src/tools.rs` 禁用后台时取消 Shell 模型参数、验证阶段拒绝后台请求；完整文档均待验收 |
| 10 | 24 | 15 | 8 | 1 | `src/tools.rs` 在 Read 前拦截可能阻塞的设备路径；图片/PDF/Notebook 仍须完整对照；完整文档均待验收 |

原始非排除路径中大量为部分实现；以上值反映本次执行前最新单一矩阵，不能将功能修复数量直接折算为完整验收。缺失源码路径仍需单独补足。**本次不变更 192 条用户批准排除名单，不改变 38 条历史验收登记，不宣称十份文档已全部闭环。**

## 90 个原版关键函数核查队列

| 文档 | 原源码函数及行号 | PR6 Rust 定位 | 单函数状态 |
|---|---|---|---|
| 01 | `src/QueryEngine.ts:209-1156` · `submitMessage` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/cli/print.ts:1865-2681` · `run` | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/cli/print.ts:455-974` · `runHeadless` | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/cli/print.ts:4893-5197` · `loadInitialMessages` | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/entrypoints/cli.tsx:33-299` · `main` | `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/main.tsx:585-856` · `main` | `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/main.tsx:884-4513` · `run` | `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/cli/print.ts:1934-2366` · `drainCommandQueue` | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 01 | `src/cli/print.ts:4336-4518` · `handleInitializeRequest` | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/QueryEngine.ts:209-1156` · `submitMessage` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/utils/processUserInput/processUserInput.ts:281-589` · `processUserInputBase` | 待定 | 需移植主链 |
| 02 | `src/utils/processUserInput/processUserInput.ts:85-270` · `processUserInput` | 待定 | 需移植主链 |
| 02 | `src/utils/messageQueueManager.ts:428-484` · `popAllEditable` | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/utils/messageQueueManager.ts:244-266` · `dequeueAllMatching` | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/utils/messageQueueManager.ts:167-193` · `dequeue` | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/utils/messageQueueManager.ts:273-292` · `remove` | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/utils/messageQueueManager.ts:298-316` · `removeByFilter` | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分代码/待源码级运行验收 |
| 02 | `src/QueryEngine.ts:1186-1295` · `ask` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/query.ts:241-1729` · `queryLoop` | `src/agent.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolExecution.ts:599-1745` · `checkPermissionsAndCallTool` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/QueryEngine.ts:209-1156` · `submitMessage` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/query/stopHooks.ts:65-473` · `handleStopHooks` | `src/hooks.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolExecution.ts:337-490` · `runToolUse` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolExecution.ts:1403-1474` · `addToolResult` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolExecution.ts:207-250` · `decisionReasonToOTelSource` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolOrchestration.ts:19-82` · `runTools` | `src/tool_orchestration.rs` | 部分代码/待源码级运行验收 |
| 03 | `src/services/tools/toolExecution.ts:578-597` · `buildSchemaNotSentHint` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/claude.ts:1017-2892` · `queryModel` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/errors.ts:425-934` · `getAssistantMessageFromError` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/withRetry.ts:170-517` · `withRetry` | `src/provider.rs / src/sse.rs / src/gemini_sse.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/errors.ts:222-382` · `logToolUseToolResultMismatch` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/client.ts:88-316` · `getAnthropicClient` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/errors.ts:965-1161` · `classifyAPIError` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/claude.ts:3063-3211` · `addCacheBreakpoints` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/claude.ts:1538-1729` · `paramsFromContext` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 04 | `src/services/api/withRetry.ts:696-787` · `shouldRetry` | `src/provider.rs / src/sse.rs / src/gemini_sse.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 05 | `src/services/api/claude.ts:1017-2892` · `queryModel` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 05 | `src/QueryEngine.ts:209-1156` · `submitMessage` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 05 | `src/services/api/claude.ts:3063-3211` · `addCacheBreakpoints` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 05 | `src/utils/model/model.ts:445-506` · `parseUserSpecifiedModel` | 待定 | 需移植主链 |
| 05 | `src/utils/model/model.ts:349-384` · `getPublicModelDisplayName` | 待定 | 需移植主链 |
| 05 | `src/utils/model/model.ts:570-614` · `getMarketingNameForModel` | 待定 | 需移植主链 |
| 05 | `src/utils/model/model.ts:217-270` · `firstPartyNameToCanonical` | 待定 | 需移植主链 |
| 05 | `src/services/api/claude.ts:1538-1729` · `paramsFromContext` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 05 | `src/services/api/claude.ts:272-331` · `getExtraBodyParams` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/services/api/claude.ts:1017-2892` · `queryModel` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/QueryEngine.ts:209-1156` · `submitMessage` | `src/provider.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/utils/attachments.ts:3532-3769` · `getTeammateMailboxAttachments` | `src/generated_ports/p_4d95b4cfbf91b.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/utils/attachments.ts:3020-3199` · `generateFileAttachment` | `src/generated_ports/p_4d95b4cfbf91b.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/services/api/claude.ts:3063-3211` · `addCacheBreakpoints` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/utils/claudemd.ts:697-788` · `processMdRules` | `src/conversation_context.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/utils/attachments.ts:743-1003` · `getAttachments` | `src/generated_ports/p_4d95b4cfbf91b.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/services/api/claude.ts:1538-1729` · `paramsFromContext` | `src/provider.rs / src/sse.rs` | 部分代码/待源码级运行验收 |
| 06 | `src/utils/claudemd.ts:1249-1318` · `getMemoryFilesForNestedDirectory` | `src/conversation_context.rs / src/agent.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/compact.ts:772-1106` · `partialCompactConversation` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/compact.ts:387-763` · `compactConversation` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/compact.ts:1136-1396` · `streamCompactSummary` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/sessionMemoryCompact.ts:514-630` · `trySessionMemoryCompaction` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/sessionMemoryCompact.ts:232-314` · `adjustIndexToPreserveAPIInvariants` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/autoCompact.ts:241-351` · `autoCompactIfNeeded` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/microCompact.ts:305-399` · `cachedMicrocompactPath` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/autoCompact.ts:160-239` · `shouldAutoCompact` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 07 | `src/services/compact/sessionMemoryCompact.ts:324-397` · `calculateMessagesToKeepIndex` | `src/agent.rs / src/session.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolExecution.ts:599-1745` · `checkPermissionsAndCallTool` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/tools.ts:193-251` · `getAllBaseTools` | `src/generated_ports/p_522c7b52c49b4.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolExecution.ts:337-490` · `runToolUse` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolExecution.ts:1403-1474` · `addToolResult` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/tools.ts:271-327` · `getTools` | `src/generated_ports/p_522c7b52c49b4.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolExecution.ts:207-250` · `decisionReasonToOTelSource` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolOrchestration.ts:19-82` · `runTools` | `src/tool_orchestration.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolExecution.ts:578-597` · `buildSchemaNotSentHint` | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分代码/待源码级运行验收 |
| 08 | `src/services/tools/toolOrchestration.ts:118-150` · `runToolsSerially` | `src/tool_orchestration.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:624-820` · `call` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:826-1143` · `runShellCommand` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/prompt.ts:275-369` · `getSimplePrompt` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/prompt.ts:172-273` · `getSimpleSandboxSection` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:95-172` · `isSearchOrReadBashCommand` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:555-623` · `mapToolResultToToolResultBlockParam` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:360-419` · `applySedEdit` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/BashTool.tsx:178-217` · `isSilentBashCommand` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 09 | `src/tools/BashTool/prompt.ts:42-161` · `getCommitAndPRInstructions` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:804-1086` · `callInner` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:496-651` · `call` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:1097-1183` · `readImageWithTokenBudget` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:418-495` · `validateInput` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:652-717` · `mapToolResultToToolResultBlockParam` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:195-225` · `detectSessionFileType` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:755-772` · `validateContentTokens` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:117-128` · `isBlockedDevicePath` | `src/tools.rs` | 部分代码/待源码级运行验收 |
| 10 | `src/tools/FileReadTool/FileReadTool.ts:347-360` · `prompt` | `src/tools.rs` | 部分代码/待源码级运行验收 |

## 核销条件与下一项

每个函数按原版参数、调用、正常与失败结果、权限、并发、取消、持久化、恢复及实际 CLI/SDK/Agent 入口逐项核验；变更纳入同一 PR #6，Rustfmt、Clippy、全量编译/测试和与 **最新 HEAD** 对应的 CI 都成功才能登记完成。跨文档共同函数只开发一次，共用验证证据，避免重复重写。

**当前明确未闭环：** 01 各种启动/控制输入与完整流式事件，02 命令解析全路径与弹窗/多模态，03 复杂错误/停止/取消，04 厂商协议与流式重试，05 精确模型别名/预算/回退，06 缓存块及附件动态来源，07 微压缩和历史边界，08 全部工具启用规则/并发，09 shell 输出异常与后台生命周期，10 图片/PDF/Notebook/读取缓存的完整原版语义。按照源代码事实保留待开发状态。
