# PR #6：100 条独立源路径的补齐与验收进度

**核对基准**：GitHub PR #6 提取时 HEAD `bd9844bc9f23617bef6574466097070228b62a46`，原始 Claude Code 2.1.88 源路径与 `docs/PORT_BASELINE.json` 冻结 SHA-256。原始 1,906 个源码文件已经在本地逐一独立 SHA-256 验证，**1,906/1,906 一致**。

本清单跟踪 **100 条不同原路径（已验收 4 条，仍待完整等价验收 96 条）**，优先选取本 PR 实际修改所关联的 Rust 生产模块，并覆盖 Bridge、SDK/CLI、TUI、工具/MCP、命令与 Agent。用于后续修复、逐项验收与反复核查。表中“部分实现/未迁移”准确表示当前尚无完整等价证据，不能作为 100 条通过的证明。

每条源路径只有同时达到以下标准才可转为 `已验收`：原始 SHA-256 一致；全部必要函数/类型和行为语义覆盖；生产入口真实调用；正常、错误、权限、取消、并发、恢复等适用测试实际执行；`docs/PORT_EVIDENCE.jsonl` 绑定通过的 tested_commit / workflow_run_id；原路径剩余差异为空。已跳过作业或静态源码匹配不得用于升格。

- 本次独立追踪：**100 条（已验收 4、仍未验收 96）**；源文件哈希均有冻结基线（并已对完整原始包复核）。
- 本轮新增完整等价验收：**4 条**，已同时更新矩阵及逐路径证据；其余 96 条不凭单测/映射/CI 绿灯冒充独立原路径闭环。
- 历史基线已验收：**29 条**；本轮新增 4 条，动态台账当前总计 **33 条**。
- 本轮已落地修正：`src/portable_async_runtime.rs` 修复 JS `parseInt` 请求 ID 解析差异；`src/portable_ink_core.rs` 增加完整常量断言；`src/tool_names.rs` 补充 `TodoWrite` 原版工具名与测试。对应最新提交/CI 仍须按最新 PR HEAD 动态复核。

