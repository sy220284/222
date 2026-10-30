# 2.1.88 → Rust 逐文件迁移台账（原始清单）

基于用户提供源码包提取出的 **1,906** 个 TS/TSX/JS 文件。原路径保留仅供映射与核对，不用于运行时品牌与界面。状态为按功能模块的保守初判，**不代表已完成逐文件行为等价**。

执行规则：原厂登录、专属模型 API → 可配置多模型实现；原厂遥测／行为与环境数据回传 → 删除；其他代码 → Rust 等价实现并提供行为测试。改造的远程功能转为自建协议，而不是简单删除。

注：应用发布包源码映射不含完整原生代码及测试，无法从这些文件证明未知原生实现的精确行为。

| 原路径 | 原行数 | 原字节数 | 处理方式 | Rust 目标 | 当前验证状态 |
|---|---:|---:|---|---|---|
| `src/QueryEngine.ts` | 1296 | 46630 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/Task.ts` | 126 | 3152 | 重写：任务调度 | `src/background.rs / src/task.rs` | 部分实现：local_bash 任务使用原版 b+8 位小写数字字母 ID，查询/停止/增量读取兼容旧 UUID；新增会话级输出隔离、启动失败回收、输出落盘错误状态，源码已提交待验收；其他任务类型、统一生命周期和任务依赖仍未等价 |
| `src/Tool.ts` | 793 | 29516 | 重写：保持通用行为 | `src/generated_ports/p_2980dcfa0210a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands.ts` | 755 | 25185 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context.ts` | 190 | 6446 | 重写：保持通用行为 | `src/conversation_context.rs / src/agent.rs / src/main.rs` | 部分实现：会话首次启动的真实 Git 状态/分支/最近提交快照与本地日期进入模型 system，后续轮次不刷新；补齐用户级 AGENTS.md/规则、父目录继承、项目及本地指令、递归无条件规则与 paths 条件规则隔离，并接通 Agent 系统提示词；Git 状态 UTF-16 截断与用户指令配置错误传播修复（42b729ca、549f12b2、5db0100c、24e27e1c），源码已提交待验收；已补充工作区内 @include 递归引用（b7dd2cf0、1b670581、ae1f934b），源码已提交待验收；嵌套 Git 工作树主仓库项目指令去重（fb4210e7），保留主仓库本地指令；授权附加目录指令按显式 ATLAS_ADDITIONAL_DIRECTORIES_INSTRUCTIONS 开关加载并接入 Agent/上下文统计（80336875、22b50ecd），均已提交待验收；原版外部引用审批、完整条件规则按文件动态加载、记忆排除、源设置开关和完整 SDK 行为仍待等价；指令 glob 排除与来源独立开关、外部 @引用显式配置授权已接通真实 Agent，代码 7de3b735/03b5697f/95f8dde9/f3c2c0f6/93493750/123d67f0，均已提交待验收；完整交互审批、动态条件规则、SDK 来源传播与跨平台行为仍缺 |
| `src/cost-tracker.ts` | 324 | 10706 | 重写：用量与成本统计 | `src/protocol.rs` / `src/session.rs` / `src/cost.rs` / `src/agent.rs` / `src/tool_metrics.rs` | 部分实现：多模型/子代理/团队费用汇总、会话恢复、API/工具/墙钟耗时及改动行数；已补充模型请求最终失败时 API/墙钟耗时持久化；重试逐次耗时拆分、退出回调仍待核对 |
| `src/costHook.ts` | 23 | 617 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/dialogLaunchers.tsx` | 133 | 22948 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/history.ts` | 465 | 14081 | 重写：保持通用行为 | `src/prompt_history.rs` | 部分实现：全局 JSONL 历史、会话优先、去重检索、撤销与粘贴引用；原版 React 上下键和异步写入时序仍待验证 |
| `src/ink.ts` | 86 | 3887 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/interactiveHelpers.tsx` | 366 | 57424 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/main.tsx` | 4684 | 803924 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/projectOnboardingState.ts` | 84 | 2337 | 重写：保持通用行为 | `src/project_onboarding.rs / src/main.rs` | 部分实现：工作区是否为空、工程指令文件探测、最多四次提示、按工作区持久化完成标记，接入真实交互 CLI；原版交互式 onboarding 视觉组件未等价 |
| `src/query.ts` | 1730 | 68683 | 重写：模型／Agent 循环 | `src/agent.rs / src/sse.rs` | 部分实现：多轮循环、SSE 增量和工具回填；原版上下文状态机未全等；模型返回非法 JSON/非对象工具参数时逐工具错误回填，不中断同轮合法兄弟工具及后续模型请求；Pre/PostToolUse 子进程 Hook 异常逐工具回填且保留原操作副作用和后续合法工具 ；SDK 文本+base64 图片混合块经 Agent 传递到多模型、SSE 与会话恢复；SSE 无终止标记时拒绝执行未确认工具；多模态 UI/非图片块仍未等价|
| `src/replLauncher.tsx` | 23 | 3517 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/setup.ts` | 478 | 20646 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/tasks.ts` | 40 | 1355 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/tools.ts` | 390 | 17294 | 重写：保持通用行为 | `src/generated_ports/p_522c7b52c49b4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/assistant/sessionHistory.ts` | 88 | 2503 | 重写：保持通用行为 | `src/generated_ports/p_3532178887ef5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/bootstrap/state.ts` | 1759 | 56109 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/bridge/bridgeApi.ts` | 540 | 18066 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeConfig.ts` | 49 | 1695 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeDebug.ts` | 136 | 4926 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeEnabled.ts` | 203 | 8442 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeMain.ts` | 3000 | 115571 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeMessaging.ts` | 462 | 15703 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgePermissionCallbacks.ts` | 44 | 1411 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgePointer.ts` | 211 | 7611 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeStatusUtil.ts` | 164 | 5143 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/bridgeUI.ts` | 531 | 16780 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/capacityWake.ts` | 57 | 1841 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/codeSessionApi.ts` | 169 | 4840 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/createSession.ts` | 385 | 12157 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/debugUtils.ts` | 142 | 4240 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/envLessBridgeConfig.ts` | 166 | 7250 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/flushGate.ts` | 72 | 1981 | 改造：自建远程连接协议 | `src/bridge_core.rs / src/main.rs` | 部分实现：本地 SDK 控制入口可达；初始历史回放中的出站消息有序排队、传输替换保留、永久关闭清除与隔离测试已通过；远程 HTTP/WebSocket 实际发送仍未贯通 |
| `src/bridge/inboundAttachments.ts` | 176 | 6267 | 改造：自建远程连接协议 | `src/bridge_inbound_attachments.rs / src/main.rs` | 部分实现：可信可配置附件服务的 OAuth 风格令牌请求、30秒超时和16MiB限制、路径清洗及会话隔离、最后文本块 @引用插入与无附件容错已接入真实 SDK/模型链；原版上传服务专用协议、重连和所有附件恢复行为尚未对齐 |
| `src/bridge/inboundMessages.ts` | 81 | 2727 | 改造：自建远程连接协议 | `src/multimodal.rs / src/main.rs` | 部分实现：用户消息/UUID 提取、camelCase mediaType 兼容、缺失媒体类型 PNG/JPEG/GIF/WebP 魔数识别已通过 SDK 直接及排队模型 HTTP 测试；远端传输、附件及恢复链仍待迁移 |
| `src/bridge/initReplBridge.ts` | 570 | 23849 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/jwtUtils.ts` | 257 | 9444 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/pollConfig.ts` | 111 | 4562 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/pollConfigDefaults.ts` | 83 | 4018 | 改造：自建远程连接协议 | `src/bridge_core.rs / src/main.rs` | 部分实现：本地 SDK 控制入口可达；原版轮询/心跳默认数值与多会话默认值已验证；远端真实轮询调度与状态恢复仍未贯通 |
| `src/bridge/remoteBridgeCore.ts` | 1009 | 39434 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/replBridge.ts` | 2407 | 100537 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/replBridgeHandle.ts` | 37 | 1473 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/replBridgeTransport.ts` | 371 | 15523 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/sessionIdCompat.ts` | 58 | 2536 | 改造：自建远程连接协议 | `src/bridge_core.rs / src/main.rs` | 部分实现：本地 SDK 控制入口可达；会话标识双向转换与可选兼容门禁已测试；远程服务完整身份调用链仍未贯通 |
| `src/bridge/sessionRunner.ts` | 551 | 18020 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/trustedDevice.ts` | 211 | 7764 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/types.ts` | 263 | 10161 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/bridge/workSecret.ts` | 128 | 4672 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/buddy/CompanionSprite.tsx` | 371 | 45922 | 重写：保持通用行为 | `src/generated_ports/p_ab10542ccfa4c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/buddy/companion.ts` | 134 | 3715 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/buddy/prompt.ts` | 37 | 1451 | 重写：保持通用行为 | `src/generated_ports/p_bf68628a27ff8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/buddy/sprites.ts` | 515 | 9797 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/buddy/types.ts` | 149 | 3805 | 重写：保持通用行为 | `src/generated_ports/p_0e2477efcdf43.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/buddy/useBuddyNotification.tsx` | 98 | 9959 | 重写：保持通用行为 | `src/generated_ports/p_336c8aa7f1741.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/cli/exit.ts` | 32 | 1310 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/cli/handlers/agents.ts` | 71 | 2089 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/handlers/auth.ts` | 331 | 10756 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/handlers/autoMode.ts` | 171 | 5742 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/handlers/mcp.tsx` | 362 | 56167 | 重写：保持通用行为 | `src/generated_ports/p_f4f160d5e6a79.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/cli/handlers/plugins.ts` | 879 | 31073 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/handlers/util.tsx` | 110 | 14501 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/ndjsonSafeStringify.ts` | 33 | 1408 | 重写：保持通用行为 | `src/ndjson.rs / src/main.rs / src/agent.rs` | 部分实现：JS 特殊行分隔符转义、SDK 输出回环与 JSON 值保持；jsonStringify 的非标准对象语义待核对 |
| `src/cli/print.ts` | 5595 | 212735 | 重写：Headless 排队消息合并与会话执行 | `src/queued_prompt_batch.rs` / `src/sdk_queue.rs` / `src/main.rs`；SDK seed_read_state 接入共享会话已读缓存（9743d9bc），源码已提交待验收； | 部分实现：同 workload/meta 提示合并调用和去重计费、结构化 task_notification 系统事件、状态/用量解析；新增按 UUID 原始内容逐条回放（含批次合并与直接 SDK 消息），实际进程+HTTP 验证；原版全输出协议、完整消息确认/生命周期回调、远端 IO 和命令主流程仍未等价；SDK 原生取消待处理 UUID，取消后不会发送模型请求 ；SDK 文本+base64 图片混合块经 Agent 传递到多模型、SSE 与会话恢复；队列同 workload/meta 图文合并单次模型调用及各 UUID 回放已验证；多模态 UI/非图片块仍未等价；MCP Prompt 的 base64 图片内容经同一多模态通道真实送达模型，音频/资源等仍待验证；SDK initialize 已按真实插件/本地技能源文件返回描述和 argumentHint（48e34f9、2da7e57、ffd1056），仅源码提交待验收；排队命令处理、输出协议及取消时序仍未等价；SDK 非排队图文消息现在按原版识别末尾文本块斜杠命令，替换实际模型文本块并保留前置图片（64e2a9b），源码待验收；SDK side_question 独立模型调用已接线（21d3014、e165563），主模型忙碌期间 stdin 控制并发与中断仍缺；队列执行/取消仍缺 |
| `src/cli/remoteIO.ts` | 256 | 9946 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/structuredIO.ts` | 860 | 28720 | 重写：保持通用行为 | `src/main.rs / src/agent.rs / src/sse.rs` | 部分实现：NDJSON SDK 多轮、事件和实时文本增量；控制请求体系未全等；SDK 本地控制包络初始化/状态/取消回环，其他协议仍未全等 ；SDK 文本+base64 图片混合块经 Agent 传递到多模型、SSE 与会话恢复；多模态 UI/非图片块仍未等价|
| `src/cli/transports/HybridTransport.ts` | 283 | 10883 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/transports/SSETransport.ts` | 712 | 23758 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/transports/SerialBatchEventUploader.ts` | 276 | 9089 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/transports/WebSocketTransport.ts` | 801 | 28195 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/transports/WorkerStateUploader.ts` | 132 | 3879 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/transports/ccrClient.ts` | 999 | 33775 | 重写：保持通用行为 | `src/generated_ports/p_03ce73c80da6a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/cli/transports/transportUtils.ts` | 46 | 1767 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/cli/update.ts` | 423 | 14487 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/add-dir/add-dir.tsx` | 126 | 18017 | 重写：保持通用行为 | `src/workspace_dirs.rs / src/main.rs / src/agent.rs / src/tools.rs` | 部分实现：CLI 确认、目录校验、授权文件工具与设置保存；原版表单/沙箱/SDK 链仍缺 |
| `src/commands/add-dir/index.ts` | 12 | 260 | 重写：保持通用行为 | `src/workspace_dirs.rs / src/main.rs / src/agent.rs / src/tools.rs` | 部分实现：CLI 确认、目录校验、授权文件工具与设置保存；原版表单/沙箱/SDK 链仍缺 |
| `src/commands/add-dir/validation.ts` | 111 | 3207 | 重写：保持通用行为 | `src/workspace_dirs.rs / src/main.rs / src/agent.rs / src/tools.rs` | 部分实现：CLI 确认、目录校验、授权文件工具与设置保存；原版表单/沙箱/SDK 链仍缺 |
| `src/commands/advisor.ts` | 110 | 3182 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/agents/agents.tsx` | 12 | 2500 | 重写：保持通用行为 | `src/generated_ports/p_c014e56745958.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/agents/index.ts` | 11 | 232 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/ant-trace/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/autofix-pr/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/backfill-sessions/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/branch/branch.ts` | 297 | 9580 | 重写：保持通用行为 | `src/agent.rs / src/session.rs / src/main.rs / tests/integration_cli.rs` | 部分实现：/branch 已连接真实运行主流程；克隆当前完整会话消息与工具结果、生成新会话 UUID、原会话不变、来源跟踪、默认/自定义标题加 (Branch) 后缀及冲突递增，持久化后可独立恢复。新分支隔离旧会话 TodoWrite/兼容任务清单；原版逐消息 forkedFrom UUID/JSONL content-replacement、额外 transcript 元数据仍待等价 |
| `src/commands/branch/index.ts` | 15 | 445 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/break-cache/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/bridge/bridge.tsx` | 509 | 46907 | 重写：保持通用行为 | `src/generated_ports/p_c5c7a3a7a86d5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/bridge/index.ts` | 27 | 604 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/bridge-kick.ts` | 201 | 6703 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/brief.ts` | 131 | 5173 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/btw/btw.tsx` | 243 | 30250 | 重写：保持通用行为 | `src/generated_ports/p_45e0c7d37ad4a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/btw/index.ts` | 14 | 314 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/bughunter/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/chrome/chrome.tsx` | 285 | 31974 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/chrome/index.ts` | 14 | 381 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/clear/caches.ts` | 145 | 6370 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/clear/clear.ts` | 8 | 254 | 重写：保持通用行为 | `src/agent.rs / src/main.rs` | 部分实现：会话结束/开始 Hook、旧会话保留与新会话 ID、权限模式保留接入实际聊天和 SDK；完整缓存、MCP 热重置与全部后台任务生命周期待验收 |
| `src/commands/clear/conversation.ts` | 252 | 9325 | 重写：保持通用行为 | `src/agent.rs / src/main.rs` | 部分实现：会话结束/开始 Hook、旧会话保留与新会话 ID、权限模式保留接入实际聊天和 SDK；完整缓存、MCP 热重置与全部后台任务生命周期待验收 |
| `src/commands/clear/index.ts` | 20 | 603 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/color/color.ts` | 94 | 2712 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/color/index.ts` | 17 | 417 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/commit-push-pr.ts` | 159 | 6309 | 重写：保持通用行为 | `src/git_workflows.rs / src/main.rs` | 部分实现：真实默认分支检测、差异与中文 PR 工作流接入；推送和远端状态完整链待验收 |
| `src/commands/commit.ts` | 93 | 3492 | 重写：保持通用行为 | `src/git_workflows.rs / src/main.rs` | 部分实现：真实 Git 状态与差异生成提交提示，CLI/交互可执行；原版权限特化仍待验收 |
| `src/commands/compact/compact.ts` | 288 | 10079 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/compact/index.ts` | 16 | 530 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/config/config.tsx` | 7 | 1408 | 重写：保持通用行为 | `src/generated_ports/p_18a7a03aea84b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/config/index.ts` | 12 | 247 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/context/context-noninteractive.ts` | 326 | 10781 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/context/context.tsx` | 64 | 9099 | 重写：保持通用行为 | `src/generated_ports/p_3225eaf0db243.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/context/index.ts` | 25 | 695 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/copy/copy.tsx` | 371 | 42252 | 重写：保持通用行为 | `src/copy_response.rs` / `src/generated_ports/p_c886837faf980.rs` | 部分实现：真实 /copy 可达执行链补齐多文本块双换行、跳过纯工具调用、未闭合代码围栏提取、UTF-16 字符与末尾换行计数、终端剪贴板优先及文件不可写时回退；源码已提交待验收，原版完整 Markdown lexer 与交互选择器仍缺 |
| `src/commands/copy/index.ts` | 16 | 393 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/cost/cost.ts` | 25 | 909 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/cost/index.ts` | 24 | 668 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/createMovedToPluginCommand.ts` | 66 | 1789 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/ctx_viz/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/debug-tool-call/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/desktop/desktop.tsx` | 9 | 1645 | 重写：保持通用行为 | `src/generated_ports/p_55d1ffc73afa2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/desktop/index.ts` | 27 | 601 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/diff/diff.tsx` | 9 | 1461 | 重写：保持通用行为 | `src/generated_ports/p_8e039596a976f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/diff/index.ts` | 9 | 221 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/doctor/doctor.tsx` | 7 | 1309 | 重写：保持通用行为 | `src/diagnostics.rs / src/main.rs` | 部分实现：Rust Doctor 主入口已连接 CLI doctor 和交互 /doctor，原生离线检查版本、可执行路径、工作区、Git、配置、模型供应商、MCP可执行程序与上下文文件大小；原版 Doctor 多面板 UI、更新/版本锁/安装渠道等尚未等价 |
| `src/commands/doctor/index.ts` | 13 | 381 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/effort/effort.tsx` | 183 | 22128 | 重写：保持通用行为 | `src/generated_ports/p_942c3541355e4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/effort/index.ts` | 14 | 428 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/env/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/exit/exit.tsx` | 33 | 5272 | 重写：保持通用行为 | `src/generated_ports/p_8028611d439a8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/exit/index.ts` | 13 | 250 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/export/export.tsx` | 91 | 15104 | 重写：保持通用行为 | `src/main.rs / src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/commands/export/index.ts` | 12 | 303 | 重写：保持通用行为 | `src/main.rs / src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/commands/extra-usage/extra-usage-core.ts` | 119 | 4000 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/extra-usage/extra-usage-noninteractive.ts` | 17 | 466 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/extra-usage/extra-usage.tsx` | 17 | 3134 | 重写：保持通用行为 | `src/generated_ports/p_fa4cf9bf81ae4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/extra-usage/index.ts` | 32 | 1101 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/fast/fast.tsx` | 269 | 33931 | 重写：保持通用行为 | `src/generated_ports/p_beab21f3c4363.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/fast/index.ts` | 27 | 693 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/feedback/feedback.tsx` | 25 | 5095 | 重写：保持通用行为 | `src/generated_ports/p_db7aa9242d70e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/feedback/index.ts` | 27 | 931 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/files/files.ts` | 20 | 688 | 重写：保持通用行为 | `src/files_context.rs / src/agent.rs / src/main.rs` | 部分实现：/files 从真实工具结果重建读、写、编辑、NotebookEdit 的成功文件缓存，按最近使用排序、去重、保留最多 100 项，失败不入缓存，删除后的历史文件仍可跨会话恢复显示；原版独立 FileStateCache 的 25MB 字节淘汰、完整内容/mtime/offset/limit、编辑并发保护、压缩状态及实时失效仍待补齐 |
| `src/commands/files/index.ts` | 13 | 316 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/good-claude/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：原版禁用注册节点 Rust 化，保持不可调用并等待统一验收 |
| `src/commands/heapdump/heapdump.ts` | 18 | 398 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/heapdump/index.ts` | 13 | 288 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/help/help.tsx` | 11 | 1432 | 重写：保持通用行为 | `src/generated_ports/p_101b0ecfa9b9b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/help/index.ts` | 11 | 229 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/hooks/hooks.tsx` | 13 | 2679 | 重写：保持通用行为 | `src/generated_ports/p_a4ee4bb49da12.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/hooks/index.ts` | 12 | 260 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/ide/ide.tsx` | 646 | 77066 | 重写：保持通用行为 | `src/generated_ports/p_d57f9f8b75cd8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/ide/index.ts` | 12 | 258 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/init-verifiers.ts` | 263 | 10315 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/init.ts` | 257 | 20961 | 重写：保持通用行为 | `src/project_init.rs / src/main.rs / src/agent.rs / tests/integration_cli.rs` | 部分实现：已实现模型中立的 AGENTS.md 初始化主流程，project-init 与交互 /init 连接真实模型及用户问答；实际文件扫描、两组用户偏好、方案预览、写入审批及完成复查通过模型+工具执行；未等价原版特定 React 预览、实验开关与外部身份/Onboarding 状态 |
| `src/commands/insights.ts` | 3201 | 115949 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/ApiKeyStep.tsx` | 231 | 23518 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/CheckExistingSecretStep.tsx` | 190 | 18135 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/CheckGitHubStep.tsx` | 15 | 1248 | 重写：保持通用行为 | `src/generated_ports/p_26ba971b8d52d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/ChooseRepoStep.tsx` | 211 | 21279 | 重写：保持通用行为 | `src/generated_ports/p_7c4a2c598240c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/CreatingStep.tsx` | 65 | 9266 | 重写：保持通用行为 | `src/generated_ports/p_7a696c359ec3c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/ErrorStep.tsx` | 85 | 8602 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/ExistingWorkflowStep.tsx` | 103 | 9833 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/InstallAppStep.tsx` | 94 | 9601 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/OAuthFlowStep.tsx` | 276 | 39935 | 重写：保持通用行为 | `src/generated_ports/p_92dc7da033a9e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/SuccessStep.tsx` | 96 | 10170 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-github-app/WarningsStep.tsx` | 73 | 9132 | 重写：保持通用行为 | `src/generated_ports/p_c4344a216659f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/index.ts` | 14 | 471 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/install-github-app/install-github-app.tsx` | 587 | 87155 | 重写：保持通用行为 | `src/generated_ports/p_a9174729be5dd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/install-github-app/setupGitHubActions.ts` | 326 | 10168 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install-slack-app/index.ts` | 13 | 333 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/install-slack-app/install-slack-app.ts` | 31 | 877 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/install.tsx` | 300 | 39068 | 重写：保持通用行为 | `src/generated_ports/p_3289389e68d82.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/issue/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/keybindings/index.ts` | 14 | 448 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/keybindings/keybindings.ts` | 54 | 1645 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：CLI keybindings 子命令和聊天 /keybindings 实际创建、校验、查询与编辑器入口已落地 Rust；终端配置编辑体验待验收 |
| `src/commands/login/index.ts` | 15 | 489 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/commands/login/login.tsx` | 104 | 16109 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/commands/logout/index.ts` | 11 | 341 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/commands/logout/logout.tsx` | 82 | 10732 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/commands/mcp/addCommand.ts` | 281 | 9848 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/mcp/index.ts` | 13 | 280 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/mcp/mcp.tsx` | 85 | 12126 | 重写：保持通用行为 | `src/generated_ports/p_7522da9606aa2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/mcp/xaaIdpCommand.ts` | 267 | 10290 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/memory/index.ts` | 11 | 220 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/memory/memory.tsx` | 90 | 12608 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/mobile/index.ts` | 12 | 282 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/mobile/mobile.tsx` | 274 | 21797 | 重写：保持通用行为 | `src/generated_ports/p_ad72c0a43614a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/mock-limits/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/model/index.ts` | 17 | 559 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/model/model.tsx` | 297 | 37569 | 重写：保持通用行为 | `src/generated_ports/p_8f75dbf7e21af.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/oauth-refresh/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/onboarding/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/output-style/index.ts` | 12 | 291 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/output-style/output-style.tsx` | 7 | 1343 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/passes/index.ts` | 23 | 632 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/passes/passes.tsx` | 24 | 3783 | 重写：保持通用行为 | `src/generated_ports/p_31ab458ab85e0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/perf-issue/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/permissions/index.ts` | 12 | 296 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/permissions/permissions.tsx` | 10 | 2223 | 重写：保持通用行为 | `src/generated_ports/p_f0fb508cfc148.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plan/index.ts` | 12 | 286 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/plan/plan.tsx` | 122 | 13904 | 重写：保持通用行为 | `src/generated_ports/p_bcae150cd625f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/AddMarketplace.tsx` | 162 | 21821 | 重写：保持通用行为 | `src/generated_ports/p_100ae039b4f18.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/BrowseMarketplace.tsx` | 802 | 119562 | 重写：保持通用行为 | `src/generated_ports/p_aa9a2fff6266e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/DiscoverPlugins.tsx` | 781 | 106666 | 重写：保持通用行为 | `src/generated_ports/p_b2359c395e89f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/ManageMarketplaces.tsx` | 838 | 118912 | 重写：保持通用行为 | `src/generated_ports/p_7337a24f3e0b4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/ManagePlugins.tsx` | 2215 | 321775 | 重写：保持通用行为 | `src/generated_ports/p_9d478e1ba087e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/PluginErrors.tsx` | 124 | 23316 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/plugin/PluginOptionsDialog.tsx` | 357 | 34537 | 重写：保持通用行为 | `src/generated_ports/p_24eaf2e46f24a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/PluginOptionsFlow.tsx` | 135 | 18702 | 重写：保持通用行为 | `src/generated_ports/p_8dddb5ec693ee.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/PluginSettings.tsx` | 1072 | 128668 | 重写：保持通用行为 | `src/generated_ports/p_632353e8f57fc.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/PluginTrustWarning.tsx` | 32 | 3919 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/plugin/UnifiedInstalledCell.tsx` | 565 | 44441 | 重写：保持通用行为 | `src/generated_ports/p_48aef09fe12b8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/ValidatePlugin.tsx` | 98 | 12369 | 重写：保持通用行为 | `src/generated_ports/p_458eb28ef635b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/index.tsx` | 11 | 1337 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/plugin/parseArgs.ts` | 104 | 2818 | 重写：保持通用行为 | `src/plugin_commands.rs / src/plugin_manager.rs / src/main.rs` | 部分实现：原版菜单、别名、安装参数、市场动作与完整解析分支已实现；管理命令落地本地插件安装/删除/启停，远程市场和 UI 待补 |
| `src/commands/plugin/plugin.tsx` | 7 | 1590 | 重写：保持通用行为 | `src/generated_ports/p_75e7e879bf2e0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/pluginDetailsHelpers.tsx` | 117 | 12424 | 重写：保持通用行为 | `src/generated_ports/p_6c7ca7c1ed900.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/plugin/usePagination.ts` | 172 | 4990 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/pr_comments/index.ts` | 51 | 1848 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/privacy-settings/index.ts` | 15 | 399 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/privacy-settings/privacy-settings.tsx` | 58 | 10043 | 重写：保持通用行为 | `src/generated_ports/p_5d2e3ca07cba4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/rate-limit-options/index.ts` | 20 | 511 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/rate-limit-options/rate-limit-options.tsx` | 210 | 23749 | 重写：保持通用行为 | `src/generated_ports/p_f500b0cca6994.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/release-notes/index.ts` | 12 | 268 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/release-notes/release-notes.ts` | 51 | 1524 | 重写：保持通用行为 | `src/release_notes.rs / src/main.rs` | 部分实现：版本更新日志解析、500ms 下载、缓存及 CLI/交互调用已实现；自动通知仍缺 |
| `src/commands/reload-plugins/index.ts` | 19 | 653 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/reload-plugins/reload-plugins.ts` | 62 | 2598 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/remote-env/index.ts` | 16 | 577 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/remote-env/remote-env.tsx` | 7 | 1454 | 重写：保持通用行为 | `src/generated_ports/p_1d6be8358d910.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/remote-setup/api.ts` | 183 | 5519 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/remote-setup/index.ts` | 21 | 693 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/remote-setup/remote-setup.tsx` | 187 | 21786 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/rename/generateSessionName.ts` | 68 | 2297 | 重写：保持通用行为 | `src/agent.rs / src/session_control.rs` | 部分实现：通过当前选定模型生成 JSON kebab-case 名称、验证输入并持久保存、不污染历史会话且计入模型用量；近期上下文与多模态文本已按原版补齐；原厂专用模型与超时/中断策略按通用化要求调整 |
| `src/commands/rename/index.ts` | 13 | 281 | 重写：保持通用行为 | `src/main.rs / src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/commands/rename/rename.ts` | 88 | 2759 | 重写：保持通用行为 | `src/agent.rs / src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/commands/reset-limits/index.js` | 5 | 172 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/resume/index.ts` | 13 | 303 | 重写：保持通用行为 | `src/main.rs / src/session.rs / src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/commands/resume/resume.tsx` | 275 | 37026 | 重写：保持通用行为 | `src/session_control.rs / src/main.rs` | 部分实现：真实 UUID/标题搜索、歧义拒绝、原会话恢复、跨进程 CLI；原版 React 选择器、跨项目搜索和聊天即时状态迁移未等价 |
| `src/commands/review/UltrareviewOverageDialog.tsx` | 96 | 9655 | 重写：保持通用行为 | `src/generated_ports/p_8a0b520eb5d3b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/review/reviewRemote.ts` | 317 | 11926 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/review/ultrareviewCommand.tsx` | 58 | 9855 | 重写：保持通用行为 | `src/generated_ports/p_2a0eb439f5a02.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/review/ultrareviewEnabled.ts` | 15 | 526 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/review.ts` | 58 | 2188 | 重写：保持通用行为 | `src/git_workflows.rs / src/main.rs` | 部分实现：通过真实 Agent 执行 PR 审查任务；原版专属 Ultrareview 远程链仍待改造 |
| `src/commands/rewind/index.ts` | 14 | 337 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/rewind/rewind.ts` | 14 | 376 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/sandbox-toggle/index.ts` | 51 | 1520 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/sandbox-toggle/sandbox-toggle.tsx` | 83 | 13157 | 重写：保持通用行为 | `src/generated_ports/p_d71c68a412fbd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/security-review.ts` | 244 | 12531 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/session/index.ts` | 17 | 418 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/session/session.tsx` | 140 | 13009 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/share/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/skills/index.ts` | 11 | 226 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/skills/skills.tsx` | 8 | 1888 | 重写：保持通用行为 | `src/generated_ports/p_d54f470418286.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/stats/index.ts` | 11 | 252 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/stats/stats.tsx` | 7 | 1141 | 重写：保持通用行为 | `src/stats.rs / src/main.rs / tests/integration_cli.rs` | 部分实现：/stats 和独立 stats 命令已经真实读取本地持久会话，按供应商/模型汇总使用量和已知费用、缓存/搜索/工具调用及代码行统计；分支按保存的基线扣除继承历史，未知费率不猜。原版多标签页/日期热力图/趋势图与每消息时间戳尚未等价 |
| `src/commands/status/index.ts` | 13 | 322 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/status/status.tsx` | 8 | 1855 | 重写：保持通用行为 | `src/agent.rs / src/main.rs / tests/integration_cli.rs` | 部分实现：/status 已连入交互终端，展示真实会话标题/ID、工作区、模型/供应商、权限模式、MCP 服务及工具数，测试覆盖动态改名与不泄露凭据；原版 Settings 各页、IDE、完整安装/沙箱诊断 UI 仍未等价 |
| `src/commands/statusline.tsx` | 24 | 3568 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/stickers/index.ts` | 12 | 268 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/stickers/stickers.ts` | 17 | 476 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/summary/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/tag/index.ts` | 13 | 321 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/tag/tag.tsx` | 215 | 20999 | 重写：保持通用行为 | `src/session.rs / src/agent.rs / src/session_control.rs / src/main.rs` | 部分实现：/tag 终端主流程、Unicode 规范化、原标签切换确认、新标签替换、原子保存、列表带 #标签、按标签检索和跨进程恢复；原版 React Select/Dialog 等 UI 仍待等价 |
| `src/commands/tasks/index.ts` | 12 | 256 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/tasks/tasks.tsx` | 8 | 1901 | 重写：保持通用行为 | `src/generated_ports/p_65123b2aecfbb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/teleport/index.js` | 2 | 73 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/terminalSetup/index.ts` | 24 | 725 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/terminalSetup/terminalSetup.tsx` | 531 | 77528 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/theme/index.ts` | 11 | 217 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/theme/theme.tsx` | 57 | 5180 | 重写：保持通用行为 | `src/generated_ports/p_cb6d3fa44994c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/thinkback/index.ts` | 14 | 442 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/thinkback/thinkback.tsx` | 554 | 61993 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/thinkback-play/index.ts` | 18 | 608 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/thinkback-play/thinkback-play.ts` | 44 | 1430 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/ultraplan.tsx` | 471 | 66629 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/upgrade/index.ts` | 17 | 523 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/upgrade/upgrade.tsx` | 38 | 7438 | 重写：保持通用行为 | `src/generated_ports/p_ee2b485a76e48.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/usage/index.ts` | 10 | 233 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/usage/usage.tsx` | 7 | 1403 | 重写：保持通用行为 | `src/generated_ports/p_4c1965fccb05b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/commands/version.ts` | 23 | 577 | 重写：保持通用行为 | `src/version.rs / src/main.rs` | 部分实现：当前运行二进制版本、可选编译时间、交互 /version 和版本 CLI 已落地；专属用户开关按多模型要求移除，原调用链完整验收待做 |
| `src/commands/vim/index.ts` | 12 | 273 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/vim/vim.ts` | 39 | 1139 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/commands/voice/index.ts` | 21 | 482 | 重写：保持通用行为 | `src/command_manifest.rs` | 部分实现：Rust 指令注册描述/别名/隐藏/禁用索引和交互别名查询已落地，命令处理器独立迁移中 |
| `src/commands/voice/voice.ts` | 151 | 5264 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/components/AgentProgressLine.tsx` | 136 | 14238 | 重写：终端 UI／交互 | `src/generated_ports/p_6df1d1f49fc09.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/App.tsx` | 56 | 5204 | 重写：终端 UI／交互 | `src/generated_ports/p_e55bdaf86a903.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ApproveApiKey.tsx` | 123 | 10621 | 重写：终端 UI／交互 | `src/generated_ports/p_aacc3f689f27c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/AutoModeOptInDialog.tsx` | 142 | 13302 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/AutoUpdater.tsx` | 198 | 30788 | 重写：终端 UI／交互 | `src/generated_ports/p_33630f62d996d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/AutoUpdaterWrapper.tsx` | 91 | 11972 | 重写：终端 UI／交互 | `src/generated_ports/p_f96c082aa2c27.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/AwsAuthStatusBox.tsx` | 82 | 9707 | 重写：终端 UI／交互 | `src/generated_ports/p_dde39deb9414b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/BaseTextInput.tsx` | 136 | 19313 | 重写：终端 UI／交互 | `src/generated_ports/p_3949a8e5bc4d2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/BashModeProgress.tsx` | 56 | 5675 | 重写：终端 UI／交互 | `src/generated_ports/p_0b29474a0ccd4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/BridgeDialog.tsx` | 401 | 34418 | 重写：终端 UI／交互 | `src/generated_ports/p_21443fd0f496d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/BypassPermissionsModeDialog.tsx` | 87 | 9136 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ChannelDowngradeDialog.tsx` | 102 | 8253 | 重写：终端 UI／交互 | `src/generated_ports/p_e842880c14f05.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ClaudeCodeHint/PluginHintMenu.tsx` | 78 | 9341 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ClaudeInChromeOnboarding.tsx` | 121 | 12151 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ClaudeMdExternalIncludesDialog.tsx` | 137 | 13532 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ClickableImageRef.tsx` | 73 | 7495 | 重写：终端 UI／交互 | `src/generated_ports/p_25c626561a30d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CompactSummary.tsx` | 118 | 14437 | 重写：终端 UI／交互 | `src/generated_ports/p_4ef6869f716c2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ConfigurableShortcutHint.tsx` | 57 | 5436 | 重写：终端 UI／交互 | `src/generated_ports/p_b0d8db582e8fb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ConsoleOAuthFlow.tsx` | 631 | 79968 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ContextSuggestions.tsx` | 47 | 5952 | 重写：终端 UI／交互 | `src/generated_ports/p_68ce9b76726b6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ContextVisualization.tsx` | 489 | 76073 | 重写：终端 UI／交互 | `src/generated_ports/p_40acc6bc55581.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CoordinatorAgentStatus.tsx` | 273 | 36038 | 重写：终端 UI／交互 | `src/generated_ports/p_f1c50b7cfd309.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CostThresholdDialog.tsx` | 50 | 4336 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CtrlOToExpand.tsx` | 51 | 6062 | 重写：终端 UI／交互 | `src/generated_ports/p_7fa135c73cbe6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CustomSelect/SelectMulti.tsx` | 213 | 29884 | 重写：终端 UI／交互 | `src/generated_ports/p_1006dc067830f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CustomSelect/index.ts` | 4 | 118 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CustomSelect/option-map.ts` | 51 | 1189 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CustomSelect/select-input-option.tsx` | 488 | 58172 | 重写：终端 UI／交互 | `src/generated_ports/p_917c310662d72.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CustomSelect/select-option.tsx` | 68 | 5793 | 重写：终端 UI／交互 | `src/generated_ports/p_5937f7d9e3063.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CustomSelect/select.tsx` | 690 | 115187 | 重写：终端 UI／交互 | `src/generated_ports/p_0fde04c6a0116.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/CustomSelect/use-multi-select-state.ts` | 415 | 10952 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CustomSelect/use-select-input.ts` | 288 | 8770 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CustomSelect/use-select-navigation.ts` | 654 | 16388 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/CustomSelect/use-select-state.ts` | 158 | 2859 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/DesktopHandoff.tsx` | 193 | 19335 | 重写：终端 UI／交互 | `src/generated_ports/p_d3509722b2b3c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/DesktopUpsell/DesktopUpsellStartup.tsx` | 171 | 15520 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/DevBar.tsx` | 49 | 5040 | 重写：终端 UI／交互 | `src/generated_ports/p_0e01a1adf25b5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/DevChannelsDialog.tsx` | 105 | 9113 | 重写：终端 UI／交互 | `src/generated_ports/p_b476dbe9d3b70.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/DiagnosticsDisplay.tsx` | 95 | 13220 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/EffortCallout.tsx` | 265 | 24778 | 重写：终端 UI／交互 | `src/generated_ports/p_e8590503524b9.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/EffortIndicator.ts` | 43 | 1128 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ExitFlow.tsx` | 48 | 4364 | 重写：终端 UI／交互 | `src/generated_ports/p_96e0a274d18c4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ExportDialog.tsx` | 128 | 19333 | 重写：终端 UI／交互 | `src/generated_ports/p_26e43c0a828cf.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FallbackToolUseErrorMessage.tsx` | 116 | 12623 | 重写：终端 UI／交互 | `src/generated_ports/p_5b716827434fa.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FallbackToolUseRejectedMessage.tsx` | 16 | 1754 | 重写：终端 UI／交互 | `src/generated_ports/p_0660c482ad072.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FastIcon.tsx` | 46 | 4627 | 重写：终端 UI／交互 | `src/generated_ports/p_c6fbf2ff82c98.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Feedback.tsx` | 592 | 87696 | 重写：终端 UI／交互 | `src/generated_ports/p_58fbc53b1f78a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FeedbackSurvey/FeedbackSurvey.tsx` | 174 | 19393 | 重写：终端 UI／交互 | `src/generated_ports/p_cacc692b263f9.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FeedbackSurvey/FeedbackSurveyView.tsx` | 108 | 10660 | 重写：终端 UI／交互 | `src/generated_ports/p_31d30d3206690.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FeedbackSurvey/TranscriptSharePrompt.tsx` | 88 | 9933 | 重写：终端 UI／交互 | `src/generated_ports/p_d283d404b97e8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FeedbackSurvey/submitTranscriptShare.ts` | 113 | 3251 | 重写：终端 UI／交互 | `src/generated_ports/p_9274e52fe3673.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FeedbackSurvey/useDebouncedDigitInput.ts` | 83 | 2722 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/FeedbackSurvey/useFeedbackSurvey.tsx` | 296 | 48187 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/FeedbackSurvey/useMemorySurvey.tsx` | 213 | 30418 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/FeedbackSurvey/usePostCompactSurvey.tsx` | 206 | 24071 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/FeedbackSurvey/useSurveyState.tsx` | 100 | 14800 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/FileEditToolDiff.tsx` | 181 | 21900 | 重写：终端 UI／交互 | `src/generated_ports/p_74a5887cc8c7d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FileEditToolUpdatedMessage.tsx` | 124 | 12097 | 重写：终端 UI／交互 | `src/generated_ports/p_63b6a6c13e478.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FileEditToolUseRejectedMessage.tsx` | 170 | 15126 | 重写：终端 UI／交互 | `src/generated_ports/p_79aee3a3a9f2c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FilePathLink.tsx` | 43 | 3242 | 重写：终端 UI／交互 | `src/generated_ports/p_c042b904bc313.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/FullscreenLayout.tsx` | 637 | 84913 | 重写：终端 UI／交互 | `src/generated_ports/p_9ffbccf7a7015.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/GlobalSearchDialog.tsx` | 343 | 43997 | 重写：终端 UI／交互 | `src/generated_ports/p_57123442b4dd6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HelpV2/Commands.tsx` | 82 | 9738 | 重写：终端 UI／交互 | `src/generated_ports/p_febdb24a5a4d0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HelpV2/General.tsx` | 23 | 3101 | 重写：终端 UI／交互 | `src/generated_ports/p_c02d27a4b4b47.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HelpV2/HelpV2.tsx` | 184 | 20986 | 重写：终端 UI／交互 | `src/generated_ports/p_ad25a57c00fc4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HighlightedCode/Fallback.tsx` | 193 | 16263 | 重写：终端 UI／交互 | `src/generated_ports/p_ce20cba9c18c8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HighlightedCode.tsx` | 190 | 17579 | 重写：终端 UI／交互 | `src/generated_ports/p_a72db0598ed11.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/HistorySearchDialog.tsx` | 118 | 19923 | 重写：终端 UI／交互 | `src/generated_ports/p_808a4f8e2054f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/IdeAutoConnectDialog.tsx` | 154 | 13271 | 重写：终端 UI／交互 | `src/generated_ports/p_01742ba7da80d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/IdeOnboardingDialog.tsx` | 167 | 16399 | 重写：终端 UI／交互 | `src/generated_ports/p_403601f46d449.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/IdeStatusIndicator.tsx` | 58 | 6430 | 重写：终端 UI／交互 | `src/generated_ports/p_efeefe8aa34bc.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/IdleReturnDialog.tsx` | 118 | 9861 | 重写：终端 UI／交互 | `src/generated_ports/p_3ab35d9de2f94.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/InterruptedByUser.tsx` | 15 | 1962 | 重写：终端 UI／交互 | `src/generated_ports/p_67aa03093e173.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/InvalidConfigDialog.tsx` | 156 | 15252 | 重写：终端 UI／交互 | `src/generated_ports/p_c89223875c3fc.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/InvalidSettingsDialog.tsx` | 89 | 7063 | 重写：终端 UI／交互 | `src/generated_ports/p_7433840558392.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/KeybindingWarnings.tsx` | 55 | 9534 | 重写：终端 UI／交互 | `src/generated_ports/p_239f0550e25c5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LanguagePicker.tsx` | 86 | 8664 | 重写：终端 UI／交互 | `src/generated_ports/p_baa83b625cf4b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogSelector.tsx` | 1575 | 200487 | 重写：终端 UI／交互 | `src/generated_ports/p_6ed34733fbcc3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/AnimatedAsterisk.tsx` | 50 | 7562 | 重写：终端 UI／交互 | `src/generated_ports/p_0465d9076d4a5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/AnimatedClawd.tsx` | 124 | 14054 | 重写：终端 UI／交互 | `src/generated_ports/p_3efd291fe36da.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/ChannelsNotice.tsx` | 266 | 29503 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/LogoV2/Clawd.tsx` | 240 | 18574 | 重写：终端 UI／交互 | `src/generated_ports/p_086673d16adbb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/CondensedLogo.tsx` | 161 | 19395 | 重写：终端 UI／交互 | `src/generated_ports/p_912880da11cd9.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/EmergencyTip.tsx` | 58 | 6707 | 重写：终端 UI／交互 | `src/generated_ports/p_eb6ebe3e9e07d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/Feed.tsx` | 112 | 13804 | 重写：终端 UI／交互 | `src/generated_ports/p_146d6a79b5789.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/FeedColumn.tsx` | 59 | 5379 | 重写：终端 UI／交互 | `src/generated_ports/p_2e288793aaf3b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/GuestPassesUpsell.tsx` | 70 | 9122 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/LogoV2/LogoV2.tsx` | 543 | 75183 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/LogoV2/Opus1mMergeNotice.tsx` | 55 | 5812 | 重写：终端 UI／交互 | `src/generated_ports/p_92ccc82ba7039.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/OverageCreditUpsell.tsx` | 166 | 18347 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/LogoV2/VoiceModeNotice.tsx` | 68 | 7668 | 重写：终端 UI／交互 | `src/generated_ports/p_9eba1e482615d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/WelcomeV2.tsx` | 433 | 57864 | 重写：终端 UI／交互 | `src/generated_ports/p_ea10c11eb3bed.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/LogoV2/feedConfigs.tsx` | 92 | 12205 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/LspRecommendation/LspRecommendationMenu.tsx` | 88 | 10247 | 重写：终端 UI／交互 | `src/generated_ports/p_d3216b1df5fb4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MCPServerApprovalDialog.tsx` | 115 | 11563 | 重写：终端 UI／交互 | `src/generated_ports/p_28b59eec0efcf.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MCPServerDesktopImportDialog.tsx` | 203 | 21016 | 重写：终端 UI／交互 | `src/generated_ports/p_9d6b2f5def3de.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MCPServerDialogCopy.tsx` | 15 | 1829 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/MCPServerMultiselectDialog.tsx` | 133 | 16029 | 重写：终端 UI／交互 | `src/generated_ports/p_498bc5257103e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ManagedSettingsSecurityDialog/ManagedSettingsSecurityDialog.tsx` | 149 | 14484 | 重写：终端 UI／交互 | `src/generated_ports/p_b4669dd7c2988.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ManagedSettingsSecurityDialog/utils.ts` | 145 | 3979 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/Markdown.tsx` | 236 | 28160 | 重写：终端 UI／交互 | `src/generated_ports/p_67d9d2b82e9b7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MarkdownTable.tsx` | 322 | 47567 | 重写：终端 UI／交互 | `src/generated_ports/p_1f8d93c1c6b27.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MemoryUsageIndicator.tsx` | 37 | 4377 | 重写：终端 UI／交互 | `src/generated_ports/p_4501df2866732.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Message.tsx` | 627 | 79113 | 重写：终端 UI／交互 | `src/generated_ports/p_9f44ff177a927.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MessageModel.tsx` | 43 | 4110 | 重写：终端 UI／交互 | `src/generated_ports/p_0ae9a8bbc4878.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MessageResponse.tsx` | 78 | 6933 | 重写：终端 UI／交互 | `src/generated_ports/p_df8523c974296.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MessageRow.tsx` | 383 | 48342 | 重写：终端 UI／交互 | `src/generated_ports/p_b8e758d1581c8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MessageSelector.tsx` | 831 | 115618 | 重写：终端 UI／交互 | `src/generated_ports/p_62d3536bfc7ba.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/MessageTimestamp.tsx` | 63 | 5448 | 重写：终端 UI／交互 | `src/generated_ports/p_ce48428997d07.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Messages.tsx` | 834 | 147457 | 重写：终端 UI／交互 | `src/generated_ports/p_5c70f2e61245e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ModelPicker.tsx` | 448 | 54246 | 重写：终端 UI／交互 | `src/generated_ports/p_3553102a913de.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/NativeAutoUpdater.tsx` | 193 | 26514 | 重写：终端 UI／交互 | `src/generated_ports/p_6d672a8f7d5df.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/NotebookEditToolUseRejectedMessage.tsx` | 92 | 8447 | 重写：终端 UI／交互 | `src/generated_ports/p_969c9d9b904d0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/OffscreenFreeze.tsx` | 44 | 5664 | 重写：终端 UI／交互 | `src/generated_ports/p_6f565c2c13b92.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Onboarding.tsx` | 244 | 31528 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/OutputStylePicker.tsx` | 112 | 13233 | 重写：终端 UI／交互 | `src/generated_ports/p_f8adfbabecf36.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PackageManagerAutoUpdater.tsx` | 104 | 13907 | 重写：终端 UI／交互 | `src/generated_ports/p_3869eb9c14573.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Passes/Passes.tsx` | 184 | 27380 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PrBadge.tsx` | 97 | 7703 | 重写：终端 UI／交互 | `src/generated_ports/p_6da763d87b5ad.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PressEnterToContinue.tsx` | 15 | 1526 | 重写：终端 UI／交互 | `src/generated_ports/p_806f37fa5888a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/HistorySearchInput.tsx` | 51 | 5005 | 重写：终端 UI／交互 | `src/generated_ports/p_3ed42dc8aa879.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/IssueFlagBanner.tsx` | 12 | 1914 | 重写：终端 UI／交互 | `src/generated_ports/p_5ca89f5b33dd6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/Notifications.tsx` | 332 | 48104 | 重写：终端 UI／交互 | `src/generated_ports/p_4375034f382f7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInput.tsx` | 2339 | 355032 | 重写：终端 UI／交互 | `src/generated_ports/p_ee2a843014c61.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputFooter.tsx` | 191 | 33176 | 重写：终端 UI／交互 | `src/generated_ports/p_c39f8f63b92a4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputFooterLeftSide.tsx` | 517 | 87315 | 重写：终端 UI／交互 | `src/generated_ports/p_100d28512b001.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputFooterSuggestions.tsx` | 293 | 34158 | 重写：终端 UI／交互 | `src/generated_ports/p_a680bef86b91f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputHelpMenu.tsx` | 358 | 32811 | 重写：终端 UI／交互 | `src/generated_ports/p_ed2482259a075.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputModeIndicator.tsx` | 93 | 11127 | 重写：终端 UI／交互 | `src/generated_ports/p_4ae3f27ee4b20.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputQueuedCommands.tsx` | 117 | 19587 | 重写：终端 UI／交互 | `src/generated_ports/p_9b6293319d85b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/PromptInputStashNotice.tsx` | 25 | 2291 | 重写：终端 UI／交互 | `src/generated_ports/p_f6470f6cbfbdc.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/SandboxPromptFooterHint.tsx` | 64 | 8031 | 重写：终端 UI／交互 | `src/generated_ports/p_90f4b2e173492.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/ShimmeredInput.tsx` | 143 | 16680 | 重写：终端 UI／交互 | `src/generated_ports/p_2f84f9e35605c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/VoiceIndicator.tsx` | 137 | 10810 | 重写：终端 UI／交互 | `src/generated_ports/p_92d31b9e1cfc8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/PromptInput/inputModes.ts` | 34 | 731 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/components/PromptInput/inputPaste.ts` | 91 | 2693 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PromptInput/useMaybeTruncateInput.ts` | 59 | 1468 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PromptInput/usePromptInputPlaceholder.ts` | 77 | 2391 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PromptInput/useShowFastIconHint.ts` | 32 | 696 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PromptInput/useSwarmBanner.ts` | 156 | 5344 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/PromptInput/utils.ts` | 61 | 1744 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/QuickOpenDialog.tsx` | 244 | 28556 | 重写：终端 UI／交互 | `src/generated_ports/p_72b601fb5bd8f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/RemoteCallout.tsx` | 76 | 9978 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/RemoteEnvironmentDialog.tsx` | 340 | 33519 | 重写：终端 UI／交互 | `src/generated_ports/p_6945c8538facb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ResumeTask.tsx` | 268 | 38550 | 重写：终端 UI／交互 | `src/generated_ports/p_0b6fa0f37f0d7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/SandboxViolationExpandedView.tsx` | 99 | 11401 | 重写：终端 UI／交互 | `src/generated_ports/p_d899e6008e2c6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ScrollKeybindingHandler.tsx` | 1012 | 149202 | 重写：终端 UI／交互 | `src/generated_ports/p_875b44513014b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/SearchBox.tsx` | 72 | 9421 | 重写：终端 UI／交互 | `src/generated_ports/p_b99a90b93dfe8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/SentryErrorBoundary.ts` | 29 | 487 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/SessionBackgroundHint.tsx` | 108 | 13154 | 重写：终端 UI／交互 | `src/generated_ports/p_67610e3e6c4b3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/SessionPreview.tsx` | 194 | 19227 | 重写：终端 UI／交互 | `src/generated_ports/p_d5494fca23881.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Settings/Config.tsx` | 1822 | 271407 | 重写：终端 UI／交互 | `src/generated_ports/p_304f6b94b4985.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Settings/Settings.tsx` | 137 | 18641 | 重写：终端 UI／交互 | `src/generated_ports/p_cd5dd5eb5b5b8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Settings/Status.tsx` | 241 | 25871 | 重写：终端 UI／交互 | `src/generated_ports/p_cbf04f614d757.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Settings/Usage.tsx` | 377 | 39524 | 重写：终端 UI／交互 | `src/generated_ports/p_52c6c162180e4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ShowInIDEPrompt.tsx` | 170 | 17230 | 重写：终端 UI／交互 | `src/generated_ports/p_489fa54bdb60f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/SkillImprovementSurvey.tsx` | 152 | 15172 | 重写：终端 UI／交互 | `src/generated_ports/p_d7e7ec5a41e23.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/FlashingChar.tsx` | 61 | 6310 | 重写：终端 UI／交互 | `src/generated_ports/p_48b72004b31de.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/GlimmerMessage.tsx` | 328 | 26955 | 重写：终端 UI／交互 | `src/generated_ports/p_50bf5166734f7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/ShimmerChar.tsx` | 36 | 3357 | 重写：终端 UI／交互 | `src/generated_ports/p_ec706ec57f4b9.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/SpinnerAnimationRow.tsx` | 265 | 42767 | 重写：终端 UI／交互 | `src/generated_ports/p_210fb85258179.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/SpinnerGlyph.tsx` | 80 | 10291 | 重写：终端 UI／交互 | `src/generated_ports/p_3d04352c988c3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/TeammateSpinnerLine.tsx` | 233 | 38859 | 重写：终端 UI／交互 | `src/generated_ports/p_09647f4245ece.rs` | 部分实现：已完成独立队友状态行 Rust 动态渲染（状态、选择、工具用量、消息预览及宽度），通过源码 inspect/view 导出和状态树嵌套消费；已新增真实数据回归，终端生产入口、键盘事件及完整原版交互仍需验收 |
| `src/components/Spinner/TeammateSpinnerTree.tsx` | 272 | 28052 | 重写：终端 UI／交互 | `src/generated_ports/p_024038719ca35.rs` | 部分实现：队友运行状态筛选、排序和领导行已从实际 AppState 输入生成；已接通 TeammateSpinnerLine 动态渲染、导出 inspect/view 和组合回归；终端实时刷新、可交互键盘事件及完整调用链仍待验收 |
| `src/components/Spinner/index.ts` | 11 | 602 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/Spinner/teammateSelectHint.ts` | 2 | 64 | 重写：终端 UI／交互 | `src/generated_ports/p_9cdb888d4eefe.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner/useShimmerAnimation.ts` | 32 | 1236 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/Spinner/useStalledAnimation.ts` | 76 | 2500 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/Spinner/utils.ts` | 85 | 2261 | 重写：终端 UI／交互 | `src/generated_ports/p_43185acc13130.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Spinner.tsx` | 562 | 87940 | 重写：终端 UI／交互 | `src/generated_ports/p_752a76791bf16.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/Stats.tsx` | 1228 | 152782 | 重写：终端 UI／交互 | `src/stats.rs / src/main.rs` | 部分实现：已有可运行离线文本统计面板和分供应商用量视图；原版热力图、区间切换、键盘交互、ANSI 截图等终端 UI 尚未等价 |
| `src/components/StatusLine.tsx` | 324 | 49465 | 重写：终端 UI／交互 | `src/generated_ports/p_b297abdcbeb6f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/StatusNotices.tsx` | 55 | 5694 | 重写：终端 UI／交互 | `src/generated_ports/p_1f9cbcf671874.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/StructuredDiff/Fallback.tsx` | 487 | 56579 | 重写：终端 UI／交互 | `src/generated_ports/p_d32fceeb42c6c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/StructuredDiff/colorDiff.ts` | 38 | 1137 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/components/StructuredDiff.tsx` | 190 | 25007 | 重写：终端 UI／交互 | `src/generated_ports/p_5b170911b9f28.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/StructuredDiffList.tsx` | 30 | 4252 | 重写：终端 UI／交互 | `src/generated_ports/p_ea89f1ce49f9d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TagTabs.tsx` | 139 | 20853 | 重写：终端 UI／交互 | `src/generated_ports/p_fdf5a5900b02b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TaskListV2.tsx` | 378 | 50200 | 重写：终端 UI／交互 | `src/generated_ports/p_8f8e588ccdcf6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TeammateViewHeader.tsx` | 82 | 7415 | 重写：终端 UI／交互 | `src/generated_ports/p_00d240667df39.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TeleportError.tsx` | 189 | 18824 | 重写：终端 UI／交互 | `src/generated_ports/p_841962979051b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TeleportProgress.tsx` | 140 | 16142 | 重写：终端 UI／交互 | `src/generated_ports/p_af902ff34c817.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TeleportRepoMismatchDialog.tsx` | 104 | 13046 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/TeleportResumeWrapper.tsx` | 167 | 15381 | 重写：终端 UI／交互 | `src/generated_ports/p_145c13866144f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TeleportStash.tsx` | 116 | 15582 | 重写：终端 UI／交互 | `src/generated_ports/p_a3657ca61fd5e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TextInput.tsx` | 124 | 20998 | 重写：终端 UI／交互 | `src/generated_ports/p_d47fe9c9aae64.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ThemePicker.tsx` | 333 | 35662 | 重写：终端 UI／交互 | `src/generated_ports/p_b8623882ef24c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ThinkingToggle.tsx` | 153 | 18209 | 重写：终端 UI／交互 | `src/generated_ports/p_e827f2867375a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TokenWarning.tsx` | 179 | 21485 | 重写：终端 UI／交互 | `src/generated_ports/p_236973c85ed82.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/ToolUseLoader.tsx` | 42 | 4843 | 重写：终端 UI／交互 | `src/generated_ports/p_02c961fae3b16.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TrustDialog/TrustDialog.tsx` | 290 | 32481 | 重写：终端 UI／交互 | `src/generated_ports/p_0c75706230e07.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/TrustDialog/utils.ts` | 246 | 7005 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ValidationErrorsList.tsx` | 148 | 19567 | 重写：终端 UI／交互 | `src/generated_ports/p_5d92d9ccbc122.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/VimTextInput.tsx` | 140 | 16170 | 重写：终端 UI／交互 | `src/generated_ports/p_bfd0c9b4cdfac.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/VirtualMessageList.tsx` | 1082 | 148516 | 重写：终端 UI／交互 | `src/generated_ports/p_4e97f317c2f21.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/WorkflowMultiselectDialog.tsx` | 128 | 14276 | 重写：终端 UI／交互 | `src/generated_ports/p_11a1cccd83ccb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/WorktreeExitDialog.tsx` | 231 | 35484 | 重写：终端 UI／交互 | `src/generated_ports/p_1bacff6173180.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/AgentDetail.tsx` | 220 | 23566 | 重写：终端 UI／交互 | `src/generated_ports/p_0b420be1b193b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/AgentEditor.tsx` | 178 | 26437 | 重写：终端 UI／交互 | `src/generated_ports/p_31b29ef90def3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/AgentNavigationFooter.tsx` | 26 | 3067 | 重写：终端 UI／交互 | `src/generated_ports/p_880d83dc9c810.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/AgentsList.tsx` | 440 | 52257 | 重写：终端 UI／交互 | `src/generated_ports/p_65895b818e969.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/AgentsMenu.tsx` | 800 | 70586 | 重写：终端 UI／交互 | `src/generated_ports/p_584ab0ab3562d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/ColorPicker.tsx` | 112 | 14204 | 重写：终端 UI／交互 | `src/generated_ports/p_b91406e37fdf3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/ModelSelector.tsx` | 68 | 6927 | 重写：终端 UI／交互 | `src/generated_ports/p_2757935ec52a4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/ToolSelector.tsx` | 562 | 64871 | 重写：终端 UI／交互 | `src/generated_ports/p_4f1f5ef1057ac.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/agentFileUtils.ts` | 273 | 7487 | 重写：终端 UI／交互 | `src/agent_definition.rs / src/main.rs` | 部分实现：Markdown 元数据、转义、项目与用户配置文件、创建/覆盖/删除及同步落盘已接通；其他设置来源和 UI 生命周期待迁 |
| `src/components/agents/generateAgent.ts` | 198 | 10151 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/agents/new-agent-creation/CreateAgentWizard.tsx` | 97 | 10427 | 重写：终端 UI／交互 | `src/generated_ports/p_5df7355c6cbdd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/ColorStep.tsx` | 84 | 10430 | 重写：终端 UI／交互 | `src/generated_ports/p_ec56e2d844582.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/ConfirmStep.tsx` | 378 | 35034 | 重写：终端 UI／交互 | `src/generated_ports/p_317c3c95bd58f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/ConfirmStepWrapper.tsx` | 74 | 14450 | 重写：终端 UI／交互 | `src/generated_ports/p_686b4d7de99f9.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/DescriptionStep.tsx` | 123 | 14331 | 重写：终端 UI／交互 | `src/generated_ports/p_afafd1aeec08f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/GenerateStep.tsx` | 143 | 22113 | 重写：终端 UI／交互 | `src/generated_ports/p_cce865c493883.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/LocationStep.tsx` | 80 | 8280 | 重写：终端 UI／交互 | `src/generated_ports/p_639c60e23091f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/MemoryStep.tsx` | 113 | 14084 | 重写：终端 UI／交互 | `src/generated_ports/p_b53eb6713e37d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/MethodStep.tsx` | 80 | 8516 | 重写：终端 UI／交互 | `src/generated_ports/p_b9891a496c986.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/ModelStep.tsx` | 52 | 6321 | 重写：终端 UI／交互 | `src/generated_ports/p_70d76a64a3322.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/PromptStep.tsx` | 128 | 14831 | 重写：终端 UI／交互 | `src/generated_ports/p_6c70a25431244.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/ToolsStep.tsx` | 61 | 7220 | 重写：终端 UI／交互 | `src/generated_ports/p_d64688e260f00.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/new-agent-creation/wizard-steps/TypeStep.tsx` | 103 | 12474 | 重写：终端 UI／交互 | `src/generated_ports/p_9d5de99c90d12.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/agents/types.ts` | 28 | 915 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/agents/utils.ts` | 19 | 528 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/components/agents/validateAgent.ts` | 110 | 3150 | 重写：终端 UI／交互 | `src/agent_definition.rs / src/main.rs` | 部分实现：名称、描述、系统提示词、当前可用工具与跨来源重复校验已进入真实 CLI；描述/系统提示词长短判断已按原版 JavaScript UTF-16 字符单元计算，兼容中文和表情；工具权限规则按原版解析 Bash(git *)、Agent(类型) 等规则并映射厂商中立工具名；完整动态工具过滤和配置编辑 UI 尚未迁移 |
| `src/components/design-system/Byline.tsx` | 77 | 6493 | 重写：终端 UI／交互 | `src/generated_ports/p_d78b4b20e79ce.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/Dialog.tsx` | 138 | 14112 | 重写：终端 UI／交互 | `src/generated_ports/p_5bbc621df3eba.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/Divider.tsx` | 149 | 11094 | 重写：终端 UI／交互 | `src/generated_ports/p_24c4306aaf4fa.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/FuzzyPicker.tsx` | 312 | 40862 | 重写：终端 UI／交互 | `src/generated_ports/p_c78fd261d2dd8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/KeyboardShortcutHint.tsx` | 81 | 6847 | 重写：终端 UI／交互 | `src/generated_ports/p_c67bd922d31e8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/ListItem.tsx` | 244 | 19535 | 重写：终端 UI／交互 | `src/generated_ports/p_90f7490644747.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/LoadingState.tsx` | 94 | 6448 | 重写：终端 UI／交互 | `src/generated_ports/p_17e756c76b4c5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/Pane.tsx` | 77 | 6907 | 重写：终端 UI／交互 | `src/generated_ports/p_739e7c996602c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/ProgressBar.tsx` | 86 | 7170 | 重写：终端 UI／交互 | `src/generated_ports/p_a9eb5b2575d7c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/design-system/Ratchet.tsx` | 80 | 7166 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/StatusIcon.tsx` | 95 | 7568 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/Tabs.tsx` | 340 | 41447 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/ThemeProvider.tsx` | 170 | 18852 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/ThemedBox.tsx` | 156 | 18043 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/ThemedText.tsx` | 124 | 13877 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/design-system/color.ts` | 31 | 853 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/components/diff/DiffDetailView.tsx` | 281 | 22615 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/diff/DiffDialog.tsx` | 383 | 43407 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/diff/DiffFileList.tsx` | 292 | 25342 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/grove/Grove.tsx` | 463 | 49525 | 重写：终端 UI／交互 | `src/generated_ports/p_dad974e93dda6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/hooks/HooksConfigMenu.tsx` | 578 | 54469 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/hooks/PromptDialog.tsx` | 90 | 7482 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/hooks/SelectEventMode.tsx` | 127 | 13555 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/hooks/SelectHookMode.tsx` | 112 | 12907 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/hooks/SelectMatcherMode.tsx` | 144 | 14809 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/hooks/ViewHookMode.tsx` | 199 | 17969 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/CapabilitiesSection.tsx` | 61 | 4957 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/ElicitationDialog.tsx` | 1169 | 179643 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPAgentServerMenu.tsx` | 183 | 26662 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPListPanel.tsx` | 504 | 58231 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPReconnect.tsx` | 167 | 16195 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPRemoteServerMenu.tsx` | 649 | 102489 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPSettings.tsx` | 398 | 40386 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPStdioServerMenu.tsx` | 177 | 28394 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPToolDetailView.tsx` | 212 | 23139 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/MCPToolListView.tsx` | 141 | 16252 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/McpParsingWarnings.tsx` | 213 | 22079 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/index.ts` | 10 | 523 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/mcp/utils/reconnectHelpers.tsx` | 49 | 5292 | 重写：终端 UI／交互 | `src/generated_ports/p_962b63a53cf13.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/memory/MemoryFileSelector.tsx` | 438 | 48121 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/memory/MemoryUpdateNotification.tsx` | 45 | 5100 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messageActions.tsx` | 450 | 54948 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AdvisorMessage.tsx` | 158 | 14479 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AssistantRedactedThinkingMessage.tsx` | 31 | 2603 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AssistantTextMessage.tsx` | 270 | 30427 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AssistantThinkingMessage.tsx` | 86 | 8082 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AssistantToolUseMessage.tsx` | 368 | 45285 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/AttachmentMessage.tsx` | 536 | 71430 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/CollapsedReadSearchContent.tsx` | 484 | 78078 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/CompactBoundaryMessage.tsx` | 18 | 2275 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/GroupedToolUseContent.tsx` | 58 | 8289 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/HighlightedThinkingText.tsx` | 162 | 14902 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/HookProgressMessage.tsx` | 116 | 10613 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/PlanApprovalMessage.tsx` | 222 | 25339 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/RateLimitMessage.tsx` | 161 | 17162 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/ShutdownMessage.tsx` | 132 | 14493 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/SystemAPIErrorMessage.tsx` | 141 | 12285 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/SystemTextMessage.tsx` | 827 | 79395 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/TaskAssignmentMessage.tsx` | 76 | 8186 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserAgentNotificationMessage.tsx` | 83 | 6151 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserBashInputMessage.tsx` | 58 | 4503 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserBashOutputMessage.tsx` | 54 | 4188 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserChannelMessage.tsx` | 137 | 11325 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserCommandMessage.tsx` | 108 | 9210 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserImageMessage.tsx` | 59 | 5897 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserLocalCommandOutputMessage.tsx` | 167 | 14566 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserMemoryInputMessage.tsx` | 75 | 6516 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserPlanMessage.tsx` | 42 | 3743 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserPromptMessage.tsx` | 80 | 15172 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserResourceUpdateMessage.tsx` | 121 | 12363 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserTeammateMessage.tsx` | 206 | 24126 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserTextMessage.tsx` | 275 | 29051 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/RejectedPlanMessage.tsx` | 31 | 3409 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/RejectedToolUseMessage.tsx` | 16 | 1763 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/UserToolCanceledMessage.tsx` | 16 | 1777 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/UserToolErrorMessage.tsx` | 103 | 12222 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/UserToolRejectMessage.tsx` | 95 | 8570 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/UserToolResultMessage.tsx` | 106 | 13742 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/UserToolSuccessMessage.tsx` | 104 | 16409 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/UserToolResultMessage/utils.tsx` | 44 | 3938 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/nullRenderingAttachments.ts` | 71 | 2257 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/messages/teamMemCollapsed.tsx` | 140 | 13711 | 重写：终端 UI／交互 | `src/generated_ports/p_7ea337b79dc53.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/messages/teamMemSaved.ts` | 20 | 711 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/AskUserQuestionPermissionRequest.tsx` | 645 | 82469 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/PreviewBox.tsx` | 229 | 25959 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/PreviewQuestionView.tsx` | 328 | 52983 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/QuestionNavigationBar.tsx` | 178 | 22963 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/QuestionView.tsx` | 465 | 58579 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/SubmitQuestionsView.tsx` | 144 | 16641 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/AskUserQuestionPermissionRequest/use-multiple-choice-state.ts` | 180 | 4142 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/BashPermissionRequest/BashPermissionRequest.tsx` | 482 | 75744 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/BashPermissionRequest/bashToolUseOptions.tsx` | 147 | 21446 | 重写：终端 UI／交互 | `src/generated_ports/p_aa15b3a617dd6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/permissions/ComputerUseApproval/ComputerUseApproval.tsx` | 441 | 44745 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/EnterPlanModePermissionRequest/EnterPlanModePermissionRequest.tsx` | 122 | 13340 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/ExitPlanModePermissionRequest/ExitPlanModePermissionRequest.tsx` | 768 | 121607 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FallbackPermissionRequest.tsx` | 333 | 30677 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FileEditPermissionRequest/FileEditPermissionRequest.tsx` | 182 | 16257 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FilePermissionDialog/FilePermissionDialog.tsx` | 204 | 30317 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FilePermissionDialog/ideDiffConfig.ts` | 43 | 858 | 重写：终端 UI／交互 | `src/generated_ports/p_ffb259d974835.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/permissions/FilePermissionDialog/permissionOptions.tsx` | 177 | 22367 | 重写：终端 UI／交互 | `src/generated_ports/p_998a62fbd6f32.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/permissions/FilePermissionDialog/useFilePermissionDialog.ts` | 213 | 6809 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FilePermissionDialog/usePermissionHandler.ts` | 186 | 5105 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FileWritePermissionRequest/FileWritePermissionRequest.tsx` | 161 | 16526 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FileWritePermissionRequest/FileWriteToolDiff.tsx` | 89 | 9672 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/FilesystemPermissionRequest/FilesystemPermissionRequest.tsx` | 115 | 13085 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/NotebookEditPermissionRequest/NotebookEditPermissionRequest.tsx` | 166 | 16494 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/NotebookEditPermissionRequest/NotebookEditToolDiff.tsx` | 235 | 24680 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionDecisionDebugInfo.tsx` | 460 | 52533 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionDialog.tsx` | 72 | 7317 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionExplanation.tsx` | 272 | 23700 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionPrompt.tsx` | 336 | 37391 | 重写：终端 UI／交互 | `src/generated_ports/p_da009de33c5ba.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/permissions/PermissionRequest.tsx` | 217 | 33579 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionRequestTitle.tsx` | 66 | 5781 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PermissionRuleExplanation.tsx` | 121 | 14921 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PowerShellPermissionRequest/PowerShellPermissionRequest.tsx` | 235 | 38696 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/PowerShellPermissionRequest/powershellToolUseOptions.tsx` | 91 | 11950 | 重写：终端 UI／交互 | `src/generated_ports/p_ec381b97899e3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/permissions/SandboxPermissionRequest.tsx` | 163 | 14272 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/SedEditPermissionRequest/SedEditPermissionRequest.tsx` | 230 | 20862 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/SkillPermissionRequest/SkillPermissionRequest.tsx` | 369 | 36531 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/WebFetchPermissionRequest/WebFetchPermissionRequest.tsx` | 258 | 22888 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/WorkerBadge.tsx` | 49 | 3850 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/WorkerPendingPermission.tsx` | 105 | 9365 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/hooks.ts` | 210 | 8453 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/AddPermissionRules.tsx` | 180 | 22020 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/AddWorkspaceDirectory.tsx` | 340 | 37741 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/PermissionRuleDescription.tsx` | 76 | 6900 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/PermissionRuleInput.tsx` | 138 | 16427 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/PermissionRuleList.tsx` | 1179 | 118930 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/RecentDenialsTab.tsx` | 207 | 18857 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/RemoveWorkspaceDirectory.tsx` | 110 | 9757 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/rules/WorkspaceTab.tsx` | 150 | 15172 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/shellPermissionHelpers.tsx` | 164 | 22517 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/useShellPermissionFeedback.ts` | 149 | 4605 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/permissions/utils.ts` | 26 | 660 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/sandbox/SandboxConfigTab.tsx` | 45 | 16895 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/sandbox/SandboxDependenciesTab.tsx` | 120 | 17402 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/sandbox/SandboxDoctorSection.tsx` | 46 | 6190 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/sandbox/SandboxOverridesTab.tsx` | 193 | 20473 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/sandbox/SandboxSettings.tsx` | 296 | 29988 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/shell/ExpandShellOutputContext.tsx` | 36 | 3504 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/shell/OutputLine.tsx` | 118 | 14388 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/shell/ShellProgressMessage.tsx` | 150 | 14373 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/shell/ShellTimeDisplay.tsx` | 74 | 5229 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/skills/SkillsMenu.tsx` | 237 | 27508 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/AsyncAgentDetailDialog.tsx` | 229 | 29805 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/BackgroundTask.tsx` | 345 | 31248 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/BackgroundTaskStatus.tsx` | 429 | 43375 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/BackgroundTasksDialog.tsx` | 652 | 116395 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/DreamDetailDialog.tsx` | 251 | 25889 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/InProcessTeammateDetailDialog.tsx` | 266 | 31018 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/RemoteSessionDetailDialog.tsx` | 904 | 96185 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/RemoteSessionProgress.tsx` | 243 | 27859 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/ShellDetailDialog.tsx` | 404 | 39167 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/ShellProgress.tsx` | 87 | 7004 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/renderToolActivity.tsx` | 33 | 4414 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/tasks/taskStatusUtils.tsx` | 107 | 13548 | 重写：终端 UI／交互 | `src/generated_ports/p_75dd35b9b8459.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/components/teams/TeamStatus.tsx` | 80 | 6897 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/teams/TeamsDialog.tsx` | 715 | 94680 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ui/OrderedList.tsx` | 71 | 7294 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ui/OrderedListItem.tsx` | 45 | 3477 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/ui/TreeSelect.tsx` | 397 | 38964 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/wizard/WizardDialogLayout.tsx` | 65 | 6271 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/wizard/WizardNavigationFooter.tsx` | 24 | 4246 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/wizard/WizardProvider.tsx` | 213 | 18665 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/wizard/index.ts` | 10 | 328 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/components/wizard/useWizard.ts` | 14 | 447 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/constants/apiLimits.ts` | 95 | 3443 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/constants/betas.ts` | 53 | 2218 | 重写：保持通用行为 | `src/generated_ports/p_fa5438ef3e848.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/constants/common.ts` | 34 | 1516 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/constants/cyberRiskInstruction.ts` | 25 | 1525 | 重写：保持通用行为 | `src/generated_ports/p_96fa9b93b1ef7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/constants/errorIds.ts` | 16 | 476 | 重写：保持通用行为 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/constants/figures.ts` | 46 | 2063 | 重写：保持通用行为 | `src/generated_ports/p_7e4e3f10886b7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/constants/files.ts` | 157 | 2625 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/github-app.ts` | 145 | 5347 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/keys.ts` | 12 | 444 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/messages.ts` | 2 | 49 | 重写：保持通用行为 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/constants/oauth.ts` | 235 | 8970 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/outputStyles.ts` | 217 | 9886 | 重写：保持通用行为 | `src/prompt_runtime.rs / src/agent.rs / src/generated_ports/p_630eea798f5af.rs` | 部分实现：3 种内置输出风格及本地自定义 Markdown 风格已接入真实系统提示词；原版设置界面、完整格式与全局范围优先级未等价 |
| `src/constants/product.ts` | 77 | 2563 | 重写：保持通用行为 | `src/generated_ports/p_9ae508cd79a96.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/constants/prompts.ts` | 915 | 54320 | 重写：保持通用行为 | `src/prompt_runtime.rs / src/agent.rs / src/generated_ports/p_4573ed881a545.rs` | 部分实现：运行时主 system、来源边界与持续执行原则已实际发给模型；动态条件段落与原版完整指令仍待逐项对照，AST 常量仅供核查 |
| `src/constants/spinnerVerbs.ts` | 205 | 3453 | 重写：保持通用行为 | `src/generated_ports/p_7763c000d96fd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/constants/system.ts` | 96 | 3859 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/systemPromptSections.ts` | 69 | 1794 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/constants/toolLimits.ts` | 57 | 2169 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/constants/tools.ts` | 113 | 4690 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/constants/turnCompletionVerbs.ts` | 13 | 269 | 重写：保持通用行为 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/constants/xml.ts` | 87 | 3325 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/context/QueuedMessageContext.tsx` | 63 | 5598 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/fpsMetrics.tsx` | 30 | 3158 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/mailbox.tsx` | 38 | 3436 | 重写：保持通用行为 | `src/agent.rs / src/in_memory_mailbox.rs` | 部分实现：Agent 为会话持有独立内存邮箱，并通过 SDK 控制事件收发；React Provider/Context 生命周期与未挂载异常仍未等价 |
| `src/context/modalContext.tsx` | 58 | 6266 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/notifications.tsx` | 240 | 33050 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/overlayContext.tsx` | 151 | 14123 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/promptOverlayContext.tsx` | 125 | 12138 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/stats.tsx` | 220 | 22006 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/context/voice.tsx` | 88 | 8784 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/coordinator/coordinatorMode.ts` | 370 | 19020 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/entrypoints/agentSdkTypes.ts` | 444 | 13076 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/cli.tsx` | 303 | 39275 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/init.ts` | 341 | 13780 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/mcp.ts` | 197 | 6270 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/sandboxTypes.ts` | 157 | 5735 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/sdk/controlSchemas.ts` | 664 | 19554 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价；已接通 initialize（仅现有本地会话元数据）、mcp_status、cancel_async_message，原版其他控制类型仍未迁移；seed_read_state 控制请求已接通真实文件读取状态（9743d9bc），源码已提交待验收； |
| `src/entrypoints/sdk/coreSchemas.ts` | 1890 | 56493 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/entrypoints/sdk/coreTypes.ts` | 63 | 1466 | 重写：CLI／SDK 入口 | `src/main.rs` | 部分实现，多模式与 SDK 未等价 |
| `src/hooks/fileSuggestions.ts` | 812 | 27152 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useAutoModeUnavailableNotification.ts` | 57 | 1965 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useCanSwitchToExistingSubscription.tsx` | 60 | 7274 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useDeprecationWarningNotification.tsx` | 44 | 4605 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useFastModeNotification.tsx` | 162 | 14961 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useIDEStatusIndicator.tsx` | 186 | 20764 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useInstallMessages.tsx` | 26 | 3180 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useLspInitializationNotification.tsx` | 143 | 16569 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useMcpConnectivityStatus.tsx` | 88 | 14710 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useModelMigrationNotifications.tsx` | 52 | 6988 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useNpmDeprecationNotification.tsx` | 25 | 3602 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/usePluginAutoupdateNotification.tsx` | 83 | 9015 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/usePluginInstallationStatus.tsx` | 128 | 12012 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useRateLimitWarningNotification.tsx` | 114 | 12250 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useSettingsErrors.tsx` | 69 | 6756 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useStartupNotification.ts` | 42 | 1278 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/notifs/useTeammateShutdownNotification.ts` | 79 | 2296 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/renderPlaceholder.ts` | 52 | 1280 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/toolPermission/PermissionContext.ts` | 389 | 12768 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/toolPermission/handlers/coordinatorHandler.ts` | 66 | 2374 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/toolPermission/handlers/interactiveHandler.ts` | 537 | 20194 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/toolPermission/handlers/swarmWorkerHandler.ts` | 160 | 5537 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/toolPermission/permissionLogging.ts` | 239 | 7286 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/unifiedSuggestions.ts` | 203 | 5837 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useAfterFirstRender.ts` | 18 | 485 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useApiKeyVerification.ts` | 85 | 2678 | 重写：终端 UI／交互 | `src/generated_ports/p_f59e00a9dbb7d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useArrowKeyHistory.tsx` | 229 | 34135 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useAssistantHistory.ts` | 251 | 9228 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useAwaySummary.ts` | 126 | 3835 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useBackgroundTaskNavigation.ts` | 252 | 8553 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useBlink.ts` | 35 | 1279 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useCanUseTool.tsx` | 204 | 40206 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useCancelRequest.ts` | 277 | 10127 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useChromeExtensionNotification.tsx` | 50 | 6779 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useClaudeCodeHintRecommendation.tsx` | 129 | 15388 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useClipboardImageHint.ts` | 78 | 2458 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useCommandKeybindings.tsx` | 108 | 11023 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useCommandQueue.ts` | 16 | 543 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useCopyOnSelect.ts` | 99 | 4287 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useDeferredHookMessages.ts` | 47 | 1499 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useDiffData.ts` | 111 | 2839 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useDiffInIDE.ts` | 380 | 9867 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useDirectConnect.ts` | 230 | 7504 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useDoublePress.ts` | 63 | 1651 | 重写：终端 UI／交互 | `src/generated_ports/p_74bc565d88e8d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useDynamicConfig.ts` | 23 | 703 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useElapsedTime.ts` | 38 | 1226 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useExitOnCtrlCD.ts` | 96 | 3226 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useExitOnCtrlCDWithKeybindings.ts` | 25 | 948 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useFileHistorySnapshotInit.ts` | 26 | 767 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useGlobalKeybindings.tsx` | 249 | 31213 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useHistorySearch.ts` | 304 | 9488 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIDEIntegration.tsx` | 70 | 10517 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIdeAtMentioned.ts` | 77 | 2217 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIdeConnectionStatus.ts` | 34 | 981 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIdeLogging.ts` | 42 | 1201 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIdeSelection.ts` | 151 | 4349 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useInboxPoller.ts` | 970 | 34375 | 重写：终端 UI／交互 | `src/agent.rs / src/team_registry.rs / src/team_mailbox.rs` | 部分实现：队长自动审批计划并持久回执、成员验证来源/关联后退出计划模式；队长模式变更消息真实投递、成员授权校验、会话重启恢复；原版 React/UI 每秒轮询、团队配置模式同步、权限/沙箱请求分发和渲染时序未等价 |
| `src/hooks/useInputBuffer.ts` | 133 | 3386 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useIssueFlagBanner.ts` | 134 | 3828 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useLogMessages.ts` | 120 | 5710 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useLspPluginRecommendation.tsx` | 194 | 21588 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useMailboxBridge.ts` | 22 | 716 | 重写：终端 UI／交互 | `src/in_memory_mailbox.rs / src/main.rs` | 部分实现：空闲仅消费一条并提交、加载时不消费；React 外部 Store 与 effect 调度未等价 |
| `src/hooks/useMainLoopModel.ts` | 35 | 1509 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useManagePlugins.ts` | 305 | 11888 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useMemoryUsage.ts` | 40 | 1293 | 重写：终端 UI／交互 | `src/generated_ports/p_3ec18e2d10ecb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useMergedClients.ts` | 24 | 745 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useMergedCommands.ts` | 16 | 423 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useMergedTools.ts` | 45 | 1645 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useMinDisplayTime.ts` | 36 | 1010 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useNotifyAfterTimeout.ts` | 66 | 2471 | 重写：终端 UI／交互 | `src/generated_ports/p_a0553423b570f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useOfficialMarketplaceNotification.tsx` | 48 | 7299 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/usePasteHandler.ts` | 286 | 10146 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/usePluginRecommendationBase.tsx` | 105 | 11418 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/usePrStatus.ts` | 107 | 3202 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/usePromptSuggestion.ts` | 178 | 5315 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/usePromptsFromClaudeInChrome.tsx` | 71 | 11623 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useQueueProcessor.ts` | 69 | 2547 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useRemoteSession.ts` | 606 | 23010 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useReplBridge.tsx` | 723 | 115652 | 重写：终端 UI／交互 | `src/generated_ports/p_c84de04e0085c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useSSHSession.ts` | 242 | 8316 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useScheduledTasks.ts` | 140 | 5975 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSearchInput.ts` | 365 | 10327 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSessionBackgrounding.ts` | 159 | 4944 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSettings.ts` | 18 | 618 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSettingsChange.ts` | 26 | 946 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSkillImprovementSurvey.ts` | 106 | 3528 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSkillsChange.ts` | 63 | 2084 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSwarmInitialization.ts` | 82 | 3151 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useSwarmPermissionPoller.ts` | 331 | 9617 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTaskListWatcher.ts` | 222 | 6822 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTasksV2.ts` | 251 | 8808 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTeammateViewAutoExit.ts` | 64 | 2189 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTeleportResume.tsx` | 85 | 9840 | 重写：终端 UI／交互 | `src/generated_ports/p_17ee0f10fb33b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useTerminalSize.ts` | 16 | 354 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTextInput.ts` | 530 | 17027 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTimeout.ts` | 15 | 362 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTurnDiffs.ts` | 214 | 6686 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useTypeahead.tsx` | 1385 | 212610 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useUpdateNotification.ts` | 35 | 982 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useVimInput.ts` | 317 | 9741 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useVirtualScroll.ts` | 722 | 35122 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useVoice.ts` | 1145 | 45802 | 重写：终端 UI／交互 | `src/generated_ports/p_5a4cf6e30ce08.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/hooks/useVoiceEnabled.ts` | 26 | 1134 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/hooks/useVoiceIntegration.tsx` | 677 | 99464 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/Ansi.tsx` | 292 | 33264 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/bidi.ts` | 140 | 4290 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/clearTerminal.ts` | 75 | 1901 | 重写：终端 UI／交互 | `src/platform_source_utilities.rs / src/main.rs` | 部分实现：Win32 传统终端、Windows Terminal、VS Code ConPTY、Mintty 和类 Unix 清屏与 scrollback ANSI 序列已实现并接通 clear-screen CLI；原版全部调用链待完整移植 |
| `src/ink/colorize.ts` | 232 | 7647 | 重写：终端 UI／交互 | `src/generated_ports/p_36ccb582d72f4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/components/AlternateScreen.tsx` | 80 | 10377 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/App.tsx` | 658 | 98330 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/AppContext.ts` | 22 | 523 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Box.tsx` | 214 | 21652 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Button.tsx` | 192 | 16523 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/ClockContext.tsx` | 112 | 12091 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/CursorDeclarationContext.ts` | 33 | 1119 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/ErrorOverview.tsx` | 109 | 15405 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Link.tsx` | 42 | 3556 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Newline.tsx` | 39 | 2356 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/NoSelect.tsx` | 68 | 5899 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/RawAnsi.tsx` | 57 | 5157 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/ScrollBox.tsx` | 237 | 31814 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Spacer.tsx` | 20 | 1563 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/StdinContext.ts` | 50 | 1678 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/TerminalFocusContext.tsx` | 52 | 5955 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/TerminalSizeContext.tsx` | 7 | 983 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/components/Text.tsx` | 254 | 16811 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/constants.ts` | 3 | 107 | 重写：终端 UI／交互 | `src/ink_constants.rs` | 已验收：原始唯一帧间隔常量数值 16ms；证据 ink_constants::tests::original_render_frame_interval |
| `src/ink/dom.ts` | 485 | 15126 | 重写：终端 UI／交互 | `src/generated_ports/p_08d9d21613c39.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/events/click-event.ts` | 39 | 1332 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/events/dispatcher.ts` | 234 | 6004 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/events/emitter.ts` | 40 | 1125 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/events/event-handlers.ts` | 74 | 2202 | 重写：终端 UI／交互 | `src/generated_ports/p_9066c9e862a6f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/events/event.ts` | 12 | 250 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/events/focus-event.ts` | 22 | 687 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/events/input-event.ts` | 206 | 7306 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/events/keyboard-event.ts` | 52 | 1765 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust DOM 事件传播/键盘事件及 Unicode 宽度路径已编写，完整 TUI 与分割符号待验 |
| `src/ink/events/terminal-event.ts` | 108 | 2526 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust DOM 事件传播/键盘事件及 Unicode 宽度路径已编写，完整 TUI 与分割符号待验 |
| `src/ink/events/terminal-focus-event.ts` | 20 | 512 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/focus.ts` | 182 | 5142 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/frame.ts` | 125 | 4209 | 重写：终端 UI／交互 | `src/generated_ports/p_f1c981d6e64b5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/get-max-width.ts` | 28 | 1149 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/hit-test.ts` | 131 | 4228 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-animation-frame.ts` | 58 | 1933 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-app.ts` | 9 | 251 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-declared-cursor.ts` | 74 | 2996 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-input.ts` | 93 | 3107 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-interval.ts` | 68 | 1796 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-search-highlight.ts` | 54 | 2158 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-selection.ts` | 105 | 4421 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-stdin.ts` | 9 | 232 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/hooks/use-tab-status.ts` | 73 | 2175 | 重写：终端 UI／交互 | `src/generated_ports/p_7ec0d98f5f3f7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/hooks/use-terminal-focus.ts` | 17 | 556 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/hooks/use-terminal-title.ts` | 32 | 1020 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/hooks/use-terminal-viewport.ts` | 97 | 3977 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/ink.tsx` | 1723 | 251886 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/instances.ts` | 11 | 410 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/layout/engine.ts` | 7 | 177 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/layout/geometry.ts` | 98 | 2471 | 重写：终端 UI／交互 | `src/generated_ports/p_bab2fac86d685.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/layout/node.ts` | 153 | 4346 | 重写：终端 UI／交互 | `src/generated_ports/p_dbe6d69b9cec8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/layout/yoga.ts` | 309 | 7400 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/line-width-cache.ts` | 25 | 734 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/log-update.ts` | 774 | 27210 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/measure-element.ts` | 24 | 419 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/measure-text.ts` | 48 | 1138 | 重写：终端 UI／交互 | `src/portable_render_semantics.rs / src/main.rs` | 部分实现：原始单遍文本宽高测算、尾部空行、非正数/无穷宽度、逐行自动折行已实现；上游所有渲染调用链仍需继续接通 |
| `src/ink/node-cache.ts` | 55 | 1654 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/optimizer.ts` | 94 | 2588 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/output.ts` | 798 | 26183 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/parse-keypress.ts` | 802 | 23458 | 重写：终端 UI／交互 | `src/generated_ports/p_22f64b3a15765.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/reconciler.ts` | 513 | 14594 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/render-border.ts` | 232 | 6642 | 重写：终端 UI／交互 | `src/generated_ports/p_8fdccc85c5cbd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/render-node-to-output.ts` | 1463 | 63281 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/render-to-screen.ts` | 232 | 8570 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/renderer.ts` | 179 | 7665 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/root.ts` | 185 | 4600 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/screen.ts` | 1487 | 49323 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/searchHighlight.ts` | 94 | 3325 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/selection.ts` | 918 | 34933 | 重写：终端 UI／交互 | `src/generated_ports/p_9b038cbf2e98a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/squash-text-nodes.ts` | 93 | 2293 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/stringWidth.ts` | 223 | 7156 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust DOM 事件传播/键盘事件及 Unicode 宽度路径已编写，完整 TUI 与分割符号待验 |
| `src/ink/styles.ts` | 772 | 20889 | 重写：终端 UI／交互 | `src/generated_ports/p_7f9729523576c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/supports-hyperlinks.ts` | 58 | 1596 | 重写：终端 UI／交互 | `src/generated_ports/p_95ad2edddae92.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/tabstops.ts` | 47 | 1113 | 重写：终端 UI／交互 | `src/portable_render_semantics.rs / src/main.rs` | 部分实现：ANSI CSI/OSC/DCS 等控制序列不计宽度、八列制表符与换行重置已写入 Rust 并接入 ANSI SVG 渲染；上游所有渲染调用链仍需继续接通 |
| `src/ink/terminal-focus-state.ts` | 48 | 1305 | 重写：终端 UI／交互 | `src/generated_ports/p_22e7f7a29d693.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/terminal-querier.ts` | 213 | 7843 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/terminal.ts` | 249 | 8190 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/termio/ansi.ts` | 76 | 1490 | 重写：终端 UI／交互 | `src/generated_ports/p_1f1e68eb35633.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/termio/csi.ts` | 320 | 8677 | 重写：终端 UI／交互 | `src/generated_ports/p_bc5aed2cde71b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/termio/dec.ts` | 61 | 1935 | 重写：终端 UI／交互 | `src/generated_ports/p_5753cc2fee99d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/termio/esc.ts` | 68 | 1444 | 重写：终端 UI／交互 | `src/portable_render_semantics.rs / src/main.rs` | 部分实现：ESC 重置、保存/恢复光标、索引移动、换行、字符集选择及未知序列返回已完整实现；上游所有渲染调用链仍需继续接通 |
| `src/ink/termio/osc.ts` | 494 | 16842 | 重写：终端 UI／交互 | `src/generated_ports/p_95d00ece00cfd.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/termio/parser.ts` | 395 | 11546 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/termio/sgr.ts` | 309 | 6400 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/termio/tokenize.ts` | 320 | 9284 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/termio/types.ts` | 237 | 7076 | 重写：终端 UI／交互 | `src/generated_ports/p_506f1ba0cbb2f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/ink/termio.ts` | 43 | 1036 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/useTerminalNotification.ts` | 127 | 3857 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/warn.ts` | 10 | 295 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/widest-line.ts` | 20 | 434 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/ink/wrap-text.ts` | 75 | 1806 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/ink/wrapAnsi.ts` | 21 | 383 | 重写：终端 UI／交互 | `src/portable_ink_core.rs` | 部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| `src/keybindings/KeybindingContext.tsx` | 243 | 26000 | 重写：保持通用行为 | `src/keybindings.rs` | 部分实现：上下文激活/卸载计数、动作回调注册与清理、显示绑定及组合键共享状态，React UI 接线待迁 |
| `src/keybindings/KeybindingProviderSetup.tsx` | 308 | 41452 | 重写：保持通用行为 | `src/keybindings.rs` | 部分实现：组合键拦截、1 秒限时、回调分发、状态重置、配置重载入口与警告状态；终端事件循环及实际文件监听仍缺 |
| `src/keybindings/defaultBindings.ts` | 341 | 11635 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：默认快捷键配置、跨平台差异及核心上下文已迁入原生 Rust；原版全功能开关与各 UI 动作仍未连接 |
| `src/keybindings/loadUserBindings.ts` | 473 | 14551 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：用户 JSON 快捷键热读取、默认与自定义优先级、错误降级和本地路径已迁入 Rust；原版文件监听通知机制仍缺 |
| `src/keybindings/match.ts` | 121 | 3797 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：Ink 等价终端键名归一化、修饰键和 Escape 特例已迁入 Rust；原生 UI 输入派发仍待连接 |
| `src/keybindings/parser.ts` | 204 | 4972 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：组合键解析、修饰键别名、平台显示以及 bindings 映射已迁入 Rust；复杂终端输入仍需交叉验收 |
| `src/keybindings/reservedShortcuts.ts` | 128 | 3610 | 重写：保持通用行为 | `src/generated_ports/p_d338e19c29574.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/keybindings/resolver.ts` | 245 | 7087 | 重写：保持通用行为 | `src/generated_ports/p_c7d9aa37cc1b1.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/keybindings/schema.ts` | 237 | 6269 | 重写：保持通用行为 | `src/generated_ports/p_212ef30d7fd2c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/keybindings/shortcutFormat.ts` | 64 | 2575 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：可用动作的用户定义快捷键显示已迁入 Rust；缺失快捷键后的原版遥测按用户要求删除 |
| `src/keybindings/template.ts` | 53 | 1721 | 重写：保持通用行为 | `src/keybindings.rs / src/main.rs` | 部分实现：用户配置模板及保留快捷键过滤、原子排他创建已迁入 Rust；文本编辑器差异待覆盖 |
| `src/keybindings/useKeybinding.ts` | 197 | 6862 | 重写：保持通用行为 | `src/keybindings.rs` | 部分实现：单动作/多动作回调、主动上下文、拒绝传播与执行后解绑；React Ink useInput 实际事件链尚未等价 |
| `src/keybindings/useShortcutDisplay.ts` | 60 | 2510 | 重写：保持通用行为 | `src/keybindings.rs` | 部分实现：按动作/上下文查当前按键显示并随运行时配置重载；UI Hook 更新时序待迁 |
| `src/keybindings/validate.ts` | 499 | 13667 | 重写：保持通用行为 | `src/generated_ports/p_f8e473fd2917d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/memdir/findRelevantMemories.ts` | 142 | 5305 | 重写：压缩与记忆 | `src/memory.rs` | 部分实现：选用当前配置模型检索最多 5 条相关 Markdown 记忆；原版工具上下文和异常处理仍待核对 |
| `src/memdir/memdir.ts` | 508 | 21174 | 重写：压缩与记忆 | `src/memory.rs` | 部分实现：MEMORY.md 200 行/25 KB 索引及截断提示；路径和团队记忆未等价 |
| `src/memdir/memoryAge.ts` | 54 | 1931 | 重写：压缩与记忆 | `src/memory_age.rs` | 已验收：全部 4 个导出函数的日期边界、未来时间、特殊数值与原始提示文本对照；证据 memory_age::tests |
| `src/memdir/memoryScan.ts` | 95 | 3105 | 重写：压缩与记忆 | `src/memory.rs` | 部分实现：Markdown frontmatter、mtime 排序、最多 200 项；原版所有文件边界未全等 |
| `src/memdir/memoryTypes.ts` | 272 | 22866 | 重写：压缩与记忆 | `src/memory.rs` | 部分实现：基础记忆分类读取；保存约束及类型提示词未全等 |
| `src/memdir/paths.ts` | 279 | 10668 | 重写：压缩与记忆 | `src/memory.rs` | 部分实现：工作区记忆目录隔离；跨项目路径算法未等价 |
| `src/memdir/teamMemPaths.ts` | 293 | 11691 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/memdir/teamMemPrompts.ts` | 101 | 5998 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/migrations/migrateAutoUpdatesToSettings.ts` | 62 | 1953 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateBypassPermissionsAcceptedToSettings.ts` | 41 | 1262 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateEnableAllProjectMcpServersToSettings.ts` | 119 | 3977 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateFennecToOpus.ts` | 46 | 1372 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateLegacyOpusToCurrent.ts` | 58 | 1974 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateOpusToOpus1m.ts` | 44 | 1347 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateReplBridgeEnabledToRemoteControlAtStartup.ts` | 23 | 1000 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateSonnet1mToSonnet45.ts` | 49 | 1582 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/migrateSonnet45ToSonnet46.ts` | 68 | 2055 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/resetAutoModeOptInForDefaultOffer.ts` | 52 | 2106 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/migrations/resetProToOpusDefault.ts` | 52 | 1550 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/moreright/useMoreRight.tsx` | 26 | 3535 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/native-ts/color-diff/index.ts` | 1000 | 30042 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/native-ts/file-index/index.ts` | 371 | 12006 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/native-ts/yoga-layout/enums.ts` | 135 | 2823 | 重写：保持通用行为 | `src/generated_ports/p_166b646bc508b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/native-ts/yoga-layout/index.ts` | 2579 | 83377 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/outputStyles/loadOutputStylesDir.ts` | 99 | 3438 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/plugins/builtinPlugins.ts` | 160 | 4980 | 重写：保持通用行为 | `src/generated_ports/p_2f9a1e26806ee.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/plugins/bundled/index.ts` | 24 | 843 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/query/config.ts` | 47 | 1808 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/query/deps.ts` | 41 | 1445 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/query/stopHooks.ts` | 474 | 17290 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移；队友完成时在办任务 TaskCompleted→TeammateIdle 原版顺序及阻断继续已接入真实团队执行链（dd67ffc1、984410a6），待验收 |
| `src/query/tokenBudget.ts` | 94 | 2320 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/remote/RemoteSessionManager.ts` | 344 | 9320 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/remote/SessionsWebSocket.ts` | 405 | 12505 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/remote/remotePermissionBridge.ts` | 79 | 2378 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/remote/sdkMessageAdapter.ts` | 303 | 9060 | 改造：自建远程连接协议 | 待定 | 未迁移，禁止原厂端点与认证依赖 |
| `src/schemas/hooks.ts` | 223 | 7884 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/screens/Doctor.tsx` | 575 | 73277 | 重写：终端 UI／交互 | `src/diagnostics.rs / src/main.rs / tests/integration_cli.rs` | 部分实现：原版 Doctor 诊断中可移植部分落地为实际可运行命令，支持真实 MCP 状态/上下文大小/配置错误报告；新增 Bash/后台任务输出长度环境变量的非法值与上限截断诊断，以及 40,000 UTF-16 字符超大指令文件警告，源码已提交待行为验收；修复 PATH 中不可执行文件被误判为可用程序；仍缺原版 React/Ink UI、自动更新、版本锁与完整环境安装诊断 |
| `src/screens/REPL.tsx` | 5006 | 895850 | 重写：终端 UI／交互 | `src/generated_ports/p_fe5e0a874b3c2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/screens/ResumeConversation.tsx` | 399 | 59682 | 重写：终端 UI／交互 | 待定 | 未迁移 |
| `src/server/createDirectConnectSession.ts` | 89 | 2193 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/server/directConnectManager.ts` | 214 | 5898 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/server/types.ts` | 58 | 1466 | 重写：保持通用行为 | `src/generated_ports/p_0409a449c853a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/services/AgentSummary/agentSummary.ts` | 180 | 6407 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/MagicDocs/magicDocs.ts` | 255 | 7683 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/MagicDocs/prompts.ts` | 128 | 5595 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/PromptSuggestion/promptSuggestion.ts` | 524 | 17065 | 重写：保持通用行为 | `src/generated_ports/p_ac92628c81197.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/services/PromptSuggestion/speculation.ts` | 992 | 30680 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/SessionMemory/prompts.ts` | 325 | 12629 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/SessionMemory/sessionMemory.ts` | 496 | 16561 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/SessionMemory/sessionMemoryUtils.ts` | 208 | 6110 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/analytics/config.ts` | 39 | 1237 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/datadog.ts` | 308 | 9101 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/firstPartyEventLogger.ts` | 450 | 14590 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/firstPartyEventLoggingExporter.ts` | 807 | 26362 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/growthbook.ts` | 1156 | 40526 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/index.ts` | 174 | 5542 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/metadata.ts` | 974 | 32617 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/sink.ts` | 115 | 3542 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/sinkKillswitch.ts` | 26 | 1063 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/api/adminRequests.ts` | 120 | 3208 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/bootstrap.ts` | 142 | 4634 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/claude.ts` | 3420 | 125779 | 替换：独立模型／访问管理 | `src/provider.rs / src/sse.rs` | 部分实现：多提供商、SSE 增量及 Token 归一化；原版 API 流程未全等 |
| `src/services/api/client.ts` | 390 | 16164 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/dumpPrompts.ts` | 227 | 7332 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/emptyUsage.ts` | 23 | 712 | 重写：模型／Agent 循环 | `src/runtime_contracts.rs` | 已验收：零用量结构字段与嵌套默认值的完整静态对照；证据 runtime_contracts::tests::empty_usage_has_the_entire_original_schema |
| `src/services/api/errorUtils.ts` | 261 | 8405 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/errors.ts` | 1208 | 41735 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/filesApi.ts` | 749 | 21494 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/firstTokenDate.ts` | 61 | 1765 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/grove.ts` | 358 | 11543 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/logging.ts` | 789 | 24191 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/metricsOptOut.ts` | 160 | 5355 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/overageCreditGrant.ts` | 138 | 4913 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/promptCacheBreakDetection.ts` | 728 | 26288 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/referral.ts` | 282 | 7985 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/sessionIngress.ts` | 515 | 17055 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/ultrareviewQuota.ts` | 39 | 1219 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/usage.ts` | 64 | 1685 | 重写：模型／Agent 循环 | `src/provider.rs / src/agent.rs` | 部分实现，回退与流式行为未等价 |
| `src/services/api/withRetry.ts` | 823 | 28238 | 重写：模型／Agent 循环 | `src/provider.rs / src/sse.rs / src/gemini_sse.rs / src/agent.rs` | 部分实现：多模型 HTTP/流式建立前的 408/409/429/5xx 与连接超时有限重试，尊重 x-should-retry:false、Retry-After 秒数，失败不执行工具或重复落盘；仍缺原版可取消等待、按查询来源的 529 策略、流中断恢复、模型回退及完整重试事件与自定义预算 |
| `src/services/autoDream/autoDream.ts` | 325 | 11259 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/autoDream/config.ts` | 22 | 892 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/autoDream/consolidationLock.ts` | 141 | 4548 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/autoDream/consolidationPrompt.ts` | 66 | 3225 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/awaySummary.ts` | 75 | 2671 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/claudeAiLimits.ts` | 516 | 16803 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/services/claudeAiLimitsHook.ts` | 24 | 515 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/services/compact/apiMicrocompact.ts` | 154 | 4996 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/autoCompact.ts` | 352 | 12905 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/compact.ts` | 1706 | 60814 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现：先持久化压缩摘要再执行 PostCompact，Hook 失败不回滚摘要（1b652b8b）；长期记忆、完整边界标记及恢复时序未等价 |
| `src/services/compact/compactWarningHook.ts` | 17 | 568 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/compactWarningState.ts` | 19 | 693 | 重写：压缩与记忆 | `src/runtime_contracts.rs / src/agent.rs` | 部分实现：默认 false、压缩抑制、清除与重复赋值不通知已验证；证据 runtime_contracts::tests::compact_warning_state_and_subscriptions；原始 createStore 的同步 Set 订阅、派发时增删监听及跨 Agent/UI 调用时序尚未等价 |
| `src/services/compact/grouping.ts` | 64 | 2794 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/microCompact.ts` | 531 | 19544 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/postCompactCleanup.ts` | 78 | 3778 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/prompt.ts` | 375 | 16278 | 重写：压缩与记忆 | `src/prompt_runtime.rs / src/agent.rs / src/session.rs` | 部分实现：用户约束及真实证据保留摘要模板已接入工具关闭的实际模型请求；原版部分压缩和历史分组仍未完全等价 |
| `src/services/compact/sessionMemoryCompact.ts` | 631 | 21058 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/compact/timeBasedMCConfig.ts` | 44 | 1766 | 重写：压缩与记忆 | `src/agent.rs / src/session.rs` | 部分实现，长期记忆未等价 |
| `src/services/diagnosticTracking.ts` | 398 | 12315 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/services/extractMemories/extractMemories.ts` | 616 | 21684 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/extractMemories/prompts.ts` | 155 | 7673 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/internalLogging.ts` | 91 | 2799 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/lsp/LSPClient.ts` | 448 | 14361 | 重写：保持通用行为 | `src/lsp.rs` | 部分实现：JSON-RPC Content-Length、文档同步及查询测试通过 |
| `src/services/lsp/LSPDiagnosticRegistry.ts` | 387 | 11957 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/lsp/LSPServerInstance.ts` | 512 | 16864 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/lsp/LSPServerManager.ts` | 421 | 13394 | 重写：保持通用行为 | `src/lsp.rs` | 部分实现：按扩展名选择服务；进程恢复及状态管理待补 |
| `src/services/lsp/config.ts` | 80 | 2857 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/lsp/manager.ts` | 290 | 10067 | 重写：保持通用行为 | `src/lsp.rs` | 部分实现：本地客户端管理；原版完整生命周期未等价 |
| `src/services/lsp/passiveFeedback.ts` | 329 | 11190 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/mcp/InProcessTransport.ts` | 64 | 1772 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/MCPConnectionManager.tsx` | 73 | 8186 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/SdkControlTransport.ts` | 137 | 4503 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/auth.ts` | 2466 | 88879 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/channelAllowlist.ts` | 77 | 2838 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/channelNotification.ts` | 317 | 12540 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/channelPermissions.ts` | 241 | 8981 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/claudeai.ts` | 165 | 6126 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/client.ts` | 3349 | 119060 | 重写：多模型通用 MCP | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现：stdio/HTTP/SSE、故障隔离、资源/提示订阅及重连、MCP Prompt 命令发现/位置参数绑定/服务端取回/进入真实模型请求；多模态 Prompt、OAuth、独立 SSE GET/退避、其他传输及完整回调/UI 仍未等价；新增服务端 roots/list、ping、elicitation 初始化安全取消与 HTTP SSE 双向请求回执，交互式审批仍缺 ；MCP Prompt 的 base64 图片内容经同一多模态通道真实送达模型，音频/资源等仍待验证；roots/list 按实际工作区或子代理 worktree 返回，断线重建保留原根目录，并校验 stdio/HTTP 双传输 ；独立 GET 通知流增加 SDK 显式长轮询/Last-Event-ID 续传及资源变更监听，常驻自动监听未实现 |
| `src/services/mcp/config.ts` | 1579 | 51130 | 重写：多模型通用 MCP | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现：stdio/HTTP/SSE、故障隔离、资源/提示订阅及重连、MCP Prompt 命令发现/位置参数绑定/服务端取回/进入真实模型请求；多模态 Prompt、OAuth、独立 SSE GET/退避、其他传输及完整回调/UI 仍未等价 |
| `src/services/mcp/elicitationHandler.ts` | 314 | 10166 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/envExpansion.ts` | 39 | 1047 | 重写：多模型通用 MCP | `src/compat_helpers.rs` | 已验收：环境变量展开、默认值、缺失列表及重复缺失项的纯函数语义对照；证据 compat_helpers::tests::source_env_expansion_cases |
| `src/services/mcp/headersHelper.ts` | 139 | 4718 | 重写：多模型通用 MCP | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现：stdio/HTTP/SSE、故障隔离、资源/提示订阅及重连、MCP Prompt 命令发现/位置参数绑定/服务端取回/进入真实模型请求；多模态 Prompt、OAuth、独立 SSE GET/退避、其他传输及完整回调/UI 仍未等价；动态请求头助手已接入 HTTP 握手、10 秒超时、JSON 校验、静态头回退及覆盖，尚未行为测试，项目/本地信任门禁和其他传输仍缺 |
| `src/services/mcp/mcpStringUtils.ts` | 107 | 3968 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现：已按 mcp__server__tool 实现命名/路由，并在权限匹配中补 server-wide / wildcard 规则；字符串解析全部函数及全链路仍待逐项验收 |
| `src/services/mcp/normalization.ts` | 24 | 879 | 重写：多模型通用 MCP | `src/runtime_contracts.rs / src/mcp.rs` | 部分实现：已迁移通用 ASCII 名称归一化；原厂服务名前缀专有规则按要求移除 |
| `src/services/mcp/oauthPort.ts` | 79 | 2325 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/officialRegistry.ts` | 73 | 2013 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/types.ts` | 259 | 6962 | 重写：多模型通用 MCP | `src/mcp.rs / src/mcp_http.rs / src/config.rs` | 部分实现：stdio/HTTP/SSE、故障隔离、资源/提示订阅及重连、MCP Prompt 命令发现/位置参数绑定/服务端取回/进入真实模型请求；多模态 Prompt、OAuth、独立 SSE GET/退避、其他传输及完整回调/UI 仍未等价 |
| `src/services/mcp/useManageMCPConnections.ts` | 1142 | 44866 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/utils.ts` | 576 | 17931 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/vscodeSdkMcp.ts` | 113 | 3703 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/xaa.ts` | 512 | 18286 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcp/xaaIdpLogin.ts` | 488 | 16271 | 重写：多模型通用 MCP | `src/mcp.rs` | 部分实现，仅基础 stdio |
| `src/services/mcpServerApproval.tsx` | 41 | 6400 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/mockRateLimits.ts` | 883 | 29698 | 重写：保持通用行为 | `src/generated_ports/p_0f036bc7045d6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/services/notifier.ts` | 157 | 4256 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/oauth/auth-code-listener.ts` | 212 | 6650 | 重写：保持通用行为 | `src/oauth_callback_listener.rs / src/main.rs` | 部分实现：原版 localhost 动态端口监听、GET 回调地址匹配、授权码与 state 检查、404/400 及延后 302 响应已真实 Rust 实现并接入 oauth-listen CLI，覆盖真实 TCP/HTTP 回归；原版自定义成功回调及完整 OAuth token 客户端仍待对接；补齐异常 HTTP 请求头 400/408/431 后继续等待合法授权、浏览器 onReady 未完成时并发接收回调且回调后启动任务不被取消，真实 TCP/HTTP 测试通过（CI 40852563169）；完整自定义成功处理器、Token 客户端及取消生命周期尚未验收 |
| `src/services/oauth/client.ts` | 567 | 18254 | 重写：保持通用行为 | `src/oauth_client.rs / src/oauth_profile.rs` | 部分实现：授权 URL 参数、可配置权限、授权码真实 HTTP 交换、刷新令牌/轮换保留、到期缓冲、角色及 API key 端点均实现；原版专属订阅资料同步和全部账号缓存规则待补 |
| `src/services/oauth/crypto.ts` | 24 | 566 | 重写：保持通用行为 | `src/oauth_crypto.rs / src/main.rs` | 部分实现：32 字节系统级安全随机数、无填充 Base64URL、SHA-256 PKCE challenge 和独立 state 已有原生 Rust、RFC 7636 向量及真实 CLI 测试；OAuth 授权回调尚未接入 |
| `src/services/oauth/getOauthProfile.ts` | 54 | 1611 | 重写：保持通用行为 | `src/oauth_profile.rs / src/main.rs` | 部分实现：API key/account_uuid 请求及 OAuth Bearer 资料查询真实 HTTP、10 秒超时、非 2xx/无效响应处理与可配置多提供商端点已在 Rust 落地；真实本地 HTTP 服务测试已覆盖，OAuth 主客户端持久化调用方待接 |
| `src/services/oauth/index.ts` | 199 | 6554 | 重写：保持通用行为 | `src/oauth_service.rs / src/main.rs` | 部分实现：自动 localhost 回调和手动输入两条授权链、PKCE 状态、授权后凭据 0600 原子保存、真实 CLI 入口和出错浏览器回调已实现；原版浏览器自动弹出及 SDK 权限同步仍待补 |
| `src/services/plugins/PluginInstallationManager.ts` | 185 | 6017 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/plugins/pluginCliCommands.ts` | 345 | 10894 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/plugins/pluginOperations.ts` | 1089 | 35619 | 重写：保持通用行为 | `src/plugin_manager.rs / src/generated_ports/p_d37d4b8c26098.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收；本地安装身份预检、启用与回滚已有真实 Rust 执行链，原版市场/依赖闭包仍缺 |
| `src/services/policyLimits/index.ts` | 664 | 18064 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/policyLimits/types.ts` | 28 | 792 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/preventSleep.ts` | 166 | 4586 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/rateLimitMessages.ts` | 345 | 10858 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/services/rateLimitMocking.ts` | 145 | 4420 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/services/remoteManagedSettings/index.ts` | 639 | 20911 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/remoteManagedSettings/securityCheck.tsx` | 74 | 10513 | 重写：保持通用行为 | `src/generated_ports/p_8985ba0623d61.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/services/remoteManagedSettings/syncCache.ts` | 113 | 4229 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/remoteManagedSettings/syncCacheState.ts` | 97 | 4004 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/remoteManagedSettings/types.ts` | 32 | 1060 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/settingsSync/index.ts` | 582 | 17924 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/settingsSync/types.ts` | 68 | 1668 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/teamMemorySync/index.ts` | 1257 | 44153 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/teamMemorySync/secretScanner.ts` | 325 | 9458 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/teamMemorySync/teamMemSecretGuard.ts` | 45 | 1552 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/teamMemorySync/types.ts` | 157 | 4906 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/teamMemorySync/watcher.ts` | 388 | 13405 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/tips/tipHistory.ts` | 18 | 601 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/services/tips/tipRegistry.ts` | 687 | 23154 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/tips/tipScheduler.ts` | 59 | 1664 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/tokenEstimation.ts` | 496 | 16883 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/toolUseSummary/toolUseSummaryGenerator.ts` | 113 | 3376 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/tools/StreamingToolExecutor.ts` | 531 | 17196 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/tools/toolExecution.ts` | 1746 | 60309 | 重写：工具调用与 Hook 生命周期 | `src/agent.rs / src/hooks.rs / src/tools.rs` | 部分实现：非法输入与 PreToolUse 阻止/失败逐工具回填，跳过实际执行的工具不触发 PostToolUseFailure；已执行工具的 PostToolUse/Failure 获得原版顶层结构化有效输入，失败事件使用 error/is_interrupt 字段；并行调度、取消与完整原版事件时序仍待对照 |
| `src/services/tools/toolHooks.ts` | 651 | 22333 | 重写：Hooks 事件与生命周期 | `src/hooks.rs / src/agent.rs` | 部分实现：PostToolUse 的 MCP 输出替换和连续 Hook 可见性已接入 Agent；完整 Hook 事件/阻断/回调时序仍缺 |
| `src/services/tools/toolOrchestration.ts` | 189 | 5501 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/vcr.ts` | 407 | 12166 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/voice.ts` | 526 | 17116 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/voiceKeyterms.ts` | 107 | 3462 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/services/voiceStreamSTT.ts` | 545 | 21375 | 重写：保持通用行为 | `src/generated_ports/p_5bf6ce115b634.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/skills/bundled/batch.ts` | 125 | 7177 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/claudeApi.ts` | 197 | 6323 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/claudeApiContent.ts` | 76 | 4280 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/claudeInChrome.ts` | 35 | 1760 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/debug.ts` | 104 | 4219 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/index.ts` | 80 | 3252 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/keybindings.ts` | 340 | 10412 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/loop.ts` | 93 | 4667 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/loremIpsum.ts` | 283 | 4400 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/remember.ts` | 83 | 4227 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/scheduleRemoteAgents.ts` | 448 | 19048 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/simplify.ts` | 70 | 4466 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/skillify.ts` | 198 | 9483 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/stuck.ts` | 80 | 4254 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/updateConfig.ts` | 476 | 17453 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/verify.ts` | 31 | 893 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundled/verifyContent.ts` | 14 | 414 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/bundledSkills.ts` | 221 | 7497 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/skills/loadSkillsDir.ts` | 1087 | 34415 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现：插件清单自定义/嵌套技能经 src/plugins.rs::resolve_skill_path 接入 ToolExecutor::list_skills/read_skill，真实 Read 权限及技能路径/会话变量已接通，源码已提交待验收；disable-model-invocation 已在模型工具目录和读取入口阻断（cb2e0ac），已提交待验收；本地技能 /name 交互执行及 /skills 用户权限目录已接通（b3b63dc、be77ed4），源码待验收；技能模型/effort、动态刷新和完整交互仍缺；SDK 本地技能目录补齐真实 Markdown 描述和参数提示（48e34f9、94f217d、ffd1056），源码已提交待验收；复杂 YAML、模型/effort 元数据和热刷新仍缺；交互式 /skills 现展示可调用本地/插件技能说明和参数提示（68d8545），源码待验收 |
| `src/skills/mcpSkillBuilders.ts` | 45 | 1627 | 重写：技能及加载机制 | `src/tools.rs` | 部分实现，尚未完全等价 |
| `src/state/AppState.tsx` | 200 | 23480 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/state/AppStateStore.ts` | 570 | 21847 | 重写：保持通用行为 | `src/generated_ports/p_4160de63cf190.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/state/onChangeAppState.ts` | 172 | 6197 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/state/selectors.ts` | 77 | 2240 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/state/store.ts` | 35 | 836 | 重写：保持通用行为 | `src/reactive_store.rs` | 部分实现：状态/清理注册、顺序和订阅基础测试；JS Object.is 与 Promise 异常传播仍需等价验证 |
| `src/state/teammateViewHelpers.ts` | 142 | 4399 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/tasks/DreamTask/DreamTask.ts` | 158 | 4988 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/InProcessTeammateTask/InProcessTeammateTask.tsx` | 126 | 16381 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/InProcessTeammateTask/types.ts` | 122 | 4322 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/LocalAgentTask/LocalAgentTask.tsx` | 683 | 82910 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/LocalMainSessionTask.ts` | 480 | 15136 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/LocalShellTask/LocalShellTask.tsx` | 523 | 66306 | 重写：任务调度 | `src/background.rs / src/tools.rs` | 部分实现：进程启动、磁盘输出、停止和状态回填；UI 和完整通知时序未等价 |
| `src/tasks/LocalShellTask/guards.ts` | 42 | 1552 | 重写：任务调度 | `src/task_guards.rs` | 已验收：isLocalShellTask 对 object/null/array/类型字段和附加字段的行为对照；证据 task_guards::tests |
| `src/tasks/LocalShellTask/killShellTasks.ts` | 77 | 2565 | 重写：任务调度 | `src/background.rs` | 部分实现：Shell 进程组停止；取消状态和外部子进程边界未全等 |
| `src/tasks/RemoteAgentTask/RemoteAgentTask.tsx` | 856 | 126389 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/pillLabel.ts` | 83 | 2898 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/stopTask.ts` | 101 | 2894 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tasks/types.ts` | 47 | 1691 | 重写：任务调度 | `src/task.rs` | 部分实现，后台与依赖未等价 |
| `src/tools/AgentTool/AgentTool.tsx` | 1398 | 233734 | 重写：工具行为、参数和权限 | `src/agent.rs` / `src/worktree.rs` / `src/permission_rules.rs` | 部分实现：独立子任务、隔离与显式委派权限拒绝/询问；子代理 Hook 使用子会话上下文、阻断后继续工作（f3b9bac5）；团队常驻、远程和全部权限继承未等价 |
| `src/tools/AgentTool/UI.tsx` | 872 | 125359 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/agentColorManager.ts` | 67 | 1499 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/agentDisplay.ts` | 105 | 3269 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/agentMemory.ts` | 178 | 5853 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/agentMemorySnapshot.ts` | 198 | 5633 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/agentToolUtils.ts` | 687 | 22739 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/built-in/claudeCodeGuideAgent.ts` | 206 | 8959 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/built-in/exploreAgent.ts` | 84 | 4686 | 重写：工具行为、参数和权限 | `src/prompt_runtime.rs / src/agent.rs / src/tools.rs` | 部分实现：探索角色 system 和只读工具约束已进入真实执行链；原版最小搜索策略与边界待完整验收 |
| `src/tools/AgentTool/built-in/generalPurposeAgent.ts` | 35 | 2185 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/built-in/planAgent.ts` | 93 | 4318 | 重写：工具行为、参数和权限 | `src/prompt_runtime.rs / src/agent.rs / src/tools.rs` | 部分实现：规划角色 system 和只读工具约束已进入真实执行链；原版完整计划流程仍待逐项验收 |
| `src/tools/AgentTool/built-in/statuslineSetup.ts` | 145 | 7477 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/built-in/verificationAgent.ts` | 153 | 11410 | 重写：工具行为、参数和权限 | `src/prompt_runtime.rs / src/agent.rs / src/tools.rs` | 部分实现：独立验收 system 和只读工具隔离已接入模型请求；原版完整验收规范及所有权限分支待核对 |
| `src/tools/AgentTool/builtInAgents.ts` | 73 | 2756 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/constants.ts` | 13 | 547 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/forkSubagent.ts` | 211 | 8678 | 重写：工具行为、参数和权限 | `src/agent.rs` / `src/worktree.rs` | 部分实现：独立子任务和隔离，团队及远程不等价 |
| `src/tools/AgentTool/loadAgentsDir.ts` | 756 | 26220 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/prompt.ts` | 288 | 16671 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/resumeAgent.ts` | 266 | 9339 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/AgentTool/runAgent.ts` | 974 | 35768 | 重写：工具行为、参数和权限 | `src/agent.rs / src/worktree.rs` | 部分实现：独立子 Agent 调用；后台管理未等价 |
| `src/tools/AskUserQuestionTool/AskUserQuestionTool.tsx` | 266 | 39887 | 重写：工具行为、参数和权限 | `src/ask_user_question.rs / src/agent.rs` | 部分实现：已接入交互式模型工具主链，验证1-4问题、2-4选项、去重、多选、Other、拒绝模型伪造答案；CLI→HTTP模型→真实输入→模型返回集成测试通过；终端已支持所选预览与用户备注 annotations 回传模型、严格输入字段校验；SDK 外部问答回执、预览格式的 SDK 配置传播及原版 TUI 仍未等价；新增可选 HTML 片段安全校验（ATLAS_QUESTION_PREVIEW_FORMAT=html）；新改动待行为验证 |
| `src/tools/AskUserQuestionTool/prompt.ts` | 45 | 2903 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/BashTool.tsx` | 1144 | 160530 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；Bash 毫秒超时与语义数字输入接通真实进程截止时间，Shell 严格参数校验提前至 Hook 前（6de19f6、105373e），待验收 |
| `src/tools/BashTool/BashToolResultMessage.tsx` | 191 | 19215 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/UI.tsx` | 185 | 25213 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/bashCommandHelpers.ts` | 266 | 8589 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/bashPermissions.ts` | 2622 | 98756 | 重写：工具行为、参数和权限 | `src/tools.rs / src/permission_rules.rs / src/bash_permission_suggestions.rs` | 部分实现：按 2.1.88 bashPermissions.ts 逐函数转写安全环境前缀、首词 UI 过滤、heredoc/多行建议与规则对象；仍缺完整 Shell AST、沙箱及权限提示 UI |
| `src/tools/BashTool/bashSecurity.ts` | 2593 | 102561 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/commandSemantics.ts` | 141 | 3658 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/commentLabel.ts` | 14 | 637 | 重写：工具行为、参数和权限 | `src/compat_helpers.rs` | 已验收：首行注释、shebang 和空白规则纯函数语义对照；证据 compat_helpers::tests::source_bash_comment_cases |
| `src/tools/BashTool/destructiveCommandWarning.ts` | 103 | 2935 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/modeValidation.ts` | 116 | 3266 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/pathValidation.ts` | 1304 | 43679 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/prompt.ts` | 370 | 21130 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/readOnlyValidation.ts` | 1991 | 68322 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/sedEditParser.ts` | 323 | 9562 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/sedValidation.ts` | 685 | 21518 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/shouldUseSandbox.ts` | 154 | 5185 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BashTool/toolName.ts` | 3 | 89 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/BashTool/utils.ts` | 224 | 7207 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BriefTool/BriefTool.ts` | 205 | 7677 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BriefTool/UI.tsx` | 101 | 14303 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BriefTool/attachments.ts` | 111 | 3889 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BriefTool/prompt.ts` | 23 | 1933 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/BriefTool/upload.ts` | 175 | 5815 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ConfigTool/ConfigTool.ts` | 468 | 13478 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ConfigTool/UI.tsx` | 38 | 5717 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ConfigTool/constants.ts` | 2 | 41 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/ConfigTool/prompt.ts` | 94 | 2881 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ConfigTool/supportedSettings.ts` | 212 | 6374 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterPlanModeTool/EnterPlanModeTool.ts` | 127 | 4101 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterPlanModeTool/UI.tsx` | 33 | 5315 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterPlanModeTool/constants.ts` | 2 | 57 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/EnterPlanModeTool/prompt.ts` | 171 | 7752 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterWorktreeTool/EnterWorktreeTool.ts` | 128 | 4364 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterWorktreeTool/UI.tsx` | 20 | 3312 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/EnterWorktreeTool/constants.ts` | 2 | 56 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/EnterWorktreeTool/prompt.ts` | 31 | 1412 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitPlanModeTool/ExitPlanModeV2Tool.ts` | 494 | 17006 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitPlanModeTool/UI.tsx` | 82 | 11418 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitPlanModeTool/constants.ts` | 3 | 113 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/ExitPlanModeTool/prompt.ts` | 30 | 2139 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitWorktreeTool/ExitWorktreeTool.ts` | 330 | 11649 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitWorktreeTool/UI.tsx` | 25 | 4044 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ExitWorktreeTool/constants.ts` | 2 | 54 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/ExitWorktreeTool/prompt.ts` | 33 | 2021 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileEditTool/FileEditTool.ts` | 626 | 20502 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；Edit 现检查会话已读完整内容及外部修改，成功写入后更新状态（ae165e76），待验收；；Edit 弯引号容错及排版保真接入真实文件修改链（9eed00be），源码已提交待验收；；原版 Edit(file_path,old_string,new_string,replace_all) 经原版输入门禁进入跨轮读后编辑与 Hook 链（41404581、d061352e、7959a531），待验收； |
| `src/tools/FileEditTool/UI.tsx` | 289 | 34832 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileEditTool/constants.ts` | 12 | 538 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileEditTool/prompt.ts` | 29 | 1895 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileEditTool/types.ts` | 86 | 2612 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；Edit 原版严格参数名和语义布尔已接入运行时（d061352e），仍缺完整输出结构，待验收； |
| `src/tools/FileEditTool/utils.ts` | 776 | 22516 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；弯引号文本匹配、替换时保留引号排版和非 Markdown 行尾空白归一化（9eed00be、645510ed、f17bb1c9），源码已提交待验收； |
| `src/tools/FileReadTool/FileReadTool.ts` | 1184 | 39073 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；读取状态在 Agent 跨轮共享，区分完整/部分读取与输出截断（ae165e76、2206e0f9），待验收；；原版 Read(file_path,offset,limit) 严格参数转换、256KiB 文件上限及不受通用16K截断的真实执行入口（41404581、d061352e、223c5e08），待验收；；原版 Read 同范围且 mtime 未变化时返回简短复用提示，避免重复长文本占用上下文（cf83f4b6、1e8b095a、1b661e19），待验收；；按原版项目临时目录语义读取本会话后台任务 stdout/stderr，保留 Read 规则和流式有界行读取，不扩大编辑权限（46319cca、e773d797），源码已提交待验收 |
| `src/tools/FileReadTool/UI.tsx` | 185 | 22540 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileReadTool/imageProcessor.ts` | 95 | 2881 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileReadTool/limits.ts` | 93 | 3219 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileReadTool/prompt.ts` | 50 | 2868 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；Read 原版参数名和全范围读取入口已接通（223c5e08），其他媒体和完整提示仍缺，待验收； |
| `src/tools/FileWriteTool/FileWriteTool.ts` | 435 | 14927 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；覆盖已有文件前要求已读完整内容且无外部修改，新文件成功写入后记录状态（af2534e9），待验收；；原版 Write(file_path,content) 经严格输入门禁进入读后覆盖和文件新建执行链（41404581、d061352e、7959a531），待验收； |
| `src/tools/FileWriteTool/UI.tsx` | 405 | 43788 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/FileWriteTool/prompt.ts` | 19 | 969 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/GlobTool/GlobTool.ts` | 199 | 6064 | 重写：工具行为、参数和权限 | `src/search.rs` | 部分实现：通配符、结果排序与 100 项上限；原版全部边界待核对；本轮按原版补齐绝对路径通配、隐藏文件开关和可选 Git 忽略过滤，完整插件缓存排除和动态限制仍缺 |；新增按原版 `utils/glob.ts` 区分 Glob/Grep 的目录遍历：Glob 不再强行跳过 VCS、插件目录和构建目录，结果按修改时间从旧到新；代码 47684bbc、b734011b，已提交待验收；插件缓存排除、动态限制及完整边界仍缺；严格参数 schema 与语义布尔/数字转换接入 Agent PreToolUse 和 ToolExecutor 权限前置入口（f7fd05f、cc5bcc0、a24db99），源码已提交待验收，原版全部边界尚未等价；新增对每个候选文件执行 Read deny 路径过滤（ask 仍在搜索根目录审批），避免获准搜索目录内的被拒绝文件名泄露；真实调用链 ToolExecutor::dispatch→glob_files_filtered→search_read_allowed，代码 93fa1d9、7c5a8dd，已提交待验收；绝对通配符现按原版解析静态搜索根，经过额外工作区授权和真实 Read 审批后执行（814bb0d、33dc985），源码已提交待验收；特殊平台路径仍待验收；新增与原版 ripGrep 一致的 Glob 搜索时间预算：默认 20 秒，WSL 60 秒，可由 ATLAS_GLOB_TIMEOUT_SECONDS 正整数覆盖，遍历超时返回明确错误，不把未扫完的文件列表当成完整结果（17a8e25e，源码已提交待验收）；子进程级中止/完整忽略规则仍缺；Windows 正斜杠绝对通配符可按原版提取搜索根并进入授权/Glob 执行链（b7a54b76，已提交待验收）；UNC 和驱动器边界仍待验证；GlobTool 否定模式 !pattern 现排除匹配文件，保留原版 ripgrep --glob 语义（8510c3d2，源码已提交待验收）；复杂模式仍待验证；补齐过期插件缓存目录遍历前剪枝（952cf33f、5218a04f，已提交待验收）；会话级缓存冻结及重载仍缺
| `src/tools/GlobTool/UI.tsx` | 63 | 8040 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/GlobTool/prompt.ts` | 8 | 439 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/GrepTool/GrepTool.ts` | 578 | 20087 | 重写：工具行为、参数和权限 | `src/search.rs` | 部分实现：内容/路径/计数/分页/上下文；原版 ripgrep 语义待对照；本轮按原版补齐跨行正则、逐行计数、-C/context 优先级、-n 行号及 head_limit=0 不限量，仍缺完整 ripgrep 忽略规则/中断语义；新增按原版 `GrepTool.ts::call` 的 `--hidden` 检索隐藏文件、仅排除原版六类 VCS 目录、移除错误的 2MiB 整文件跳过限制；代码 47684bbc，已提交待验收；取消和完整 ripgrep 语义仍缺；严格参数 schema 与语义布尔/数字转换接入 Agent PreToolUse 和 ToolExecutor 权限前置入口（f7fd05f、cc5bcc0、a24db99），源码已提交待验收，原版全部边界尚未等价；对照原版 GrepTool.ts::call 的 --max-columns=500 及 ripgrep 上下文分组输出，补齐 UTF-8/CRLF 长行省略、匹配/上下文省略提示、跨文件及非连续匹配的 -- 分隔符，并保持分页统计一致（b72b803d，源码已提交待验收）；完整忽略规则、取消与 ripgrep 边界仍待等价；对照原版 ripgrep.ts 的文本/二进制搜索路径，修复非 UTF-8 文本整文件被跳过、目录扫描二进制结果及显式二进制文件内容提示（e311d942，源码已提交待验收）；完整 ripgrep 错误/中断/忽略/编码语义仍待核验；按原版 ripgrep 类型表补充 TS/JS/Python/Markdown/JSON 常见扩展名筛选（ee329ec4、2c7d3ca7，源码已提交待验收）；新增逐文件 Read deny 过滤（ask 在搜索根目录审批），在打开文件前生效并覆盖 content/count/files_with_matches 三种模式；读取权限/IO 错误显式上报，已删除文件仍可继续搜索，代码 93fa1d9、7c5a8dd、e039a06，已提交待验收；用户指定的上下文/分页上限不再被静默裁剪，极大参数按平台范围明确处理（60ef1287，已提交待验收）；完整 ripgrep 行为仍未等价 |；Grep 搜索按原版 ripgrep.ts 采用 20 秒（WSL 60 秒）预算，逐目录/文件及输出检查超时并报告未完成，避免把部分搜索结果伪装为完整结果（fbd4a626，源码已提交待验收）；正在执行的阻塞 I/O 和正则无法立即抢占，取消/完整 ripgrep 语义仍缺；Grep 多个正反 glob 现按原版输入顺序由后命中规则覆盖（82b0bcd6，源码已提交待验收）；与 Git ignore 的跨来源覆盖仍缺；Grep content/count 不再被强制按文件时间重排，只有文件名模式按原版新到旧排序（98b6fc42，源码已提交待验收）；ripgrep 默认遍历顺序仍待验收；补齐过期插件缓存版本的检索排除（952cf33f、5218a04f，已提交待验收）；原版缓存刷新语义仍缺；增加 .ignore/.rgignore 多层目录规则、反向规则及高于 Git 忽略的优先级，Grep 与可选忽略模式 Glob 共同接入（6b3d3292、55328d32），源码已提交待验收；复杂 gitignore 目录恢复、全局忽略和外部取消仍待核对
| `src/tools/GrepTool/UI.tsx` | 201 | 21784 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/GrepTool/prompt.ts` | 19 | 1150 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/LSPTool/LSPTool.ts` | 861 | 25710 | 重写：工具行为、参数和权限 | `src/lsp.rs` | 部分实现：9 类请求与真实模拟服务器回环；LSP 管理行为未全等 |
| `src/tools/LSPTool/UI.tsx` | 228 | 25038 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/LSPTool/formatters.ts` | 593 | 17399 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/LSPTool/prompt.ts` | 22 | 1114 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/LSPTool/schemas.ts` | 216 | 6064 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/LSPTool/symbolContext.ts` | 91 | 3392 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ListMcpResourcesTool/ListMcpResourcesTool.ts` | 124 | 3907 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ListMcpResourcesTool/UI.tsx` | 29 | 4757 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ListMcpResourcesTool/prompt.ts` | 21 | 776 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/MCPTool/MCPTool.ts` | 78 | 2170 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/MCPTool/UI.tsx` | 403 | 50730 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/MCPTool/classifyForCollapse.ts` | 605 | 15178 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/MCPTool/prompt.ts` | 4 | 119 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/McpAuthTool/McpAuthTool.ts` | 216 | 7873 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/NotebookEditTool/NotebookEditTool.ts` | 491 | 15271 | 重写：工具行为、参数和权限 | `src/notebook.rs / src/tools.rs` | 部分实现：插入、替换、删除、ID 定位及读后编辑测试通过；编码和 UI 待等价；笔记本读取修订跨模型轮次保持（2206e0f9），待验收； |
| `src/tools/NotebookEditTool/UI.tsx` | 93 | 13540 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/NotebookEditTool/constants.ts` | 3 | 104 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/NotebookEditTool/prompt.ts` | 4 | 632 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/PowerShellTool.tsx` | 1001 | 144624 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全；新增 PowerShell 超时毫秒与后台执行布尔值的原版语义字符串输入转换，实际执行器接线仍部分实现 |
| `src/tools/PowerShellTool/UI.tsx` | 131 | 19768 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/clmTypes.ts` | 212 | 7229 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/commandSemantics.ts` | 143 | 5459 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/commonParameters.ts` | 31 | 894 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/destructiveCommandWarning.ts` | 110 | 3402 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/gitSafety.ts` | 177 | 7695 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/modeValidation.ts` | 405 | 17399 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/pathValidation.ts` | 2050 | 73059 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/powershellPermissions.ts` | 1649 | 67606 | 重写：工具行为、参数和权限 | `src/tools.rs / src/powershell_permission_matching.rs / src/bash_permission_suggestions.rs` | 部分实现：PowerShell/Bash 规则隔离、别名/大小写/拒绝优先、复合语句分拆；新增变量展开、动态环境变量、脚本路径冒充 cmdlet、#Requires 与 using/--% 的显式审批门槛；原生 PowerShell AST 全语义和权限 UI 仍未等价 |
| `src/tools/PowerShellTool/powershellSecurity.ts` | 1091 | 37651 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/prompt.ts` | 146 | 9826 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/readOnlyValidation.ts` | 1824 | 67327 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/PowerShellTool/toolName.ts` | 3 | 110 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/REPLTool/constants.ts` | 47 | 1799 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/REPLTool/primitiveTools.ts` | 40 | 1532 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ReadMcpResourceTool/ReadMcpResourceTool.ts` | 159 | 4654 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ReadMcpResourceTool/UI.tsx` | 37 | 6109 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ReadMcpResourceTool/prompt.ts` | 17 | 544 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/RemoteTriggerTool/RemoteTriggerTool.ts` | 162 | 4715 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/RemoteTriggerTool/UI.tsx` | 17 | 3096 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/RemoteTriggerTool/prompt.ts` | 16 | 697 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ScheduleCronTool/CronCreateTool.ts` | 158 | 5710 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ScheduleCronTool/CronDeleteTool.ts` | 96 | 2616 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ScheduleCronTool/CronListTool.ts` | 98 | 2888 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ScheduleCronTool/UI.tsx` | 60 | 8075 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ScheduleCronTool/prompt.ts` | 136 | 7518 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SendMessageTool/SendMessageTool.ts` | 918 | 27506 | 重写：工具行为、参数和权限 | `src/team_registry.rs / src/team_mailbox.rs / src/agent.rs` | 部分实现：单发/广播、关停及计划审批请求/回复、队长模式设置均进入真实模型工具链；计划回复关联真实请求、并发去重、失败保留未读；远程路由、权限询问、完整原版协议未等价 |
| `src/tools/SendMessageTool/UI.tsx` | 31 | 4679 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SendMessageTool/constants.ts` | 2 | 52 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/SendMessageTool/prompt.ts` | 50 | 2356 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SkillTool/SkillTool.ts` | 1109 | 38175 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现：模型工具 list_skills/read_skill 使用插件真实清单路径及 Read 权限检查，修复自定义/嵌套技能不可执行；原版技能/插件根路径及会话变量展开，已提交待验收；模型调用禁用标志已在真实 read_skill/list_skills 双重生效（cb2e0ac），待验收；用户与模型技能权限双通道已在 ToolExecutor::render_skill/list_skills_filtered 接线，源码待验收；完整 SkillTool UI/其他权限/异步模型元数据仍缺；模型直接读取插件技能时复用非敏感配置展开和敏感占位保护（0b0ba2a、3325716），源码待验收；技能完整生命周期/取消仍缺 |
| `src/tools/SkillTool/UI.tsx` | 128 | 19189 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SkillTool/constants.ts` | 2 | 39 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/SkillTool/prompt.ts` | 242 | 8221 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SleepTool/prompt.ts` | 18 | 774 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/SyntheticOutputTool/SyntheticOutputTool.ts` | 164 | 5468 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskCreateTool/TaskCreateTool.ts` | 139 | 3441 | 重写：工具行为、参数和权限 | `src/task_graph.rs / src/agent.rs` | 部分实现：磁盘持久化、任务编号、元数据、TaskCreated hook 阻断回滚及模型调用；UI 自动展开与上下文差异待对照；严格参数校验已提前到 PreToolUse 前，非法创建不触发 Hook（06c65cc） |
| `src/tools/TaskCreateTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TaskCreateTool/prompt.ts` | 57 | 2760 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskGetTool/TaskGetTool.ts` | 129 | 2881 | 重写：工具行为、参数和权限 | `src/task_graph.rs / src/agent.rs` | 部分实现：任务不存在、状态、描述及双向依赖读取与原版文本输出；所有错误处理和 SDK 数据结构待对照；TaskGet 输入严格校验已接入实际 Agent Hook 前置门禁（06c65cc） |
| `src/tools/TaskGetTool/constants.ts` | 2 | 44 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TaskGetTool/prompt.ts` | 25 | 823 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskListTool/TaskListTool.ts` | 117 | 2803 | 重写：工具行为、参数和权限 | `src/task_graph.rs / src/agent.rs` | 部分实现：任务过滤 _internal、已完成依赖解除、负责人/阻塞展示；完整 UI/排序与团队可见性待核对；TaskList 拒绝未知参数的校验已接入实际 Agent Hook 前置门禁（06c65cc） |
| `src/tools/TaskListTool/constants.ts` | 2 | 46 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TaskListTool/prompt.ts` | 50 | 2066 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskOutputTool/TaskOutputTool.tsx` | 584 | 66568 | 重写：工具行为、参数和权限 | `src/background.rs / src/tools.rs` | 部分实现：阻塞/非阻塞输出、超时和状态；Agent/远程输出尚未覆盖；原版严格参数/semanticBoolean 统一预校验，在 PreToolUse 前拒绝无效请求（0bb6043），待验收；长任务输出保持结构化 JSON/状态与输出路径，不再被通用16k截断（8f59c4b0），源码已提交待验收 |
| `src/tools/TaskOutputTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TaskStopTool/TaskStopTool.ts` | 132 | 3935 | 重写：工具行为、参数和权限 | `src/background.rs / src/tools.rs` | 部分实现：task_id、shell_id、KillShell、成功返回和异常状态对齐；新增停止与自然完成竞态结果核对，源码已提交待验收；其余任务类型/完整生命周期待验收；停止前核对实时状态、旧 shell_id 与严格参数，阻止不存在/已完成任务触发权限与 Hook（9008bf3、9b0a7fe），待验收 |
| `src/tools/TaskStopTool/UI.tsx` | 41 | 5672 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskStopTool/prompt.ts` | 9 | 280 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TaskUpdateTool/TaskUpdateTool.ts` | 407 | 12161 | 重写：工具行为、参数和权限 | `src/task_graph.rs / src/agent.rs / src/team_mailbox.rs` | 部分实现：状态/所有者/元数据、双向依赖与删除级联、TaskCompleted hook 阻断、队员启动任务自动认领及负责人邮箱通知；队员完成任务后收到领取下一任务的提示；验证提醒、UI 与完整时序未等价；完成 Hook 前校验全部 TaskUpdate 字段，非法参数不触发副作用（809afb、06c65cc） |
| `src/tools/TaskUpdateTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TaskUpdateTool/prompt.ts` | 78 | 2375 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TeamCreateTool/TeamCreateTool.ts` | 241 | 7665 | 重写：工具行为、参数和权限 | `src/team_registry.rs / src/agent.rs` | 部分实现：团队配置、任务图、成员登记与工具入口；原版全部成员字段、后端与生命周期未等价 |
| `src/tools/TeamCreateTool/UI.tsx` | 6 | 1038 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TeamCreateTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TeamCreateTool/prompt.ts` | 114 | 6900 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TeamDeleteTool/TeamDeleteTool.ts` | 140 | 4221 | 重写：工具行为、参数和权限 | `src/team_registry.rs / src/agent.rs` | 部分实现：活跃成员阻止清理、目录删除、任务图恢复；原版工作树/窗格清理未等价 |
| `src/tools/TeamDeleteTool/UI.tsx` | 20 | 2906 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TeamDeleteTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/TeamDeleteTool/prompt.ts` | 17 | 684 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TodoWriteTool/TodoWriteTool.ts` | 116 | 3881 | 重写：工具行为、参数和权限 | `src/task.rs / src/session.rs / src/agent.rs` | 部分实现：无交互 SDK/单次运行暴露 TodoWrite；严格清单字段与状态校验、整表替换、全部完成自动清空、会话保存恢复；原版 UI 状态与跨会话边界待验收 |
| `src/tools/TodoWriteTool/constants.ts` | 2 | 48 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/TodoWriteTool/prompt.ts` | 185 | 9527 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/ToolSearchTool/ToolSearchTool.ts` | 472 | 14275 | 重写：工具行为、参数和权限 | `src/tool_search.rs / src/agent.rs` | 部分实现：已接入模型可调用工具搜索，精确选择、MCP前缀、关键词权重和必须词、数量限制；新增 MCP 工具名称延迟目录、模型选中后下一轮动态注入 schema、JSON 模式定义回传。CLI→HTTP模型→查询结果→会话保存测试通过；原版 tool_reference 原生协议及更多动态同步场景尚未等价；新增完整工具名回退评分及 searchHint 关键词权重，完整 deferred/tool_reference 协议仍缺 |
| `src/tools/ToolSearchTool/constants.ts` | 2 | 50 | 重写：工具行为、参数和权限 | `src/tool_names.rs` | 已验收：固定工具名常量的所有导出值；证据 tool_names::tests::names_and_aliases |
| `src/tools/ToolSearchTool/prompt.ts` | 122 | 5227 | 重写：工具行为、参数和权限 | `src/tool_search.rs` | 部分实现：可按真实工具表检索；原版 deferred 动态加载提示协议尚未等价 |
| `src/tools/WebFetchTool/UI.tsx` | 72 | 8083 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/WebFetchTool/WebFetchTool.ts` | 319 | 9324 | 重写：工具行为、参数和权限 | `src/webfetch.rs` | 部分实现：公网文本抓取/权限/跳转；原版网页摘要算法未等价 ；原版 domain:<hostname> 的精确域名权限 allow/ask/deny 已连通 web_fetch 权限检查 ；跨主机 3xx 跳转前重新按目标域名复核权限，禁止借已批准来源绕过 deny；本轮修复跨来源重定向返回新 URL 供重新授权，禁止继承来源审批自动抓取，仍缺完整原版网页摘要 |
| `src/tools/WebFetchTool/preapproved.ts` | 167 | 5248 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/WebFetchTool/prompt.ts` | 47 | 2200 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/WebFetchTool/utils.ts` | 531 | 16761 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/WebSearchTool/UI.tsx` | 101 | 12160 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/WebSearchTool/WebSearchTool.ts` | 436 | 13548 | 重写：工具行为、参数和权限 | `src/websearch.rs` | 部分实现：SearXNG/Brave 可配置搜索；原版搜索返回行为未等价；本轮在通用搜索后端支持 allowed_domains/blocked_domains 及子域名匹配、查询长度校验，原版模型内置检索/引用协议仍缺 |
| `src/tools/WebSearchTool/prompt.ts` | 35 | 1545 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/shared/gitOperationTracking.ts` | 278 | 9485 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/shared/spawnMultiAgent.ts` | 1094 | 35553 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/testing/TestingPermissionTool.tsx` | 74 | 7312 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/tools/utils.ts` | 41 | 1105 | 重写：工具行为、参数和权限 | `src/tools.rs` | 部分实现，工具种类尚不全 |
| `src/types/command.ts` | 217 | 7736 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/types/generated/events_mono/claude_code/v1/claude_code_internal_event.ts` | 866 | 30728 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/types/generated/events_mono/common/v1/auth.ts` | 101 | 2822 | 重写：保持通用行为 | `src/generated_ports/p_90bb0f65ccd4f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/generated/events_mono/growthbook/v1/growthbook_experiment_event.ts` | 224 | 7591 | 重写：保持通用行为 | `src/generated_ports/p_461ec04c9ec8b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/generated/google/protobuf/timestamp.ts` | 188 | 6565 | 重写：保持通用行为 | `src/generated_ports/p_5144380e07c74.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/hooks.ts` | 291 | 9138 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/types/ids.ts` | 45 | 1295 | 重写：保持通用行为 | `src/generated_ports/p_87d79f34fe49a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/logs.ts` | 331 | 11291 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/types/permissions.ts` | 442 | 13145 | 重写：保持通用行为 | `src/generated_ports/p_cf19f3fcff0bf.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/plugin.ts` | 364 | 11308 | 重写：保持通用行为 | `src/generated_ports/p_cc2135ed93bfb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/types/textInputTypes.ts` | 388 | 11628 | 重写：保持通用行为 | `src/generated_ports/p_1b3d61a277808.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/upstreamproxy/relay.ts` | 456 | 14937 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/upstreamproxy/upstreamproxy.ts` | 286 | 9812 | 重写：保持通用行为 | `src/generated_ports/p_456bc58f2042a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/CircularBuffer.ts` | 85 | 1888 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/Cursor.ts` | 1531 | 46663 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/QueryGuard.ts` | 122 | 3597 | 重写：查询排队、预约、运行、取消与代次状态机 | `src/query_guard.rs` / `src/sdk_queue.rs` | 部分实现：同步预约、运行、取消和代次保护有测试；原始信号订阅动态增删、React UI 生命周期及跨模块取消时序未验收；证据 query_guard::tests / integration_cli::sdk_queue_drain_honors_priority_fifo_and_worker_isolation |
| `src/utils/Shell.ts` | 475 | 16929 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ShellCommand.ts` | 466 | 14138 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/abortController.ts` | 100 | 3364 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/activityManager.ts` | 165 | 4973 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/advisor.ts` | 146 | 5585 | 重写：保持通用行为 | `src/generated_ports/p_f90f0871a04d8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/agentContext.ts` | 179 | 6660 | 重写：保持通用行为 | `src/generated_ports/p_9029c5407283f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/agentId.ts` | 100 | 2805 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/agentSwarmsEnabled.ts` | 45 | 1417 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/agenticSessionSearch.ts` | 308 | 10067 | 重写：保持通用行为 | `src/session_control.rs / src/semantic_session_search.rs / src/agent.rs / src/main.rs` | 部分实现：保留标签、标题、摘要和首尾对话离线预筛；新增最多100会话候选、首尾50条/2000字符截断、用户配置多模型语义排序、严格结果索引过滤、Agent用量持久化、独立CLI与交互入口、模拟模型及磁盘会话测试；原版分支字段、LiteLog完整还原、模型取消与跨端UI仍未等价 |
| `src/utils/analyzeContext.ts` | 1383 | 42931 | 重写：保持通用行为 | `src/context_usage.rs` / `src/generated_ports/p_a127d290c8efb.rs` | 部分实现：真实 /context 统计修复空多模态块导致文本漏计及 SDK 双表示工具调用重复计数；源码已提交待验收，原版完整工具/系统/记忆令牌精算仍缺 |
| `src/utils/ansiToPng.ts` | 335 | 214955 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ansiToSvg.ts` | 273 | 8212 | 重写：保持通用行为 | `src/generated_ports/p_a9e775f18b3e2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/api.ts` | 719 | 26078 | 重写：保持通用行为 | `src/generated_ports/p_e7319660e090e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/apiPreconnect.ts` | 72 | 2843 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/appleTerminalBackup.ts` | 125 | 2798 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/argumentSubstitution.ts` | 146 | 5079 | 重写：技能参数解析、位置/命名替换及渐进提示 | `src/skill_arguments.rs` / `src/tools.rs` | 部分实现：原始占位符顺序、Shell 引号分组、frontmatter 参数名和实际 read_skill 工具接入；复杂 shell-quote 歧义、全部命令前端及未列出来源待继续核对 |
| `src/utils/array.ts` | 14 | 364 | 重写：保持通用行为 | `src/array_utils.rs` | 部分实现：intersperse 序号、count 布尔谓词、uniq 基础值去重；JS 对象引用身份与非布尔 Truthy 待核对 |
| `src/utils/asciicast.ts` | 240 | 7755 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/attachments.ts` | 3998 | 127423 | 重写：保持通用行为 | `src/generated_ports/p_4d95b4cfbf91b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/attribution.ts` | 394 | 13346 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/auth.ts` | 2003 | 65436 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/authFileDescriptor.ts` | 197 | 6787 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/authPortable.ts` | 20 | 674 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/autoModeDenials.ts` | 27 | 720 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/autoRunIssue.tsx` | 122 | 10579 | 重写：保持通用行为 | `src/generated_ports/p_d3dd9f3d01f73.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/autoUpdater.ts` | 562 | 18012 | 重写：保持通用行为 | `src/generated_ports/p_276b14dd2f617.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/aws.ts` | 75 | 2305 | 重写：保持通用行为 | `src/generated_ports/p_6ae88763d0d0f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/awsAuthStatusManager.ts` | 82 | 2043 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/background/remote/preconditions.ts` | 236 | 7336 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/background/remote/remoteSession.ts` | 99 | 3050 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/backgroundHousekeeping.ts` | 95 | 3213 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/ParsedCommand.ts` | 319 | 9221 | 重写：保持通用行为 | `src/generated_ports/p_46a866ee4d3c1.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/bash/ShellSnapshot.ts` | 583 | 21780 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/ast.ts` | 2680 | 112054 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/bashParser.ts` | 4437 | 130810 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/bashPipeCommand.ts` | 295 | 10708 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/commands.ts` | 1340 | 51078 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/heredoc.ts` | 734 | 31458 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/parser.ts` | 231 | 6652 | 重写：保持通用行为 | `src/generated_ports/p_b4c8857055c0a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/bash/prefix.ts` | 205 | 6222 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/registry.ts` | 54 | 1433 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/bash/shellCompletion.ts` | 260 | 7874 | 重写：保持通用行为 | `src/generated_ports/p_e33e4c1fdec9c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/bash/shellPrefix.ts` | 29 | 1028 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/bash/shellQuote.ts` | 305 | 10824 | 重写：Shell 解析、安全引用与歧义检测 | `src/shell_quote.rs` / `src/skill_arguments.rs` | 部分实现：单双引号、转义、命令分隔符、安全引用；H1 单引号/反斜杠漏洞和控制字符已接入真实 Bash 审批；完整第三方 shell-quote token、glob、其他复杂 Bash 安全校验待核对 |
| `src/utils/bash/shellQuoting.ts` | 129 | 4718 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/bash/specs/alias.ts` | 15 | 313 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/bash/specs/index.ts` | 19 | 380 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/bash/specs/nohup.ts` | 14 | 274 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/bash/specs/pyright.ts` | 92 | 2670 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/specs/sleep.ts` | 14 | 315 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/bash/specs/srun.ts` | 32 | 654 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/bash/specs/time.ts` | 14 | 244 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/bash/specs/timeout.ts` | 21 | 426 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/bash/treeSitterAnalysis.ts` | 507 | 17594 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/betas.ts` | 435 | 15682 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/billing.ts` | 79 | 2413 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/binaryCheck.ts` | 54 | 1466 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/browser.ts` | 69 | 1867 | 重写：保持通用行为 | `src/browser_launcher.rs / src/main.rs` | 部分实现：真实 OS 启动浏览器及文件默认处理器、HTTP/HTTPS URL 校验、BROWSER 环境选择和平台命令已通过 Tokio 子进程接入；原版全部调用链待完整移植 |
| `src/utils/bufferedWriter.ts` | 101 | 2584 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/bundledMode.ts` | 23 | 618 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/caCerts.ts` | 116 | 4401 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/caCertsConfig.ts` | 89 | 3608 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/cachePaths.ts` | 39 | 1411 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/classifierApprovals.ts` | 89 | 2448 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/classifierApprovalsHook.ts` | 18 | 531 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/claudeCodeHints.ts` | 194 | 6472 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeDesktop.ts` | 153 | 4063 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/chromeNativeHost.ts` | 528 | 13876 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/common.ts` | 541 | 14489 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/mcpServer.ts` | 294 | 11104 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/prompt.ts` | 84 | 5605 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/setup.ts` | 401 | 13011 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/setupPortable.ts` | 234 | 6767 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudeInChrome/toolRendering.tsx` | 262 | 33862 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/claudemd.ts` | 1480 | 46391 | 重写：保持通用行为 | `src/conversation_context.rs / src/agent.rs` | 部分实现：用户级/父目录/项目级指令及递归规则进入真实 Agent system prompt；新增 Markdown 文本中的 @路径引用、最多 5 层递归、循环去重、文本扩展名限制、代码围栏/行内代码/HTML 注释排除，工作区外未经授权的项目引用不注入；提交 b7dd2cf0、1b670581、ae1f934b，源码已提交待验收；嵌套工作树项目指令去重（fb4210e7）及授权附加目录可选指令发现（80336875、22b50ecd）已提交待验收；外部引用审批、条件规则按文件动态加载、记忆排除、来源开关及 Markdown 完整词法仍未等价；指令 glob 排除与来源独立开关、外部 @引用显式配置授权已接通真实 Agent，代码 7de3b735/03b5697f/95f8dde9/f3c2c0f6/93493750/123d67f0，均已提交待验收；完整交互审批、动态条件规则、SDK 来源传播与跨平台行为仍缺；修复 paths: ** / 空列表全路径规则误丢弃（63133ee2），已提交待验收；托管指令与规则配置加载进入真实 Agent（fa0895c8、be2417eb），源码已提交待验收；原版长文件/多文件及 UTF-8 容错读取补齐（764d05f8），已提交待验收；外部引用授权进一步按 Git 项目隔离（5f5a9940、9c8d6609、63695104），已提交待验收 |
| `src/utils/cleanup.ts` | 603 | 17770 | 重写：保持通用行为 | `src/generated_ports/p_971a6526ff4ab.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/cleanupRegistry.ts` | 26 | 900 | 重写：保持通用行为 | `src/cleanup_registry.rs` | 部分实现：状态/清理注册、顺序和订阅基础测试；JS Object.is 与 Promise 异常传播仍需等价验证 |
| `src/utils/cliArgs.ts` | 61 | 2067 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/cliHighlight.ts` | 55 | 2140 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/codeIndexing.ts` | 207 | 6110 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/collapseBackgroundBashNotifications.ts` | 85 | 2750 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/collapseHookSummaries.ts` | 60 | 1714 | 重写：保持通用行为 | `src/portable_render_semantics.rs / src/main.rs` | 部分实现：连续同标签 Hook 的计数、内容/错误并集、阻断、输出状态和最大并发耗时已实现；上游所有渲染调用链仍需继续接通 |
| `src/utils/collapseReadSearch.ts` | 1110 | 37902 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/collapseTeammateShutdowns.ts` | 56 | 1322 | 重写：保持通用行为 | `src/portable_render_semantics.rs / src/main.rs` | 部分实现：连续完成的进程内队员关停消息折叠、首消息元信息、边界与非关停保留已实现；上游所有渲染调用链仍需继续接通 |
| `src/utils/combinedAbortSignal.ts` | 48 | 1714 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/commandLifecycle.ts` | 22 | 440 | 重写：保持通用行为 | `src/command_lifecycle.rs / src/tools.rs` | 已验收：单监听器设置/替换/清除、通知同步顺序；证据 command_lifecycle::tests::single_listener_replace_clear_and_event_order |
| `src/utils/commitAttribution.ts` | 962 | 29594 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/completionCache.ts` | 167 | 5654 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/appNames.ts` | 197 | 6575 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/cleanup.ts` | 87 | 3298 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/common.ts` | 62 | 2617 | 重写：保持通用行为 | `src/generated_ports/p_94132d04ecb5a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/computerUse/computerUseLock.ts` | 216 | 7135 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/drainRunLoop.ts` | 80 | 2821 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/escHotkey.ts` | 55 | 1965 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/executor.ts` | 659 | 23812 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/gates.ts` | 73 | 2563 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/hostAdapter.ts` | 70 | 2771 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/inputLoader.ts` | 31 | 1190 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/computerUse/mcpServer.ts` | 107 | 4123 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/setup.ts` | 54 | 2020 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/swiftLoader.ts` | 24 | 925 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/toolRendering.tsx` | 125 | 17706 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/computerUse/wrapper.tsx` | 336 | 49436 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/concurrentSessions.ts` | 205 | 6808 | 重写：保持通用行为 | `src/generated_ports/p_fde79388ae178.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/config.ts` | 1818 | 63496 | 重写：保持通用行为 | `src/generated_ports/p_afe390eab4f35.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/configConstants.ts` | 22 | 733 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/contentArray.ts` | 52 | 1621 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/context.ts` | 222 | 6871 | 重写：保持通用行为 | `src/generated_ports/p_ac10a911dfef7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/contextAnalysis.ts` | 273 | 7733 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/contextSuggestions.ts` | 236 | 7258 | 重写：保持通用行为 | `src/generated_ports/p_91fa3406d8f8b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/controlMessageCompat.ts` | 33 | 1216 | 重写：保持通用行为 | `src/control_message_compat.rs / src/main.rs` | 部分实现：请求 ID 两种命名兼容、嵌套 response、优先级与 SDK 回复 ID；其余控制协议未全量验证 |
| `src/utils/conversationRecovery.ts` | 598 | 21077 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/cron.ts` | 309 | 9462 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/cronJitterConfig.ts` | 76 | 3414 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/cronScheduler.ts` | 566 | 21478 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/cronTasks.ts` | 459 | 17301 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/cronTasksLock.ts` | 196 | 6259 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/crossProjectResume.ts` | 76 | 2104 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/crypto.ts` | 14 | 763 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/cwd.ts` | 33 | 985 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/debug.ts` | 269 | 8000 | 重写：保持通用行为 | `src/generated_ports/p_5b257a1951fde.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/debugFilter.ts` | 158 | 5069 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/deepLink/banner.ts` | 124 | 4694 | 重写：保持通用行为 | `src/deep_link_banner.rs / src/main.rs / src/git_filesystem.rs` | 部分实现：原版外部深链来源、工作目录展示、长预填提示与本地克隆 FETCH_HEAD 新旧状态告警已原生 Rust 实现并接入 CLI；原版无头启动进程及全平台终端唤起仍未完成 |
| `src/utils/deepLink/parseDeepLink.ts` | 171 | 5715 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/deepLink/protocolHandler.ts` | 137 | 4943 | 重写：保持通用行为 | `src/deep_link_protocol.rs / src/terminal_launcher.rs / src/main.rs` | 部分实现：真实 URI 解析、显式 cwd 与本地仓库映射优先级、FETCH_HEAD 时间、当前二进制终端启动和可审阅提示输入已接入；macOS AppleEvent 原版桥接仍未完成 |
| `src/utils/deepLink/registerProtocol.ts` | 349 | 11858 | 重写：保持通用行为 | `src/deep_link_register.rs / src/main.rs` | 部分实现：Linux xdg-mime 与 desktop 文件、Windows 用户注册表、macOS AppleScript URL Handler 应用包和登记状态校验、显式 register-protocol 入口已实现；原版自动注册及限频行为未补 |
| `src/utils/deepLink/terminalLauncher.ts` | 558 | 17786 | 重写：保持通用行为 | `src/terminal_launcher.rs / src/main.rs` | 部分实现：Linux 常用终端、macOS AppleScript/原生 argv、Windows Terminal/PowerShell/cmd 分支和独立 argv/工作目录/脱离父进程已实现，平台完整等价待补 |
| `src/utils/deepLink/terminalPreference.ts` | 55 | 1897 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/desktopDeepLink.ts` | 237 | 7130 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/detectRepository.ts` | 179 | 6063 | 重写：保持通用行为 | `src/detect_repository.rs / src/diagnostics.rs` | 部分实现：Git SSH/HTTPS/企业版地址解析、仓库缓存和真实本地 Git 探测已在 Doctor 入口使用；原版全部环境调用方尚未完成等价验收 |
| `src/utils/diagLogs.ts` | 95 | 2784 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/diff.ts` | 178 | 4855 | 重写：行增删统计和差异工具 | `src/tool_metrics.rs` / `src/tools.rs` | 部分实现：使用文本行级差异统计写入与编辑增删行；原始补丁生成、应用、超时及引号转义待对照 |
| `src/utils/directMemberMessage.ts` | 70 | 1715 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/displayTags.ts` | 52 | 2256 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/doctorContextWarnings.ts` | 266 | 8071 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/doctorDiagnostic.ts` | 626 | 20502 | 重写：保持通用行为 | `src/generated_ports/p_d7010c7ce3c89.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/dxt/helpers.ts` | 89 | 2599 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/dxt/zip.ts` | 227 | 7704 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/earlyInput.ts` | 192 | 5398 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/editor.ts` | 184 | 6634 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/effort.ts` | 330 | 11583 | 重写：保持通用行为 | `src/generated_ports/p_062c5079e1519.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/embeddedTools.ts` | 30 | 1043 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/env.ts` | 348 | 10943 | 重写：保持通用行为 | `src/generated_ports/p_1580d3dd08c1e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/envDynamic.ts` | 152 | 5278 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/envUtils.ts` | 184 | 6246 | 重写：保持通用行为 | `src/env_runtime.rs / src/subprocess_env.rs` | 部分实现：真值与显式假值、KEY=VALUE 解析、启动精简模式、配置目录及区域回退已有 Rust 代码和测试；多模型区域动态配置、全部 CLI 配置调用方仍未等价 |
| `src/utils/envValidation.ts` | 39 | 1045 | 重写：保持通用行为 | `src/env_validation.rs` | 部分实现：decimal-prefix parseInt 和上下限校验；超大整数及 JS 数值精度仍需核对 |
| `src/utils/errorLogSink.ts` | 236 | 6557 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/errors.ts` | 239 | 7655 | 重写：保持通用行为 | `src/generated_ports/p_bf458f64de43d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/exampleCommands.ts` | 185 | 6200 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/execFileNoThrow.ts` | 151 | 4329 | 重写：保持通用行为 | `src/exec_file.rs / src/mcp_http.rs / src/portable_configuration.rs` | 部分实现：argv、cwd/env、stdin/EOF、stdout/stderr并发读取、失败输出策略、非零退出码、取消/超时与上限已在MCP凭据和gh认证真实链路接入；Windows批处理、shell参数及全部下游调用需继续验收 |
| `src/utils/execFileNoThrowPortable.ts` | 90 | 2685 | 重写：保持通用行为 | `src/exec_sync_compat.rs / src/portable_configuration.rs` | 部分实现：默认十分钟超时、Shell 执行、cwd/env/stdin、非零退出保留stdout并trim/null、标准流选项、输出缓冲上限、超时与取消的 Rust 同步兼容层；IDE/钥匙串等原版跨模块调用及平台差异仍待完整接线和对照 |
| `src/utils/execSyncWrapper.ts` | 39 | 1203 | 重写：保持通用行为 | `src/portable_configuration.rs / src/exec_sync_compat.rs` | 部分实现：统一执行器已实现同步 Shell、cwd/env/stdin、退出码、输出、超时/取消；严格的非零退出错误与宽松非抛出接口分离；所有业务入口与跨平台权限验收待补齐 |
| `src/utils/exportRenderer.tsx` | 98 | 16635 | 重写：保持通用行为 | `src/session_control.rs` | 部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| `src/utils/extraUsage.ts` | 24 | 635 | 替换：独立模型／访问管理 | `src/provider.rs / src/config.rs` | 部分覆盖，原路径仍需逐条核对 |
| `src/utils/fastMode.ts` | 533 | 17723 | 重写：保持通用行为 | `src/generated_ports/p_9a3112a5d3add.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/file.ts` | 585 | 18256 | 重写：保持通用行为 | `src/generated_ports/p_0ff2be80aab56.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/fileHistory.ts` | 1116 | 34660 | 重写：保持通用行为 | `src/history.rs / src/tools.rs / src/agent.rs / src/config.rs` | 部分实现：本地检查点与内容校验回退；按原版备份恢复原 Unix 文件权限，若回退前权限已被外部修改则拒绝覆盖（2a0f929b、0f3e5461）；按原版会话 ID 隔离检查点创建/查询/回退（28b82752、1b90480a）；交互配置 fileCheckpointingEnabled 与 SDK 显式 opt-in/全局禁用开关接入真实 Write/Edit/NotebookEdit 工具链（b63e2803、af837473、d8a79bb8、68f00be1），关闭文件历史时模型工具目录隐藏回退入口，会话目录拒绝符号链接（8586562f、32d8873c、dc46b1bb）；按原版恢复已删除文件并重建父目录，已删除的新文件回退幂等（95be4266）。源码已提交待验收；无损二进制及大文件独立备份、旧记录兼容恢复、检查点时间排序和损坏记录显式错误已接入（0f1c5ff9、06c700ce）；Write/Edit/NotebookEdit 修改前历史备份读取失败仍执行文件操作并明确告警，成功后历史落盘失败也不误报编辑失败（3ba0c262、771c1f45、e8823bcc、c582d31e、06ae7637），源码已提交待验收。新增原版回退方向差异预览与 Read 权限接线（ecd0dfca、54c9aa15），以及备份原始字节 SHA-256 校验、损坏备份回退前明确拒绝（ecd0dfca），均已提交待验收；预览当前按单文件检查点，尚缺原版消息级聚合。分支会话复制原会话检查点与二进制备份、优先硬链接/跨设备复制、失败回滚已接入真实 Agent（5e576b84、90879015），源码已提交待验收；原版日志恢复及消息级快照仍缺。仍缺原版按消息 ID 快照、100 次保留上限、Bash 修改跟踪、完整跨会话恢复、竞态与跨平台权限行为 |；回退工具现在在恢复磁盘前检查 Edit(path) 明确拒绝及询问规则（70d0bd1e，已提交待验收）；按消息快照与并发恢复仍缺；历史备份改存用户配置目录 file-history/<session>，旧工作区检查点与二进制备份通过临时复制、同步、同字节断点恢复后迁移清理，Unix 私有目录权限 0700（53add254、02ba3fe4），源码已提交待行为验收；消息级快照及100次保留上限仍缺 |
| `src/utils/fileOperationAnalytics.ts` | 72 | 2287 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/filePersistence/filePersistence.ts` | 288 | 7763 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/filePersistence/outputsScanner.ts` | 127 | 3682 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/fileRead.ts` | 103 | 3168 | 重写：保持通用行为 | `src/generated_ports/p_b11d1e054e4a3.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/fileReadCache.ts` | 97 | 2432 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/fileStateCache.ts` | 143 | 4175 | 重写：保持通用行为 | `src/tools.rs::FileReadCache`（实际执行） / `src/generated_ports/p_199ff998619e9.rs`（静态生成不计数） | 部分实现：实际工具读取缓存已实现 100 项/25MiB LRU；修复大于 25MiB 的新读取被拒绝缓存时旧同路径完整读取仍残留、可能错误授权后续 Edit/Write 的问题，现删除旧缓存并同步回收容量及 LRU 位置（b28fbb44），已提交待行为验收；原版克隆/合并缓存、恢复序列化及所有状态时序仍待核对 |
| `src/utils/findExecutable.ts` | 18 | 553 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/fingerprint.ts` | 77 | 2248 | 重写：保持通用行为 | `src/generated_ports/p_2cc5445fbf7a6.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/forkedAgent.ts` | 690 | 24635 | 重写：保持通用行为 | `src/generated_ports/p_c67576a3324b2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/format.ts` | 309 | 9375 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/formatBriefTimestamp.ts` | 82 | 2223 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/fpsTracker.ts` | 48 | 1299 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/frontmatterParser.ts` | 371 | 12404 | 重写：保持通用行为 | `src/generated_ports/p_2e571c4a5ca80.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/fsOperations.ts` | 771 | 24230 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/fullscreen.ts` | 203 | 8632 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/generatedFiles.ts` | 137 | 3481 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/generators.ts` | 89 | 2156 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/genericProcessUtils.ts` | 185 | 6403 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/getWorktreePaths.ts` | 71 | 2048 | 重写：Git 工作树隔离 | `src/worktree.rs` | 部分实现：创建、变更保留与清理测试通过 |
| `src/utils/getWorktreePathsPortable.ts` | 28 | 847 | 重写：Git 工作树隔离 | `src/worktree.rs` | 部分实现：创建、变更保留与清理测试通过 |
| `src/utils/ghPrStatus.ts` | 107 | 2823 | 重写：保持通用行为 | `src/generated_ports/p_c00507bfc0c68.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/git/gitConfigParser.ts` | 278 | 6645 | 重写：保持通用行为 | `src/git_config_parser.rs / src/main.rs` | 部分实现：原版配置读取、节/子节和键匹配、引号/转义/注释、首值语义及真实磁盘测试；已接入 GitConfig CLI 和仓库识别回退，完整 worktree Git 文件系统调用方待补 |
| `src/utils/git/gitFilesystem.ts` | 700 | 22340 | 重写：保持通用行为 | `src/git_filesystem.rs / src/main.rs / src/detect_repository.rs` | 部分实现：原始 .git 文件/目录、HEAD、普通与 packed refs、worktree common dir、SHA/引用名安全校验、远端及默认分支、浅克隆和工作树计数；Rust 按文件实际变化更新缓存并接入 GitState CLI；原版异步 watchFile 生命周期仍待完整验收 |
| `src/utils/git/gitignore.ts` | 100 | 3200 | 重写：保持通用行为 | `src/git_ignore.rs / src/main.rs` | 部分实现：原版 git check-ignore 本地/全局规则优先级、非仓库处理、全局忽略路径和幂等追加；已接入 GitIgnore CLI，项目本地设置自动调用及插件动态技能发现待补 |
| `src/utils/git.ts` | 927 | 30270 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/gitDiff.ts` | 533 | 16039 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/gitSettings.ts` | 19 | 838 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/github/ghAuthStatus.ts` | 30 | 942 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/githubRepoPathMapping.ts` | 163 | 5131 | 重写：保持通用行为 | `src/github_repo_mapping.rs / src/config.rs / src/main.rs` | 部分实现：真实 Git 根目录记录、大小写归一、MRU 提升、本地有效克隆过滤、移除和深链回退已实现；与原版全部启动环境调用链未完整对照 |
| `src/utils/glob.ts` | 131 | 4518 | 重写：保持通用行为 | `src/search.rs::glob_files` | 部分实现：按原版 `glob()` 发现文件，保留隐藏/忽略开关，修复 VCS/插件/构建目录误排除及按修改时间旧到新排序（47684bbc、b734011b），源码已提交待验收；插件缓存排除、动态限制和取消等仍未等价 |；新增获准目录下的单文件 Read 规则过滤并接入真实 Glob 调用链（93fa1d9、7c5a8dd），源码已提交待验收；绝对通配符现按原版解析静态搜索根，经过额外工作区授权和真实 Read 审批后执行（814bb0d、33dc985），源码已提交待验收；特殊平台路径仍待验收；新增与原版 ripGrep 一致的 Glob 搜索时间预算：默认 20 秒，WSL 60 秒，可由 ATLAS_GLOB_TIMEOUT_SECONDS 正整数覆盖，遍历超时返回明确错误，不把未扫完的文件列表当成完整结果（17a8e25e，源码已提交待验收）；子进程级中止/完整忽略规则仍缺；Windows 正斜杠绝对通配符可按原版提取搜索根并进入授权/Glob 执行链（b7a54b76，已提交待验收）；UNC 和驱动器边界仍待验证；GlobTool 否定模式 !pattern 现排除匹配文件，保留原版 ripgrep --glob 语义（8510c3d2，源码已提交待验收）；复杂模式仍待验证；增加 .ignore/.rgignore 多层目录规则、反向规则及高于 Git 忽略的优先级，Grep 与可选忽略模式 Glob 共同接入（6b3d3292、55328d32），源码已提交待验收；复杂 gitignore 目录恢复、全局忽略和外部取消仍待核对
| `src/utils/gracefulShutdown.ts` | 530 | 20088 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/groupToolUses.ts` | 183 | 5537 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/handlePromptSubmit.ts` | 611 | 21712 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/hash.ts` | 47 | 1683 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/headlessProfiler.ts` | 179 | 6058 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/heapDumpService.ts` | 304 | 9890 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/heatmap.ts` | 199 | 5305 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/highlightMatch.tsx` | 28 | 4091 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/hooks/AsyncHookRegistry.ts` | 310 | 8913 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/apiQueryHookHelper.ts` | 142 | 4383 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/execAgentHook.ts` | 340 | 12485 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，未覆盖其余全部子类型 |
| `src/utils/hooks/execHttpHook.ts` | 243 | 8871 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，未覆盖其余全部子类型 |
| `src/utils/hooks/execPromptHook.ts` | 212 | 6822 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，未覆盖其余全部子类型 |
| `src/utils/hooks/fileChangedWatcher.ts` | 192 | 5309 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/hookEvents.ts` | 193 | 4492 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，未覆盖其余全部子类型 |
| `src/utils/hooks/hookHelpers.ts` | 84 | 2521 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/hooksConfigManager.ts` | 401 | 17497 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/hooksConfigSnapshot.ts` | 134 | 5064 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/hooksSettings.ts` | 272 | 8506 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/postSamplingHooks.ts` | 71 | 1993 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/registerFrontmatterHooks.ts` | 68 | 2277 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/registerSkillHooks.ts` | 65 | 2059 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/sessionHooks.ts` | 448 | 12132 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，未覆盖其余全部子类型 |
| `src/utils/hooks/skillImprovement.ts` | 268 | 8362 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks/ssrfGuard.ts` | 295 | 8732 | 重写：Hooks 事件与生命周期 | `src/hooks.rs` | 部分实现，复杂子类型仍未迁移 |
| `src/utils/hooks.ts` | 5023 | 159458 | 重写：Hooks 事件与生命周期 | `src/hooks.rs / src/agent.rs` |部分实现，复杂子类型仍未迁移；SubagentStart/Stop、TeammateIdle Rust 实际代理链与测试已补，PostToolUse MCP 结果覆盖已进入真实模型调用链，未覆盖其余全部子类型；对照原版 2.1.88 对 hookSpecificOutput.hookEventName 强制匹配当前事件，缺失/错事件不能修改工具输入或权限；SessionStart 初始用户消息解析及有序汇总已提交待验收（1311f062）；完整并行 Hook、消息/中止/异步时序仍待验收；队友停止、任务完成与空闲 Hook 非阻断错误在终端/SDK 可见（4e058e87），停止原因事件已接线，待验收 |
| `src/utils/horizontalScroll.ts` | 138 | 4302 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/http.ts` | 137 | 4928 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/hyperlink.ts` | 40 | 1465 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/iTermBackup.ts` | 74 | 1608 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ide.ts` | 1495 | 46585 | 重写：保持通用行为 | `src/generated_ports/p_66657bb564f3f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/idePathConversion.ts` | 91 | 2602 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/idleTimeout.ts` | 54 | 1574 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/imagePaste.ts` | 417 | 14509 | 重写：保持通用行为 | `src/generated_ports/p_9d14b23b8f34e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/imageResizer.ts` | 881 | 26699 | 重写：保持通用行为 | `src/generated_ports/p_f63750a1bb1d8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/imageStore.ts` | 168 | 4320 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/imageValidation.ts` | 105 | 3619 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/immediateCommand.ts` | 16 | 547 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/inProcessTeammateHelpers.ts` | 103 | 2988 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ink.ts` | 27 | 921 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/intl.ts` | 95 | 2824 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/jetbrains.ts` | 192 | 5805 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/json.ts` | 278 | 9144 | 重写：保持通用行为 | `src/json_runtime.rs / src/keybindings.rs / src/session.rs` | 部分实现：已实现 BOM 安全 JSON、50 条限额缓存、JSONC 注释/尾逗号解析、保留注释的数组追加、容错 JSONL 与 100MB 文件尾部读取，并接入快捷键配置和会话加载；终端设置、其它调用链及源版边界行为仍待完整验收 |
| `src/utils/jsonRead.ts` | 17 | 657 | 重写：处理 UTF-8 BOM 的 JSON 读取 | `src/json_read.rs` / `src/config.rs` / `src/session.rs` | 已验收：前缀 U+FEFF 精确移除一次、空输入、非前缀保留、跨平台配置和会话读取；证据 json_read::tests / config::tests / session::tests |
| `src/utils/keyboardShortcuts.ts` | 15 | 568 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/lazySchema.ts` | 9 | 295 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/listSessionsImpl.ts` | 455 | 15071 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/localInstaller.ts` | 163 | 4810 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/lockfile.ts` | 44 | 1330 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/log.ts` | 363 | 11693 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/logoV2Utils.ts` | 351 | 9835 | 重写：保持通用行为 | `src/generated_ports/p_22d85b8e4d967.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/mailbox.ts` | 74 | 1596 | 重写：保持通用行为 | `src/in_memory_mailbox.rs / src/event_signal.rs` | 部分实现：原始消息队列、谓词匹配、等待者优先、即时登记、FIFO、revision 与取消恢复；原 JS 同步异常边界仍待对照 |
| `src/utils/managedEnv.ts` | 200 | 7908 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/managedEnvConstants.ts` | 192 | 6813 | 重写：保持通用行为 | `src/generated_ports/p_6f80998bb4902.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/markdown.ts` | 382 | 11853 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/markdownConfigLoader.ts` | 601 | 21312 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/mcp/dateTimeParser.ts` | 122 | 4322 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/mcp/elicitationValidation.ts` | 337 | 9384 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/mcpInstructionsDelta.ts` | 131 | 4752 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/mcpOutputStorage.ts` | 190 | 7086 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/mcpValidation.ts` | 209 | 6300 | 重写：保持通用行为 | `src/generated_ports/p_6287d1dbedede.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/mcpWebSocketTransport.ts` | 201 | 6052 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/memoize.ts` | 270 | 8612 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/memory/types.ts` | 13 | 264 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/memory/versions.ts` | 9 | 307 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/memoryFileDetection.ts` | 290 | 10212 | 重写：保持通用行为 | `src/generated_ports/p_e22a2930459fb.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/messagePredicates.ts` | 9 | 427 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/messageQueueManager.ts` | 548 | 16563 | 重写：队列优先级、消息身份、可编辑恢复与 SDK 处理 | `src/message_queue.rs` / `src/sdk_queue.rs` / `src/main.rs` | 部分实现：优先级 FIFO、过滤移除、代理隔离、图片引用、UTF-16 光标和真实 SDK 消费；Rust 订阅与频道可见性已有，React UI 同步、通知持久化和多模态直接模型发送仍待对照；SDK 取消按原始 UUID 移除全部等待命令，重复请求幂等 |
| `src/utils/messages/mappers.ts` | 291 | 9031 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/messages/systemInit.ts` | 97 | 3747 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/messages.ts` | 5513 | 193203 | 重写：保持通用行为 | `src/generated_ports/p_8882c41a85e4f.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/model/agent.ts` | 158 | 5578 | 重写：保持通用行为 | `src/generated_ports/p_64a95397c4c34.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/model/aliases.ts` | 26 | 793 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/model/antModels.ts` | 65 | 1798 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/bedrock.ts` | 266 | 9196 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/check1mAccess.ts` | 73 | 2215 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/configs.ts` | 119 | 4286 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/contextWindowUpgradeCheck.ts` | 48 | 1284 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/model/deprecation.ts` | 102 | 2534 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/model.ts` | 619 | 21409 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/modelAllowlist.ts` | 171 | 6030 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/modelCapabilities.ts` | 119 | 4099 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/modelOptions.ts` | 541 | 18338 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/modelStrings.ts` | 167 | 5233 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/model/modelSupportOverrides.ts` | 51 | 1537 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/model/providers.ts` | 41 | 1340 | 重写：保持通用行为 | `src/generated_ports/p_dad63e8ffdc72.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/model/validateModel.ts` | 160 | 4633 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/modelCost.ts` | 232 | 7520 | 重写：多模型价格计算 | `src/cost.rs` / `src/config.rs` | 部分实现：五类可配置费率、按服务区分缓存是否计入输入 Token、未知价格提示及模型价格格式；原厂价格常量不保留 |
| `src/utils/modifiers.ts` | 37 | 1112 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/mtls.ts` | 180 | 4655 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/nativeInstaller/download.ts` | 524 | 15191 | 重写：保持通用行为 | `src/generated_ports/p_be04dcc4b58ca.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/nativeInstaller/index.ts` | 19 | 456 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/nativeInstaller/installer.ts` | 1709 | 54752 | 重写：保持通用行为 | `src/generated_ports/p_878997a490a39.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/nativeInstaller/packageManagers.ts` | 337 | 8963 | 重写：保持通用行为 | `src/generated_ports/p_08385137b9b82.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/nativeInstaller/pidLock.ts` | 434 | 11934 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/notebook.ts` | 225 | 6368 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/objectGroupBy.ts` | 19 | 511 | 重写：迭代分组、索引与防原型污染 | `src/collections.rs` | 部分实现：逐项回调、零起始索引、键分组和 __proto__ 安全；JS Symbol/整数属性枚举次序及对象标识待核对 |
| `src/utils/pasteStore.ts` | 105 | 2950 | 重写：保持通用行为 | `src/prompt_history.rs` | 部分实现：SHA-256 内容寻址、长粘贴持久化/恢复与过期清理；图像缓存及 UI 生命周期仍待验证 |
| `src/utils/path.ts` | 156 | 5700 | 重写：保持通用行为 | `src/path_support.rs / src/workspace_dirs.rs` | 部分实现：路径展开、NFC、配置键、目录识别、相对路径、穿越检测及文件名清洗已落入真实工作区校验链；Windows 原生语义、旧会话路径兼容与其它下游入口待验收 |
| `src/utils/pdf.ts` | 301 | 8148 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/pdfUtils.ts` | 71 | 2190 | 重写：保持通用行为 | `src/platform_source_utilities.rs / src/main.rs` | 部分实现：按原始 JS parseInt 前缀、页码单值/区间/开区间、文档扩展名大小写、模型 PDF 支持边界实现并接入 inspect-source；原版全部调用链待完整移植 |
| `src/utils/peerAddress.ts` | 22 | 981 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/permissions/PermissionMode.ts` | 142 | 3489 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/PermissionPromptToolResultSchema.ts` | 128 | 4108 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/PermissionResult.ts` | 36 | 874 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/PermissionRule.ts` | 41 | 1176 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/PermissionUpdate.ts` | 390 | 11912 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/PermissionUpdateSchema.ts` | 79 | 2402 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/autoModeState.ts` | 40 | 1095 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/bashClassifier.ts` | 62 | 1444 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/bypassPermissionsKillswitch.ts` | 156 | 4839 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/classifierDecision.ts` | 99 | 4585 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/classifierShared.ts` | 40 | 1174 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/dangerousPatterns.ts` | 81 | 2476 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/denialTracking.ts` | 46 | 1101 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/filesystem.ts` | 1778 | 62254 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价；Grep/Glob 现逐文件执行 Read 权限规则过滤，避免仅检查搜索根目录时泄露被拒绝路径（93fa1d9、7c5a8dd），已提交待验收；完整权限策略仍待等价；文件历史回退的路径写权限现在复用 Edit 原始/规范化路径规则，防止绕过显式拒绝（70d0bd1e，已提交待验收）；完整权限策略仍缺；同项目跨会话临时任务输出 Read 已按原版允许且校验 UUID 会话/任务文件身份（9f2b353e），源码已提交待验收 |
| `src/utils/permissions/getNextPermissionMode.ts` | 102 | 3301 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/pathValidation.ts` | 486 | 16249 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/permissionExplainer.ts` | 251 | 7606 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/permissionRuleParser.ts` | 199 | 7274 | 重写：权限判定 | `src/permission_rules.rs` | 部分实现：已移植原始括号/反斜杠规则解析、旧工具名和规则编码；完整配置优先级仍待对照 |
| `src/utils/permissions/permissionSetup.ts` | 1533 | 53439 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/permissions.ts` | 1487 | 52190 | 重写：权限判定 | `src/permission_rules.rs / src/tools.rs / src/agent.rs` | 部分实现：用户明确配置规则的 deny/ask/allow、主子 Agent 继承、只读保护和 Shell 真实测试；原版分类器、模式切换及全部权限来源仍未等价 ；新增按原版 toolMatchesRule 对 mcp__server / mcp__server__* 的跨工具规则与冲突优先级校验 ；WebFetch(domain:hostname) 内容规则与拒绝优先级按原版精确主机名比较，其他权限来源未等价 |
| `src/utils/permissions/permissionsLoader.ts` | 297 | 8743 | 重写：权限判定 | `src/permission_settings.rs / src/main.rs / src/permission_rules.rs` | 部分实现：用户/项目/本地/配置/托管规则加载、托管独占、持久化增删、CLI 实时重载及并发文件锁；远程/MDM/注册表、来源开关、完整设置 UI/热更新及异常时序未等价 |
| `src/utils/permissions/shadowedRuleDetection.ts` | 235 | 8050 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/permissions/shellRuleMatching.ts` | 229 | 6409 | 重写：权限判定 | `src/permission_rules.rs` | 部分实现：精确、legacy :* 前缀、通配符及转义星号规则；已对照原版 BashTool 的复合命令授权边界并加入逐段匹配与异常回归；完整 Shell AST、具体适配与大小写行为尚未等价 |
| `src/utils/permissions/yoloClassifier.ts` | 1496 | 52160 | 重写：权限判定 | `src/tools.rs / src/hooks.rs` | 部分实现，规则与分类未等价 |
| `src/utils/planModeV2.ts` | 96 | 3051 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plans.ts` | 398 | 12390 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/platform.ts` | 151 | 3801 | 重写：保持通用行为 | `src/generated_ports/p_dbebe9c759444.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/addDirPluginSettings.ts` | 72 | 2320 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/cacheUtils.ts` | 197 | 6652 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/dependencyResolver.ts` | 306 | 11673 | 重写：插件依赖闭包及会话隔离 | `src/plugins.rs / src/plugin_manager.rs` | 部分实现：已按原版 verifyAndDemote 补齐已启用插件依赖的跨市场身份检查、缺失依赖固定点级联停用，覆盖 Hooks、MCP、技能发现/读取和斜杠命令入口；cab6e0fc，源码已提交待验收。安装时依赖闭包自动安装、跨市场信任白名单、完整动态刷新/错误分类和持久化恢复仍未等价 |
| `src/utils/plugins/fetchTelemetry.ts` | 136 | 4922 | 重写：保持通用行为 | `src/generated_ports/p_c16d3bc443a4b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/gitAvailability.ts` | 70 | 2273 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/plugins/headlessPluginInstall.ts` | 175 | 6775 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/hintRecommendation.ts` | 165 | 5431 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/installCounts.ts` | 293 | 8315 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/installedPluginsManager.ts` | 1269 | 41410 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/loadPluginAgents.ts` | 349 | 12485 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/loadPluginCommands.ts` | 947 | 30541 | 重写：保持通用行为 | `src/plugins.rs / src/main.rs / src/skill_arguments.rs` | 部分实现：插件 Markdown 斜杠命令目录/命名空间/参数替换经 src/plugins.rs → src/main.rs → Agent::ask 实际接线（7e2c295、a6e148a），frontmatter、内嵌 Shell、SDK 纯文本非排队命令已接线（3476d97），SDK 排队/图文混合及动态刷新仍缺，待验收；可执行插件命令同步显示于交互式 /commands，复用真实命令解析入口（848b90d、dfc0dca、3f3f10e），同功能不重复计数，待验收；新增用户可调用前置限制、插件根路径及交互会话 ID 变量展开，已提交待验收；插件 DATA 持久目录按需创建已接线；插件技能 SKILL.md 已接入交互斜杠命令与 /commands（545e01c、e4043c4），隐藏技能过滤，源码待验收；用户配置变量、内嵌 Shell/权限、SDK 与动态刷新仍缺；SDK 插件命令和嵌套技能的 description/argument-hint 从真实 Markdown 读取（2da7e57、ffd1056），待行为验收；用户配置敏感值、内嵌 Shell、动态刷新与完整 SDK 仍缺；交互式 /commands 现展示插件命令原始描述和参数提示（629f319），源码待验收；插件命令用户配置占位符已接通用户设置并按 manifest.userConfig.sensitive 遮蔽敏感值（0b29f0a），源码待验收；内嵌 Shell/权限与配置热重载仍缺 |
| `src/utils/plugins/loadPluginHooks.ts` | 288 | 10066 | 重写：保持通用行为 | `src/plugins.rs / src/hooks.rs` | 部分实现：显式启用插件 Hook；全部原版事件待核对 |
| `src/utils/plugins/loadPluginOutputStyles.ts` | 179 | 5672 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/lspPluginIntegration.ts` | 388 | 12414 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/lspRecommendation.ts` | 375 | 10695 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/managedPlugins.ts` | 28 | 877 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/plugins/marketplaceHelpers.ts` | 593 | 18217 | 重写：保持通用行为 | `src/generated_ports/p_191decb55832d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/marketplaceManager.ts` | 2644 | 93274 | 重写：保持通用行为 | `src/marketplace_registry.rs` | 部分实现：本地市场注册/列表/移除/刷新和持久化已接线；市场注册表与锁文件拒绝符号链接重定向（b9027b6、80e3be9），源码待验收；远程市场下载、版本解析、插件依赖闭包及完整原版管理流程仍未迁移 |
| `src/utils/plugins/mcpPluginIntegration.ts` | 635 | 20113 | 重写：保持通用行为 | `src/plugins.rs / src/mcp.rs` | 部分实现：插件 MCP 加载；工具兼容及认证功能未等价；插件目录 .mcp.json 与清单 mcpServers 读取、覆盖与循环引用防护已接入 src/plugins.rs（df08c7a、03e4587、328cc96），MCPB、环境展开及容错仍缺，待验收 ；同插件 MCP 服务无效配置逐条报错且保留有效服务（fb0a90b），已提交待验收，MCPB/完整认证仍缺 |
| `src/utils/plugins/mcpbHandler.ts` | 969 | 31289 | 重写：保持通用行为 | `src/generated_ports/p_ee9950d29aa12.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/officialMarketplace.ts` | 26 | 836 | 重写：保持通用行为 | `src/portable_configuration.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/plugins/officialMarketplaceGcs.ts` | 217 | 9338 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/officialMarketplaceStartupCheck.ts` | 440 | 15192 | 重写：保持通用行为 | `src/generated_ports/p_bbc95ae0a3971.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/orphanedPluginFilter.ts` | 115 | 3980 | 重写：过期插件缓存过滤 | `src/search.rs` | 部分实现：按原版 `.orphaned_at` 版本标记，在真实 Glob/Grep 目录遍历前排除已过期的插件版本，保留健康版本（952cf33f、5218a04f，源码已提交待验收）；仍缺原版会话级缓存冻结、插件重载时刷新和全部外部缓存路径处理 |
| `src/utils/plugins/parseMarketplaceInput.ts` | 163 | 6075 | 重写：保持通用行为 | `src/marketplace_source.rs / src/marketplace_registry.rs / src/plugin_manager.rs` | 部分实现：解析 Git SSH、GitHub 简写、HTTP(S)、Azure DevOps 与本地文件/目录；本地市场登记、持久管理、刷新及插件安装已接入 CLI，远程下载、信任验证与完整错误边界待验收 |
| `src/utils/plugins/performStartupChecks.tsx` | 70 | 9597 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/pluginAutoupdate.ts` | 285 | 9473 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/pluginBlocklist.ts` | 128 | 4361 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/pluginDirectories.ts` | 179 | 6669 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/pluginFlagging.ts` | 209 | 5621 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/pluginIdentifier.ts` | 124 | 3928 | 重写：保持通用行为 | `src/generated_ports/p_d5db58adb3fd2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/pluginInstallationHelpers.ts` | 596 | 20629 | 重写：保持通用行为 | `src/plugin_manager.rs` | 部分实现：本地插件安装时校验预期 ID、安装即启用、配置失败回滚；市场依赖闭包与原版多范围安装仍未迁移；本地插件安装、列举和卸载新增存储根路径符号链接/越界防护，避免安装写出工作区（2dd6c0b、14b097f），源码待验收；本地安装支持原版标准清单与无清单插件，复制时排除 .git 仓库元数据（6c17c1c、84f2a6e），待验收；并发回滚和多范围安装仍缺 |
| `src/utils/plugins/pluginLoader.ts` | 3303 | 110261 | 重写：保持通用行为 | `src/plugins.rs` | 部分实现：本地启用插件、MCP、Hooks、技能加载；市场与扩展生态未等价；插件技能目录发现与读取统一限制在插件根目录，拒绝跨插件符号链接（e45e418、5b52f8e），待验收；补齐标准 .claude-plugin 清单/无清单加载、hooks/hooks.json 与内联命令 Hook、mcpServers 名称映射，真实 Agent 初始化可达（d8d673a、c554ea7、6c17c1c），待验收；异步/HTTP Hook、完整插件生态仍缺；插件 .mcp.json、mcpServers 文件引用与有序覆盖接入真实 MCP 初始化（df08c7a、03e4587、328cc96），源码待验收；manifest.skills 自定义目录/数组经列表及读取入口接通（687d885、0b146e4），待验收；manifest.commands 默认/自定义目录及内联文件元数据已接入交互式命令执行（7e2c295、a6e148a），待验收 ；原版 allSettled 插件加载失败隔离及同插件 Hook 文件独立容错已接入实际 PluginSet/Agent（2903fca、60953ca），已提交待验收 |
| `src/utils/plugins/pluginOptionsStorage.ts` | 401 | 15302 | 重写：保持通用行为 | `src/permission_settings.rs / src/plugins.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收；已按用户设置 pluginConfigs 读取非敏感插件选项、处理唯一 name@marketplace 来源，敏感字段在模型提示中替换为说明性占位（f49b28c、0b29f0a），源码已提交待验收；安全存储、选项保存/验证及热重载仍缺 |
| `src/utils/plugins/pluginPolicy.ts` | 21 | 827 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/plugins/pluginStartupCheck.ts` | 342 | 11096 | 重写：保持通用行为 | `src/generated_ports/p_6060310ec2cbf.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/pluginVersioning.ts` | 158 | 5340 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/reconciler.ts` | 266 | 8273 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/refresh.ts` | 216 | 8537 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/schemas.ts` | 1682 | 58914 | 重写：保持通用行为 | `src/generated_ports/p_e813a809bd066.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/validatePlugin.ts` | 904 | 28366 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/walkPluginMarkdown.ts` | 70 | 2222 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/plugins/zipCache.ts` | 407 | 13163 | 重写：保持通用行为 | `src/generated_ports/p_bb3d7b51ba520.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/plugins/zipCacheAdapters.ts` | 165 | 5310 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/powershell/dangerousCmdlets.ts` | 186 | 6155 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/powershell/parser.ts` | 1805 | 66647 | 重写：保持通用行为 | `src/generated_ports/p_c4d0f9a46fa38.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/powershell/staticPrefix.ts` | 317 | 12277 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/preflightChecks.tsx` | 151 | 19309 | 重写：保持通用行为 | `src/generated_ports/p_567219a1f8574.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/privacyLevel.ts` | 56 | 1886 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/process.ts` | 69 | 2333 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/processUserInput/processBashCommand.tsx` | 140 | 22279 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/processUserInput/processSlashCommand.tsx` | 922 | 144807 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/processUserInput/processTextPrompt.ts` | 101 | 3272 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/processUserInput/processUserInput.ts` | 606 | 19507 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/profilerBase.ts` | 47 | 1572 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/promptCategory.ts` | 50 | 1502 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/promptEditor.ts` | 189 | 5664 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/promptShellExecution.ts` | 184 | 7011 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/proxy.ts` | 427 | 13545 | 重写：保持通用行为 | `src/generated_ports/p_e0abda6bc6c38.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/queryContext.ts` | 180 | 5937 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/queryHelpers.ts` | 553 | 19734 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/queryProfiler.ts` | 302 | 8894 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/queueProcessor.ts` | 96 | 3174 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/readEditContext.ts` | 228 | 7224 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/readFileInRange.ts` | 384 | 12259 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/releaseNotes.ts` | 361 | 11813 | 重写：保持通用行为 | `src/release_notes.rs` | 部分实现：版本解析排序、最近五条过滤、可信 URL 下载与原子缓存；旧配置迁移仍缺 |
| `src/utils/renderOptions.ts` | 78 | 2278 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ripgrep.ts` | 680 | 21188 | 重写：保持通用行为 | `src/search.rs` | 部分实现：按原版 ripGrep 的默认 20 秒、WSL 60 秒及可配置正整数秒超时，为 Grep/Glob 可达搜索入口增加超时错误和遍历阶段检查（fbd4a626、17a8e25e，源码已提交待验收）；子进程级取消、信号升级、超时抢占、完整忽略规则及其余 ripgrep 执行链仍未迁移；Git 忽略检查子进程共享搜索截止时间，超时强制终止并显式报告错误，异常退出不再静默当成无忽略规则（13a62847，源码已提交待验收）；外部取消与完整 ripgrep 规则仍缺；Grep 大文件逐行及跨行正则扫描增加同一截止时间检查（64627c6e，已提交待验收），仍不能立即抢占单次正则计算 |；Grep 多个正反 glob 现按原版输入顺序由后命中规则覆盖（82b0bcd6，源码已提交待验收）；与 Git ignore 的跨来源覆盖仍缺；Grep content/count 不再被强制按文件时间重排，只有文件名模式按原版新到旧排序（98b6fc42，源码已提交待验收）；ripgrep 默认遍历顺序仍待验收
| `src/utils/sandbox/sandbox-adapter.ts` | 986 | 35710 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sandbox/sandbox-ui-utils.ts` | 13 | 389 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/sanitization.ts` | 92 | 4003 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/screenshotClipboard.ts` | 122 | 3709 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sdkEventQueue.ts` | 135 | 4077 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/secureStorage/fallbackStorage.ts` | 71 | 2362 | 重写：保持通用行为 | `src/secure_fallback.rs / src/main.rs` | 部分实现：主安全存储优先、读取回退、首次主存储成功后删除副本、主失败时避免过期值遮蔽新刷新令牌、同步/异步读和删除已写入并接入凭据命令 |
| `src/utils/secureStorage/index.ts` | 18 | 559 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/secureStorage/keychainPrefetch.ts` | 117 | 4755 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/secureStorage/macOsKeychainHelpers.ts` | 112 | 4797 | 重写：保持通用行为 | `src/generated_ports/p_b2a6ef22565e7.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/secureStorage/macOsKeychainStorage.ts` | 232 | 8271 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/secureStorage/plainTextStorage.ts` | 85 | 2430 | 重写：保持通用行为 | `src/plain_text_storage.rs / src/main.rs` | 部分实现：真实凭据 JSON 文件的同步和异步读取、受限 0600 权限、持久化警告、文件原子替换和幂等删除已在原生 Rust 和 CLI 中实现 |
| `src/utils/semanticBoolean.ts` | 30 | 1167 | 重写：严格布尔字符串转换 | `src/semantic_inputs.rs` / `src/tools.rs` | 部分实现：仅精确 true/false 字符串归一化并接入工具参数；Zod 可选/默认 schema 泛型组合待核对；已接入 Grep -i/-n/multiline 与 PowerShell 后台执行布尔字符串转换，泛型 Zod 仍待对照 |
| `src/utils/semanticNumber.ts` | 37 | 1486 | 重写：严格十进制数字转换 | `src/semantic_inputs.rs` / `src/tools.rs` | 部分实现：十进制词法、非有限拒绝、IEEE-754 数字精度与负零、工具整数参数及 JSON 数值字面量对齐；Zod 泛型输出与全部调用方待核对；已接入 Grep 及 PowerShell 模型数值字段的原版十进制转换，泛型 Zod 仍待对照 |
| `src/utils/semver.ts` | 60 | 1713 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/sequential.ts` | 57 | 1641 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/sessionActivity.ts` | 134 | 4081 | 重写：保持通用行为 | `src/generated_ports/p_31abb3363c332.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/sessionEnvVars.ts` | 23 | 590 | 重写：保持通用行为 | `src/agent.rs / src/tools.rs / src/background.rs / src/main.rs` | 部分实现：会话变量增删查、前台/后台 Shell 和子代理继承、真实 CLI 回归；Hooks/MCP/LSP 完整透传未对照 |
| `src/utils/sessionEnvironment.ts` | 167 | 5053 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sessionFileAccessHooks.ts` | 251 | 8138 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sessionIngressAuth.ts` | 141 | 4737 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sessionRestore.ts` | 552 | 20406 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sessionStart.ts` | 233 | 8151 | 重写：保持通用行为 | `src/agent.rs / src/hooks.rs` | 部分实现：SessionStart 阻断不终止会话，保留上下文和警告（f2d21933）；SessionStart 初始用户消息已由 Hook JSON 进入无头 Agent 首条用户轮次（1311f062、7485c83，源码已提交待验收）；SDK 无外部输入自动启动已接入（1e055ef、974dd67，待验收）；watchPaths、插件动态加载及完整 SDK 队列时序仍缺 |
| `src/utils/sessionState.ts` | 151 | 5309 | 重写：保持通用行为 | `src/generated_ports/p_000102e58d700.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/sessionStorage.ts` | 5106 | 180620 | 重写：保持通用行为 | `src/generated_ports/p_d6a11c5239e9b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/sessionStoragePortable.ts` | 794 | 25448 | 重写：保持通用行为 | `src/generated_ports/p_64e1431d70c86.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/sessionTitle.ts` | 130 | 4757 | 重写：保持通用行为 | `src/session_control.rs / src/agent.rs / src/main.rs / src/session.rs` | 部分实现：extractConversationText 已按原版最近 1000 UTF-16 单元提取用户/助手文本和多模态 text 块并接入 /rename；SDK generate_session_title 已接通通用模型、JSON title、独立 ai_title 持久化和历史显示（f2b7d484），源码已提交待验收；SDK stdin 非阻塞执行、取消、meta/origin 过滤、跨进程标题索引及完整竞态仍未等价 |
| `src/utils/sessionUrl.ts` | 65 | 1672 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/set.ts` | 54 | 1036 | 重写：Set 差集、交集判定、包含关系与有序并集 | `src/collections.rs` | 部分实现：4 个函数保持 Set 插入顺序和空集语义；JS 对象引用与 SameValueZero 非整数数字语义待跨语言核对 |
| `src/utils/settings/allErrors.ts` | 33 | 1257 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/settings/applySettingsChange.ts` | 93 | 3656 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/changeDetector.ts` | 489 | 16384 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/constants.ts` | 203 | 5627 | 重写：保持通用行为 | `src/generated_ports/p_6a186fe197da0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/settings/internalWrites.ts` | 38 | 1380 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/settings/managedPath.ts` | 35 | 1095 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/settings/mdm/constants.ts` | 82 | 2617 | 重写：保持通用行为 | `src/generated_ports/p_b1f9ce74def72.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/settings/mdm/rawRead.ts` | 131 | 4183 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/mdm/settings.ts` | 317 | 10716 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/permissionValidation.ts` | 263 | 8657 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/pluginOnlyPolicy.ts` | 61 | 2405 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/settings/schemaOutput.ts` | 9 | 317 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/settings/settings.ts` | 1016 | 32185 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/settingsCache.ts` | 81 | 2416 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/toolValidationConfig.ts` | 104 | 3100 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/types.ts` | 1149 | 42784 | 重写：保持通用行为 | `src/generated_ports/p_71c09040fae60.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/settings/validateEditTool.ts` | 46 | 1695 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/settings/validation.ts` | 266 | 7951 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/settings/validationTips.ts` | 165 | 5464 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/shell/bashProvider.ts` | 256 | 11002 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/shell/outputLimits.ts` | 15 | 416 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/shell/powershellDetection.ts` | 108 | 3714 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/shell/powershellProvider.ts` | 124 | 5771 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/shell/prefix.ts` | 368 | 11216 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/shell/readOnlyCommandValidation.ts` | 1894 | 68294 | 重写：保持通用行为 | `src/generated_ports/p_d8621b9191802.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/shell/resolveDefaultShell.ts` | 15 | 496 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/shell/shellProvider.ts` | 34 | 955 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/shell/shellToolUtils.ts` | 23 | 1036 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/shell/specPrefix.ts` | 242 | 7905 | 重写：保持通用行为 | `src/generated_ports/p_10d5fd29bc5b5.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/shellConfig.ts` | 168 | 4737 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sideQuery.ts` | 223 | 8184 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/sideQuestion.ts` | 156 | 6133 | 重写：保持通用行为 | `src/side_question.rs / src/agent.rs / src/main.rs` | 部分实现：SDK side_question 异步单轮无工具模型调用，继承主 Agent 最近模型上下文，返回独立控制回执；交互终端 /btw 复用独立问答；源码提交 5b7c58a、174c3ed、21d3014、e165563、a4198c6，待行为验收。原版运行中主 Agent 控制并发、思考/缓存、完整取消、费用和 UI 仍未等价 |
| `src/utils/signal.ts` | 44 | 1447 | 重写：保持通用行为 | `src/event_signal.rs` | 部分实现：事件订阅/注销/去重、清空与 JS Set 实时增删监听迭代已有测试；完整调用方和异常传播仍待核对 |
| `src/utils/sinks.ts` | 17 | 608 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/skills/skillChangeDetector.ts` | 312 | 10033 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/slashCommandParsing.ts` | 61 | 1437 | 重写：斜杠命令解析 | `src/slash_command.rs` / `src/main.rs` | 已验收：完整返回结构、MCP 标记、重复空格、无效开头、UTF-8 前后空白及 CLI /model 接入；证据 slash_command::tests |
| `src/utils/sleep.ts` | 85 | 2882 | 重写：保持通用行为 | `src/sleep_helpers.rs` | 部分实现：中断前及定时中的主动停止、可选择异常拒绝、超时竞赛不取消实际底层任务，配有真实 Tokio 竞争回归；AbortError 类型和 JS unref 细节仍需补齐 |
| `src/utils/sliceAnsi.ts` | 92 | 3343 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/slowOperations.ts` | 287 | 9055 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/standaloneAgent.ts` | 24 | 803 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/startupProfiler.ts` | 195 | 6079 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/staticRender.tsx` | 116 | 12236 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/stats.ts` | 1062 | 33790 | 重写：保持通用行为 | `src/stats.rs / src/session_control.rs / src/session.rs` | 部分实现：迁移本地会话累计、模型 Tokens/费用、分支基线去重与时间范围读取；原版 JSONL 每消息时间聚合、每日活动、连续使用天数与 statsCache 锁/增量策略尚未等价 |
| `src/utils/statsCache.ts` | 435 | 13891 | 重写：保持通用行为 | `src/generated_ports/p_b9afda97bc9a8.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/status.tsx` | 362 | 48635 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/statusNoticeDefinitions.tsx` | 198 | 30638 | 重写：保持通用行为 | `src/generated_ports/p_381af97c9f891.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/statusNoticeHelpers.ts` | 21 | 673 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/stream.ts` | 77 | 1926 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/streamJsonStdoutGuard.ts` | 124 | 3987 | 重写：保持通用行为 | `src/generated_ports/p_39a1e33432530.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/streamlinedTransform.ts` | 202 | 5941 | 重写：保持通用行为 | `src/generated_ports/p_8478b958d159a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/stringUtils.ts` | 236 | 6595 | 重写：保持通用行为 | `src/generated_ports/p_33ab1f755c819.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/subprocessEnv.ts` | 100 | 3975 | 重写：保持通用行为 | `src/subprocess_env.rs / src/tools.rs / src/background.rs / src/hooks.rs / src/mcp.rs / src/lsp.rs` | 部分实现：通用凭据环境隔离，接入 Shell、Git、Hook、MCP、LSP 全部进程入口；上游代理环境尚缺 |
| `src/utils/suggestions/commandSuggestions.ts` | 568 | 18558 | 重写：保持通用行为 | `src/generated_ports/p_955a1746e1a31.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/suggestions/directoryCompletion.ts` | 264 | 7098 | 重写：保持通用行为 | `src/generated_ports/p_d37e245605bfe.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/suggestions/shellHistoryCompletion.ts` | 120 | 3456 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/suggestions/skillUsageTracking.ts` | 56 | 1948 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/suggestions/slackChannelSuggestions.ts` | 210 | 6391 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/It2SetupPrompt.tsx` | 380 | 42721 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/ITermBackend.ts` | 371 | 12927 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/InProcessBackend.ts` | 340 | 10468 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/PaneBackendExecutor.ts` | 355 | 10914 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/TmuxBackend.ts` | 765 | 21491 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/detection.ts` | 129 | 4495 | 重写：保持通用行为 | `src/generated_ports/p_5ac170fd5c1d4.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/swarm/backends/it2Setup.ts` | 246 | 6921 | 重写：保持通用行为 | `src/generated_ports/p_e1045bf52f44a.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/swarm/backends/registry.ts` | 465 | 14791 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/backends/teammateModeSnapshot.ts` | 88 | 2863 | 重写：保持通用行为 | `src/generated_ports/p_8d66e55dbd14b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/swarm/backends/types.ts` | 312 | 9906 | 重写：保持通用行为 | `src/generated_ports/p_a1327fd797f0d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/swarm/constants.ts` | 34 | 1334 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/swarm/inProcessRunner.ts` | 1553 | 53565 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/leaderPermissionBridge.ts` | 55 | 1732 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/permissionSync.ts` | 929 | 26473 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/reconnection.ts` | 120 | 3401 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/spawnInProcess.ts` | 329 | 10241 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/spawnUtils.ts` | 147 | 5202 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/teamHelpers.ts` | 684 | 21385 | 重写：保持通用行为 | `src/team_registry.rs / src/agent.rs / src/task_graph.rs` | 部分实现：团队配置、跨进程锁、成员状态；退出先收回任务并通知再标非活跃，失败保持活跃供重试；原版元数据、权限继承、后端与工作树未等价 |
| `src/utils/swarm/teammateInit.ts` | 130 | 4280 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/teammateLayoutManager.ts` | 108 | 3261 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/swarm/teammateModel.ts` | 11 | 467 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/swarm/teammatePromptAddendum.ts` | 19 | 771 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/systemDirectories.ts` | 75 | 2125 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/systemPrompt.ts` | 124 | 4867 | 重写：保持通用行为 | `src/prompt_runtime.rs / src/agent.rs` | 部分实现：主 system、团队和内置子 Agent 角色动态拼装及自定义 Agent 提示词已接入真实模型请求；原版全量优先级/缓存/门控尚缺 |
| `src/utils/systemPromptType.ts` | 15 | 382 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/systemTheme.ts` | 120 | 4231 | 重写：保持通用行为 | `src/generated_ports/p_ffaf2e1bb158c.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/taggedId.ts` | 55 | 1573 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/task/TaskOutput.ts` | 391 | 12474 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/task/diskOutput.ts` | 452 | 13568 | 重写：保持通用行为 | `src/background.rs` | 部分实现：磁盘日志上限、原版 b 前缀短 ID 与旧 UUID 输出路径兼容、输出缺失时仍返回任务状态及空日志；新增会话级隔离及安全原子元数据读写，源码已提交待验收；后台 Shell 输出现按原版迁出项目至用户/项目/会话隔离的私有临时目录（778aaad2、fb9ea960、b80f3350，源码已提交待验收）；已有旧会话保留原目录可读；显式大于 8MiB 的任务增量读取不再被额外截断（09201860，源码已提交待验收）；跨进程恢复、旧记录迁移、输出驱逐策略与完整异步生命周期仍未等价；本会话任务输出 Read 授权与增量结果路径已接线（b40a1daa、46319cca、98777615），源码已提交待验收 ；同项目跨会话任务日志 Read 已接线（9f2b353e），源码已提交待验收 |
| `src/utils/task/framework.ts` | 309 | 9869 | 重写：保持通用行为 | `src/background.rs / src/task.rs` | 部分实现：任务状态与后台状态持久化；新增批量退出取消的共享等待期限，源码已提交待验收；通知队列及通用生命周期未等价 |
| `src/utils/task/outputFormatting.ts` | 39 | 1188 | 重写：保持通用行为 | `src/task_output_format.rs` | 部分实现：长度上限、UTF-16 计数、尾部截断和日志路径；代理项边界与任务目录待核对 |
| `src/utils/task/sdkProgress.ts` | 37 | 1153 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/tasks.ts` | 863 | 26354 | 重写：保持通用行为 | `src/task_graph.rs / src/task_events.rs / src/agent.rs` | 部分实现：CRUD、依赖/认领、高水位、重置、退出收回、订阅回调锁外派发；已补齐损坏/超长任务文件名、不可读/超大任务记录隔离、异常 JSON 重置清理、创建仅落任务记录、删除/重置时更新高水位，避免额外计数写失败误报；损坏记录占用编号不复用、无效编号读取返回任务不存在；新增 ATLAS_TASK_LIST_ID 跨会话共享任务列表优先级及团队清理隔离；原版全部状态迁移、跨进程通知与文件级并发仍未等价；队友空闲前从共享任务列表逐条读取本人 in_progress 任务并执行 TaskCompleted Hook（dd67ffc1），待验收；修复同一任务双向自依赖持久化时第二次写入覆盖阻塞关系（4f4cf7b） |
| `src/utils/teamDiscovery.ts` | 82 | 2323 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teamMemoryOps.ts` | 89 | 2471 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teammate.ts` | 293 | 9206 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teammateContext.ts` | 97 | 3161 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teammateMailbox.ts` | 1184 | 33420 | 重写：保持通用行为 | `src/team_mailbox.rs / src/team_registry.rs / src/agent.rs` | 部分实现：持久邮箱、并发锁、未读/条件已读、关停/计划请求与回执、模式设置通知；请求关联、故障重试、来源校验；权限/沙箱审批与其他通知的完整分发、原版异常语义未等价 |
| `src/utils/telemetry/betaSessionTracing.ts` | 492 | 15843 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/bigqueryExporter.ts` | 253 | 7806 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/events.ts` | 76 | 2287 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/instrumentation.ts` | 826 | 26741 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/logger.ts` | 27 | 742 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/perfettoTracing.ts` | 1121 | 29792 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/pluginTelemetry.ts` | 290 | 10491 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/sessionTracing.ts` | 928 | 27991 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/skillLoadedEvent.ts` | 40 | 1425 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetryAttributes.ts` | 72 | 2095 | 排除：行为／环境遥测回传 | — | 按用户要求删除；不重写外发通道 |
| `src/utils/teleport/api.ts` | 467 | 13327 | 重写：保持通用行为 | `src/generated_ports/p_1ae7384350177.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/teleport/environmentSelection.ts` | 78 | 2695 | 重写：保持通用行为 | `src/portable_ui_runtime.rs` | 部分实现：Rust 领域逻辑已编写，展示/SDK 或来源读取端还需统一验收 |
| `src/utils/teleport/environments.ts` | 121 | 3471 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teleport/gitBundle.ts` | 293 | 9820 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/teleport.tsx` | 1226 | 175779 | 重写：保持通用行为 | `src/generated_ports/p_b9c0fcb163e64.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/tempfile.ts` | 32 | 1170 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/terminal.ts` | 132 | 4372 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/terminalPanel.ts` | 192 | 6014 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/textHighlighting.ts` | 167 | 4540 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/theme.ts` | 640 | 26830 | 重写：保持通用行为 | `src/generated_ports/p_0f699233d6b14.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/thinking.ts` | 163 | 5514 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/timeouts.ts` | 40 | 1410 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/tmuxSocket.ts` | 428 | 13693 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/todo/types.ts` | 19 | 602 | 重写：保持通用行为 | `src/portable_settings_runtime.rs / src/task.rs` | 部分实现：TodoItem content/status/activeForm 实际序列化与输入校验已接无交互 Agent，全部边界待行为验收 |
| `src/utils/tokenBudget.ts` | 74 | 2675 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/tokens.ts` | 262 | 9606 | 重写：保持通用行为 | `src/generated_ports/p_faf3dbb033a1d.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/toolErrors.ts` | 133 | 4012 | 重写：保持通用行为 | `src/tool_errors.rs / src/tools.rs` | 部分实现：按原版补充错误头尾保留、Shell 错误字段、JSON 参数校验提示，接入真实工具执行错误输出；原版 Zod Schema 输入与 AbortError 类型传播仍待验收 |
| `src/utils/toolPool.ts` | 80 | 3136 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/toolResultStorage.ts` | 1041 | 38384 | 重写：保持通用行为 | `src/generated_ports/p_8aa6e76dd0af2.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/toolSchemaCache.ts` | 27 | 1061 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/toolSearch.ts` | 757 | 26606 | 重写：保持通用行为 | `src/generated_ports/p_82c13c954be66.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/transcriptSearch.ts` | 203 | 8039 | 重写：会话可见文本检索（通用化） | `src/transcript_search.rs / src/agent.rs / src/main.rs` | 部分实现：依原版实现用户/助手/工具/附件可见文本抽取、隐藏提醒剔除、大小写归一与缓存索引；已接交互式 /find、离线 transcript-search、inspect-source 和真实 CLI 测试；原版 React/Ink 搜索交互尚未验收 |
| `src/utils/treeify.ts` | 171 | 5033 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/truncate.ts` | 180 | 5719 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/ultraplan/ccrSession.ts` | 350 | 12987 | 重写：保持通用行为 | `src/generated_ports/p_322c9d928a344.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/ultraplan/keyword.ts` | 128 | 4691 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/unaryLogging.ts` | 40 | 1254 | 重写：保持通用行为 | `src/generated_ports/p_5386fc6ccfc1e.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/undercover.ts` | 90 | 3681 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/user.ts` | 195 | 5714 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/userAgent.ts` | 11 | 281 | 重写：保持通用行为 | `src/portable_missing_utilities.rs` | 部分实现：按原始源码编写真实 Rust 函数/领域状态并配用例，原厂外部依赖及全部调用链待统一验收 |
| `src/utils/userPromptKeywords.ts` | 28 | 929 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/uuid.ts` | 28 | 888 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/warningHandler.ts` | 122 | 4486 | 重写：保持通用行为 | `src/generated_ports/p_6123ef3cb4c62.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/utils/which.ts` | 83 | 2392 | 重写：保持通用行为 | `src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs / src/diagnostics.rs` | 部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；Doctor/MCP 探测已排除不可执行的普通文件、Windows PATHEXT 扩展查找；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| `src/utils/windowsPaths.ts` | 174 | 6008 | 重写：保持通用行为 | `src/portable_platform.rs` | 部分实现：已编写 Rust 平台路径与 shell/版本/时间/定时配置逻辑，端到端待验 |
| `src/utils/withResolvers.ts` | 14 | 444 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/words.ts` | 801 | 10960 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/utils/workloadContext.ts` | 58 | 2337 | 重写：保持通用行为 | `src/portable_async_runtime.rs` | 部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| `src/utils/worktree.ts` | 1520 | 49995 | 重写：Git 工作树隔离 | `src/worktree.rs` | 部分实现：创建、变更保留与清理测试通过；本轮增加工作树创建时 HEAD 基线，退出前检测已提交但无未提交改动的分支，保留提交防止误删；命名工作树、交互选择和恢复仍缺 |
| `src/utils/worktreeModeEnabled.ts` | 12 | 415 | 重写：Git 工作树隔离 | `src/compat_helpers.rs` | 已验收：原始无条件 true 开关纯函数语义对照；证据 compat_helpers::tests::worktree_feature_is_enabled |
| `src/utils/xdg.ts` | 66 | 1876 | 重写：保持通用行为 | `src/portable_source_utils.rs` | 部分实现：已写 Rust 行为代码并加入基础用例，等待按原版完整验收与调用链补齐 |
| `src/utils/xml.ts` | 17 | 622 | 重写：XML/属性转义函数完整移植 | `src/xml_escape.rs` | 已验收：两种函数字符替换次序、双引号/单引号、Unicode、空输入与多次转义对照；证据 xml_escape::tests |
| `src/utils/yaml.ts` | 16 | 525 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/utils/zodToJsonSchema.ts` | 24 | 761 | 重写：保持通用行为 | `src/portable_settings_runtime.rs` | 部分实现：Rust 函数/类型/状态处理已写入，原版调用方与边界统一验收后置 |
| `src/vim/motions.ts` | 83 | 1902 | 重写：保持通用行为 | `src/generated_ports/p_a3cea9130934b.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/vim/operators.ts` | 557 | 15966 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/vim/textObjects.ts` | 187 | 5029 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/vim/transitions.ts` | 491 | 12381 | 重写：保持通用行为 | 待定 | 未迁移 |
| `src/vim/types.ts` | 200 | 6332 | 重写：保持通用行为 | `src/generated_ports/p_f4d7ac6b4c8c0.rs` | 部分实现：从原始 2.1.88 AST 转写 Rust 类型、字面量/纯函数或 JSX 视图结构；源码已落地，交互和完整语义待统一验收 |
| `src/voice/voiceModeEnabled.ts` | 55 | 2332 | 重写：保持通用行为 | 待定 | 未迁移 |
| `vendor/audio-capture-src/index.ts` | 152 | 4372 | 重写：保持通用行为 | 待定 | 未迁移 |
| `vendor/image-processor-src/index.ts` | 163 | 4724 | 重写：保持通用行为 | 待定 | 未迁移 |
| `vendor/modifiers-napi-src/index.ts` | 68 | 1605 | 重写：保持通用行为 | 待定 | 未迁移 |
| `vendor/url-handler-src/index.ts` | 59 | 1553 | 重写：保持通用行为 | 待定 | 未迁移 |