| # | 类别 | 原始路径 | Rust 映射 | 现有状态 | 验收缺口（须继续闭环） |
|---:|---|---|---|---|---|
| 1 | 工具/MCP | `src/tools/TodoWriteTool/constants.ts` | `src/tool_names.rs` / `src/task.rs` | **已验收** | 原版唯一导出与生产工具目录/调用分发逐项一致；`tool_names::tests::names_and_aliases`；测试提交 `061582f7`、成功 CI [41026307684](https://github.com/sy220284/222/actions/runs/41026307684)；证据见 `docs/PORT_EVIDENCE.jsonl` |
| 2 | Bridge | `src/bridge/flushGate.ts` | src/bridge_core.rs / src/main.rs | 部分实现 | 远程连接、错误与恢复链存在明确差异；原台账：部分实现：本地 SDK 控制入口可达；初始历史回放中的出站消息有序排队、传输替换保留、永久关闭清除与隔离测试已通过；远程 HTTP/WebSocket 实际发送仍未贯通 |
| 3 | Bridge | `src/bridge/sessionIdCompat.ts` | src/bridge_core.rs / src/main.rs | **已验收** | 动态门禁、双向转换与真实 Bridge SDK 入口已闭环；[CI #41032938553](https://github.com/sy220284/222/actions/runs/41032938553)、`docs/PORT_EVIDENCE.jsonl` |
| 4 | Bridge | `src/bridge/inboundMessages.ts` | src/multimodal.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：用户消息/UUID 提取、camelCase mediaType 兼容、缺失媒体类型 PNG/JPEG/GIF/WebP 魔数识别已通过 SDK 直接及排队模型 HTTP 测试；远端传输、附 |
| 5 | Bridge | `src/bridge/pollConfigDefaults.ts` | src/bridge_core.rs / src/main.rs | **已验收** | 原版八个常量值、真实 SDK Bridge 控制入口、异会话错误和不可变状态行为逐项对照；证据 `bridge_core::tests::source_poll_defaults_are_consistent_for_single_and_multiple_sessions`、[CI #41032013622](https://github.com/sy220284/222/actions/runs/41032013622)、`docs/PORT_EVIDENCE.jsonl` |
| 6 | Bridge | `src/bridge/inboundAttachments.ts` | src/bridge_inbound_attachments.rs / src/main.rs | 部分实现 | 远程连接、错误与恢复链存在明确差异；原台账：部分实现：可信可配置附件服务的 OAuth 风格令牌请求、30秒超时、32MiB分块上限、最多四个并发下载、禁止远程明文凭证及 HTTP 重定向、路径清洗及会话隔离、最后文本块 @引用插入与无附件容错 |
| 7 | SDK/CLI | `src/cli/ndjsonSafeStringify.ts` | src/ndjson.rs / src/main.rs / src/agent.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：JS 特殊行分隔符转义、SDK 输出回环与 JSON 值保持；jsonStringify 的非标准对象语义待核对 |
| 8 | SDK/CLI | `src/entrypoints/sdk/coreTypes.ts` | src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现，多模式与 SDK 未等价 |
| 9 | SDK/CLI | `src/entrypoints/sdk/controlSchemas.ts` | src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现，多模式与 SDK 未等价；已接通 initialize（仅现有本地会话元数据）、mcp_status、cancel_async_message，原版其他控制类型仍未迁移；seed_read_ |
| 10 | SDK/CLI | `src/cli/structuredIO.ts` | src/main.rs / src/agent.rs / src/sse.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：NDJSON SDK 多轮、事件和实时文本增量；控制请求体系未全等；SDK 本地控制包络初始化/状态/取消回环，其他协议仍未全等 ；SDK 文本+base64 图片混合块经 Agent 传递 |
| 11 | SDK/CLI | `src/entrypoints/sdk/coreSchemas.ts` | src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现，多模式与 SDK 未等价 |
| 12 | SDK/CLI | `src/cli/print.ts` | src/queued_prompt_batch.rs / src/sdk_queue.rs / src/main.rs；SDK seed_read_state 接入共享会话已读缓存（9743d9bc），源码已提交待验收； | 部分实现 | 远程连接、错误与恢复链存在明确差异；原台账：部分实现：同 workload/meta 提示合并调用和去重计费、结构化 task_notification 系统事件、状态/用量解析；新增按 UUID 原始内容逐条回放（含批次合并与直接 SDK 消 |
| 13 | TUI | `src/components/CustomSelect/index.ts` | src/custom_select_exports.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 统一导出单选、多选、选项映射与输入事件控制器；select.tsx 完整视图待迁移 |
| 14 | TUI | `src/ink/events/event.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 15 | TUI | `src/ink/warn.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 16 | TUI | `src/hooks/useTerminalSize.ts` | src/generated_ports/terminal_context.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：上下文读取与缺失错误已手写；React Ink Provider 与终端尺寸事件仍需完整接线验收 |
| 17 | TUI | `src/ink/wrapAnsi.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 18 | TUI | `src/ink/measure-element.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 19 | TUI | `src/ink/widest-line.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 20 | TUI | `src/ink/events/terminal-focus-event.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 21 | TUI | `src/components/agents/utils.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 22 | TUI | `src/ink/hooks/use-terminal-focus.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 23 | TUI | `src/ink/events/focus-event.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 24 | TUI | `src/hooks/useMailboxBridge.ts` | src/in_memory_mailbox.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：空闲仅消费一条并提交、加载时不消费；React 外部 Store 与 effect 调度未等价 |
| 25 | TUI | `src/components/PromptInput/inputModes.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 26 | TUI | `src/ink/line-width-cache.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 27 | TUI | `src/components/design-system/color.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 28 | TUI | `src/ink/hooks/use-terminal-title.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 29 | TUI | `src/ink/tabstops.ts` | src/portable_render_semantics.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：ANSI CSI/OSC/DCS 等控制序列不计宽度、八列制表符与换行重置已写入 Rust 并接入 ANSI SVG 渲染；上游所有渲染调用链仍需继续接通 |
| 30 | TUI | `src/ink/events/emitter.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 31 | TUI | `src/components/StructuredDiff/colorDiff.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 32 | TUI | `src/ink/measure-text.ts` | src/portable_render_semantics.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：原始单遍文本宽高测算、尾部空行、非正数/无穷宽度、逐行自动折行已实现；上游所有渲染调用链仍需继续接通 |
| 33 | TUI | `src/ink/get-max-width.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 34 | TUI | `src/components/CustomSelect/option-map.ts` | src/custom_select_source.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 选项 Map 保留顺序、前后链接与同值键覆盖，UI 生命周期尚未接通 |
| 35 | TUI | `src/hooks/renderPlaceholder.ts` | src/generated_ports/text_input_semantics.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：原版字符范围与渲染主要分支已写 Rust；占位显示已被 src/terminal_input.rs 生产入口调用；ANSI 全协议、完整输入 Hook 与渲染细节未验收 |
| 36 | TUI | `src/ink/events/click-event.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 37 | TUI | `src/ink/termio/esc.ts` | src/portable_render_semantics.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：ESC 重置、保存/恢复光标、索引移动、换行、字符集选择及未知序列返回已完整实现；上游所有渲染调用链仍需继续接通 |
| 38 | TUI | `src/keybindings/template.ts` | src/keybindings.rs / src/main.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：用户配置模板及保留快捷键过滤、原子排他创建已迁入 Rust；文本编辑器差异待覆盖 |
| 39 | 命令 | `src/commands/clear/clear.ts` | src/agent.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：会话结束/开始 Hook、旧会话保留与新会话 ID、权限模式保留接入实际聊天和 SDK；完整缓存、MCP 热重置与全部后台任务生命周期待验收 |
| 40 | 命令 | `src/commands/add-dir/index.ts` | src/workspace_dirs.rs / src/main.rs / src/agent.rs / src/tools.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：CLI 确认、目录校验、授权文件工具与设置保存；原版表单/沙箱/SDK 链仍缺 |
| 41 | 命令 | `src/commands/rename/index.ts` | src/main.rs / src/session_control.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| 42 | 命令 | `src/commands/export/index.ts` | src/main.rs / src/session_control.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| 43 | 命令 | `src/commands/resume/index.ts` | src/main.rs / src/session.rs / src/session_control.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：真实会话发现、标题持久化、完整消息及工具调用文本导出、恢复入口和跨进程 CLI；原版 UI/剪贴板/模型命名/跨项目模糊查询尚缺 |
| 44 | 命令 | `src/commands/logout/index.ts` | src/provider.rs / src/config.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分覆盖，原路径仍需逐条核对 |
| 45 | 命令 | `src/commands/login/index.ts` | src/provider.rs / src/config.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分覆盖，原路径仍需逐条核对 |
| 46 | 命令 | `src/commands/version.ts` | src/version.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：当前运行二进制版本、可选编译时间、交互 /version 和版本 CLI 已落地；专属用户开关按多模型要求移除，原调用链完整验收待做 |
| 47 | 命令 | `src/commands/files/files.ts` | src/files_context.rs / src/agent.rs / src/main.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：/files 从真实工具结果重建读、写、编辑、NotebookEdit 的成功文件缓存，按最近使用排序、去重、保留最多 100 项，失败不入缓存，删除后的历史文件仍可跨会话恢复显示；原版独立 |
| 48 | 命令 | `src/commands/vim/vim.ts` | src/vim_mode_source.rs / src/main.rs / src/config.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：/vim 命令切换并持久化 Vim/标准编辑模式、旧 emacs 模式兼容及原版反馈文案；输入处理层须读取偏好才能完成终端等价 |
| 49 | 命令 | `src/commands/stats/stats.tsx` | src/stats.rs / src/main.rs / tests/integration_cli.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：/stats 和独立 stats 命令已经真实读取本地持久会话，按供应商/模型汇总使用量和已知费用、缓存/搜索/工具调用及代码行统计；分支按保存的基线扣除继承历史，未知费率不猜。原版多标签页 |
| 50 | 命令 | `src/commands/doctor/doctor.tsx` | src/diagnostics.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust Doctor 主入口已连接 CLI doctor 和交互 /doctor，原生离线检查版本、可执行路径、工作区、Git、配置、模型供应商、MCP可执行程序与上下文文件大小；原版 D |
| 51 | 命令 | `src/commands/output-style/output-style.tsx` | src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：交互式 /output-style 输出原版完整停用提示并保持会话不追加消息；仍待终端输出回归验证 |
| 52 | 命令 | `src/commands/release-notes/release-notes.ts` | src/release_notes.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：版本更新日志解析、500ms 下载、缓存及 CLI/交互调用已实现；自动通知仍缺 |
| 53 | 命令 | `src/commands/plugin/plugin.tsx` | src/main.rs / src/plugin_commands.rs / src/plugin_manager.rs | 部分实现 | 远程连接、错误与恢复链存在明确差异；原台账：部分实现：交互式 /plugin 命令已接通原版子命令解析及真实本地插件管理，修改后刷新当前 Agent Hook/MCP/技能命令目录并显示失败（580c0586，源码已提交待验收）；原版 Plug |
| 54 | 命令 | `src/commands/keybindings/keybindings.ts` | src/keybindings.rs / src/main.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：CLI keybindings 子命令和聊天 /keybindings 实际创建、校验、查询与编辑器入口已落地 Rust；终端配置编辑体验待验收 |
| 55 | 命令 | `src/commands/status/status.tsx` | src/agent.rs / src/main.rs / tests/integration_cli.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：/status 已连入交互终端，展示真实会话标题/ID、工作区、模型/供应商、权限模式、MCP 服务及工具数，测试覆盖动态改名与不泄露凭据；原版 Settings 各页、IDE、完整安装/沙 |
| 56 | 命令 | `src/commands/review.ts` | src/git_workflows.rs / src/main.rs | 部分实现 | 远程连接、错误与恢复链存在明确差异；原台账：部分实现：通过真实 Agent 执行 PR 审查任务；原版专属 Ultrareview 远程链仍待改造 |
| 57 | 工具/MCP | `src/services/mcp/normalization.ts` | src/runtime_contracts.rs / src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：已迁移通用 ASCII 名称归一化；原厂服务名前缀专有规则按要求移除 |
| 58 | 工具/MCP | `src/services/mcp/InProcessTransport.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 59 | 工具/MCP | `src/services/mcp/officialRegistry.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 60 | 工具/MCP | `src/services/mcp/oauthPort.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 61 | 工具/MCP | `src/tools/TaskListTool/TaskListTool.ts` | src/task_graph.rs / src/agent.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：任务过滤 _internal、已完成依赖解除、负责人/阻塞展示；完整 UI/排序与团队可见性待核对；TaskList 拒绝未知参数的校验已接入实际 Agent Hook 前置门禁（06c65 |
| 62 | 工具/MCP | `src/services/mcp/channelAllowlist.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 63 | 工具/MCP | `src/tools/TaskGetTool/TaskGetTool.ts` | src/task_graph.rs / src/agent.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：任务不存在、状态、描述及双向依赖读取与原版文本输出；所有错误处理和 SDK 数据结构待对照；TaskGet 输入严格校验已接入实际 Agent Hook 前置门禁（06c65cc） |
| 64 | 工具/MCP | `src/tools/TaskCreateTool/TaskCreateTool.ts` | src/task_graph.rs / src/agent.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：磁盘持久化、任务编号、元数据、TaskCreated hook 阻断回滚及模型调用；UI 自动展开与上下文差异待对照；严格参数校验已提前到 PreToolUse 前，非法创建不触发 Hook |
| 65 | 工具/MCP | `src/services/mcp/vscodeSdkMcp.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 66 | 工具/MCP | `src/tools/TodoWriteTool/TodoWriteTool.ts` | src/task.rs / src/session.rs / src/agent.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：无交互 SDK/单次运行暴露 TodoWrite；严格清单字段与状态校验、整表替换、全部完成自动清空、会话保存恢复；原版 UI 状态与跨会话边界待验收 |
| 67 | 工具/MCP | `src/tools/TaskStopTool/TaskStopTool.ts` | src/background.rs / src/tools.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：task_id、shell_id、KillShell、成功返回和异常状态对齐；停止与自然完成竞态及停止前实时状态核对已补；新增输出排空期取消和 SDK stop_task 的 task_no |
| 68 | 工具/MCP | `src/services/mcp/mcpStringUtils.ts` | src/mcp.rs | **已验收** | 原版六函数、真实 MCP 注册路由及权限名隔离测试通过；[CI #41032938553](https://github.com/sy220284/222/actions/runs/41032938553)、`docs/PORT_EVIDENCE.jsonl` |
| 69 | 工具/MCP | `src/tools/TeamDeleteTool/TeamDeleteTool.ts` | src/team_registry.rs / src/agent.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：活跃成员阻止清理、目录删除、任务图恢复；原版工作树/窗格清理未等价 |
| 70 | 工具/MCP | `src/tools/AgentTool/built-in/planAgent.ts` | src/prompt_runtime.rs / src/agent.rs / src/tools.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：规划角色 system 和只读工具约束已进入真实执行链；原版完整计划流程仍待逐项验收 |
| 71 | 工具/MCP | `src/services/mcp/SdkControlTransport.ts` | src/mcp.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，仅基础 stdio |
| 72 | 工具/MCP | `src/tools/AgentTool/built-in/exploreAgent.ts` | src/prompt_runtime.rs / src/agent.rs / src/tools.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：探索角色 system 和只读工具约束已进入真实执行链；原版最小搜索策略与边界待完整验收 |
| 73 | Agent/其他 | `src/constants/messages.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 74 | Agent/其他 | `src/constants/turnCompletionVerbs.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 75 | Agent/其他 | `src/constants/errorIds.ts` | src/portable_ink_core.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：Rust 终端事件/宽度布局/模式/展示基础代码已落地，React/Ink 整体联动待验 |
| 76 | Agent/其他 | `src/services/claudeAiLimitsHook.ts` | src/provider.rs / src/config.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分覆盖，原路径仍需逐条核对 |
| 77 | Agent/其他 | `src/services/oauth/crypto.ts` | src/oauth_crypto.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：32 字节系统级安全随机数、无填充 Base64URL、SHA-256 PKCE challenge 和独立 state 已有原生 Rust、RFC 7636 向量及真实 CLI 测试；OA |
| 78 | Agent/其他 | `src/services/compact/compactWarningHook.ts` | src/agent.rs / src/session.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，长期记忆未等价 |
| 79 | Agent/其他 | `src/services/compact/compactWarningState.ts` | src/runtime_contracts.rs / src/agent.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：默认 false、压缩抑制、清除与重复赋值不通知已验证；证据 runtime_contracts::tests::compact_warning_state_and_subscription |
| 80 | Agent/其他 | `src/services/autoDream/config.ts` | src/generated_ports/p_3f28a74b2c6e6.rs | 部分实现 | 生成模块代码存在，完整事件回调/真实入口/组件行为未获证明；原台账：部分实现：autoDream 决策纯函数（用户设置显式值覆盖实验默认，未设时实验值决定），settings/growthbook 读取由调用方传入；实验 flag 名按隐私门禁省略 |
| 81 | Agent/其他 | `src/services/api/ultrareviewQuota.ts` | src/provider.rs / src/agent.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，回退与流式行为未等价 |
| 82 | 底层工具 | `src/utils/shell/outputLimits.ts` | src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| 83 | 底层工具 | `src/utils/withResolvers.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 84 | 底层工具 | `src/utils/shell/resolveDefaultShell.ts` | src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| 85 | 底层工具 | `src/utils/secureStorage/index.ts` | src/generated_ports/p_92f3dc63fc226.rs | 部分实现 | 生成模块代码存在，完整事件回调/真实入口/组件行为未获证明；原台账：部分实现：平台安全存储选择器返回 fallback 链（vendor Keychain 按仓库等价范围排除，macOS 分支留安全框架未来移植钩子），读写删回路测试覆盖（delete 后兜底空对象语义 |
| 86 | 底层工具 | `src/utils/sessionEnvVars.ts` | src/agent.rs / src/tools.rs / src/background.rs / src/main.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：会话变量增删查、前台/后台 Shell 和子代理继承、真实 CLI 回归；Hooks/MCP/LSP 完整透传未对照 |
| 87 | 底层工具 | `src/utils/sinks.ts` | src/generated_ports/p_3257507549930.rs | 部分实现 | 生成模块代码存在，完整事件回调/真实入口/组件行为未获证明；原台账：部分实现：错误日志与分析 sink 挂接幂等标志（原子状态），叶子模块保原版防循环结构；sink 服务本体（errorLogSink/analytics sink）待各自端口迁移，注入钩子接口已留 |
| 88 | 底层工具 | `src/utils/extraUsage.ts` | src/provider.rs / src/config.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分覆盖，原路径仍需逐条核对 |
| 89 | 底层工具 | `src/utils/bash/specs/srun.ts` | src/generated_ports/p_4007591beaebd.rs | 部分实现 | 生成模块代码存在，完整事件回调/真实入口/组件行为未获证明；原台账：部分实现：srun 规格登记至 builtin_command_specs 体系（-n/--ntasks、-N/--nodes 双旗标+command 参数），withfig options 结构向本 |
| 90 | 底层工具 | `src/utils/userPromptKeywords.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 91 | 底层工具 | `src/utils/shell/shellProvider.ts` | src/shell_runtime.rs / src/config.rs / src/agent.rs / src/tools.rs / src/background.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：运行时类型/选择、真实前后台子进程、PATH 查找与 PowerShell 参数，已加真实进程回归；原版沙箱、Windows Git Bash/完整环境和高级取消待核 |
| 92 | 底层工具 | `src/utils/controlMessageCompat.ts` | src/control_message_compat.rs / src/main.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：请求 ID 两种命名兼容、嵌套 response、优先级与 SDK 回复 ID；其余控制协议未全量验证 |
| 93 | 底层工具 | `src/utils/collapseTeammateShutdowns.ts` | src/portable_render_semantics.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：连续完成的进程内队员关停消息折叠、首消息元信息、边界与非关停保留已实现；上游所有渲染调用链仍需继续接通 |
| 94 | 底层工具 | `src/utils/lockfile.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 95 | 底层工具 | `src/utils/idleTimeout.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 96 | Agent/其他 | `src/services/oauth/getOauthProfile.ts` | src/oauth_profile.rs / src/main.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现：API key/account_uuid 请求及 OAuth Bearer 资料查询真实 HTTP、10 秒超时、非 2xx/无效响应处理与可配置多提供商端点已在 Rust 落地；真实本地  |
| 97 | 底层工具 | `src/utils/sequential.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 98 | 底层工具 | `src/utils/sessionUrl.ts` | src/portable_async_runtime.rs | 部分实现 | SDK 回调、状态、取消与流事件时序需源码对照；原台账：部分实现：Rust 异步/队列状态机已编写，生产上下游与完整生命周期待最终核对 |
| 99 | Agent/其他 | `src/services/api/usage.ts` | src/provider.rs / src/agent.rs | 部分实现 | 台账仍标部分实现；需逐导出行为、真实调用链和成功测试证据；原台账：部分实现，回退与流式行为未等价 |
| 100 | 底层工具 | `src/utils/collapseHookSummaries.ts` | src/portable_render_semantics.rs / src/main.rs | 部分实现 | TUI 实际交互/渲染与状态回调待完整验收；原台账：部分实现：连续同标签 Hook 的计数、内容/错误并集、阻断、输出状态和最大并发耗时已实现；上游所有渲染调用链仍需继续接通 |

## 验收纪律

1. 此清单每项的原版 SHA-256 使用冻结基线 `docs/PORT_BASELINE.json`，不得用文件名相似或实现数量替代导出与语义对照。
2. 每完成一个候选，补完整的真实 Rust 生产入口、原版源码对照结果、实际测试 Job 与可复现的失败/恢复验证；`docs/FILE_PORT_MATRIX.md` 的状态需要与 `docs/PORT_EVIDENCE.jsonl` 一致。
3. 若检查出现缺口，应在原 Rust 业务链补齐并测试，不得只修改本文档或虚增完成数。PR 未全量完成前保持 Draft。
