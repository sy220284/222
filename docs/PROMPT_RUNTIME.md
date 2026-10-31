# 提示词运行链及等价验收清单

目标：把用户提供的 Claude Code 2.1.88 可通用提示词行为接入 Atlas Rust 模型调用链，吸收 2.1.310 对执行证据、信息来源和恢复的强化约束。保持多模型、无需原厂账号、无监测与遥测回传。

## 真实执行路径

1. `src/prompt_runtime.rs` 集中定义主系统提示、协调者、Worker、规划、探索、独立验收、通用委派及会话压缩提示词。
2. `src/agent.rs::system_prompt` 构造主 system，添加工作区路径、`AGENTS.md`、`.atlas/instructions.md` 和可选的 `ATLAS_OUTPUT_STYLE=default|explanatory|learning` 或 `.atlas/output-styles/<名称>.md` 自定义风格。
3. `Agent::ask_with_blocks` 根据子 Agent 角色/团队身份选择专用 system；对于有效的 `.atlas/agents/<name>.md`，加载 frontmatter 后的用户自定义角色指令。角色指令保留在 system 中，独立于任务的 user 消息。
4. 随后按运行状态加入会话上下文、真实环境、会话摘要、Hook 上下文、本地相关记忆与授权目录；动态工具清单仅加入已发现的实际工具。
5. `src/provider.rs` 将 `ModelRequest.system` 传给 OpenAI-compatible 或 Gemini。工具专用提示词作为工具定义的 description 传递。
6. `Agent::compact_history` 发送不带工具的专用摘要请求，保留用户原话、已验证动作、失败原因及待办。
7. 团队 Worker 重启后仍能通过团队身份重新加载 Worker 规则；其他子 Agent 使用当前委派类型决定角色规则。
8. MCP stdio/HTTP 初始化的 `instructions` 通过 `McpManager::instruction_context` 带来源标记与 JSON 转义加入 system，不能授予权限。每一次模型请求都从当前连接状态重新读取（在动态工具重发现之后），断开、禁用或移除的服务器说明不会继续遗留在当前用户轮次的后续请求中。
9. SDK 后台任务通知经 `sdk_queue::notification_model_context` 标记为低信任状态数据，JSON 编码防止通知正文伪造新用户输入。

## 原始源码对照及已接线情况

| 原版提示词领域 | 222 实际实现 | 状态 |
|---|---|---|
| `src/constants/prompts.ts` 主身份/任务/行为边界 | `src/prompt_runtime.rs::BASE_SYSTEM` 和 `agent.rs` | 真实模型请求已接通；全部条件分支尚未逐项等价 |
| `src/utils/systemPrompt.ts` 优先级和角色选择 | `Agent::ask_with_blocks` | 内置和项目角色有真实接线；所有覆盖策略和功能门控尚未等价 |
| `src/tools/AgentTool/built-in/*` 角色指令 | `role_instructions` 与委派/团队调用 | 探索、规划、验收、Worker 已接入；可用工具的强制只读隔离仍按工具执行器权限单独验收 |
| `src/tools/*/prompt.ts` 工具描述 | `src/tools.rs::definitions`、其余各工具定义 | 文件/Git/Shell/Skills 12 个描述已实质补齐，其他工具需逐项核验 |
| `src/services/compact/prompt.ts` | `compact_prompt` 与 `Agent::compact_history` | 独立模型摘要请求已接线，原版各种压缩模式未完全移植 |
| `src/services/SessionMemory/prompts.ts` | `src/memory.rs` | 相关记忆筛选与 system 注入已接线；自动提取/整理仍缺 |
| `src/constants/outputStyles.ts` | `output_style_label`、`output_style_instructions`、`available_output_styles`、SDK initialize | 三种内置风格和可加载的工作区自定义风格会进入 SDK 实际列表，当前启用项反映真实设置；原版用户级目录、完整设置交互仍待对齐 |
| `src/utils/messages.ts` 与新版来源区分 | 主提示词、`sdk_queue::notification_model_context`、SDK 事件结构 | 后台任务通知作为低信任状态数据接入；其余来源标记/调度权限仍需补齐 |
| `src/constants/prompts.ts` MCP 指令 | `McpManager::instruction_context`、`Agent::ask_with_blocks` | 已修复同一用户轮次内动态重连/禁用后的旧说明滞留；新增重连/撤销回归测试。原版全部动态来源与策略仍待完整验收 |
| 原版自动模式权限分类器与外部规则模板 | `permission_rules.rs`、`permission_mode.rs` | 静态权限规则运行；模型分类器缺失，Auto 模式保持拒绝，防止误授权 |
| Dream、旁路 Observer、即时提示、Artifact 评论 | 对应功能尚未实现 | 不将未调用的提示词文本冒充运行功能 |

## 验收规则

- 对任何提示词，依次确认：源码对照 → 在 Rust 中存在真实文本/模板 → 对应角色或运行条件可达 → 构造进 `ModelRequest.system` 或实际工具定义 → 模拟提供商捕获断言 → 行为测试/失败边界 → 最新 CI 验证。
- `src/generated_ports` 中的静态 AST 字面量及目录扫描结果都不能直接算作生产接入。
- `cargo fmt --all -- --check`、`cargo check --all-targets`、`cargo test --all-targets` 均以最新 Head 对应 CI 为证据；不能复用被新提交取消的构建。

最近验证：模拟 MCP stdio 初始化及 Agent 模型请求测试均在 CI `40891272435` 中通过；该运行的完整测试失败原因是历史台账部分实现计数与矩阵不一致，已按当前矩阵修正，须以新 HEAD 的 CI 重新验收。此次接入没有启用自动权限模式、后台观察者或自动记忆整理。

本文件只记录已真正接线的内容与真实缺口，不将目前的增强提示词声明为两版所有原文的完全等价移植。

## 2026-10-31：MCP 提示词动态更新修复

- 原问题：`Agent::ask_with_blocks_for_message` 在进入多步工具循环前一次性拼接 MCP 初始化说明，MCP 在循环中重新发现或重连时，后续 `ModelRequest.system` 仍含旧说明。
- 修复：将追加调用下沉到每次真正构造 `ModelRequest` 之前，并在 `refresh_changed_tools()` 之后获取当前 MCP 说明；不额外改变原有工具权限。
- 回归：`src/mcp.rs::dead_stdio_process_reconnects_with_new_tool_schema_before_next_model_turn` 增加首次说明、重连后替换说明、禁用后清空说明断言。
- 验收边界：这一修改只修复动态说明的生命周期，不将 120 处提示词入口或其他未迁移分支标为原版完全等价；构建、全套测试和 CI 以本次提交后的最新 HEAD 为准。

## 2026-10-31：输出风格发现与 SDK 初始化一致性

- 修复前：SDK initialize 无条件返回 `output_style=default`、`available_output_styles=[default]`，即使实际启用其他内置/自定义风格。
- 修复后：`prompt_runtime::available_output_styles` 只枚举真正能够加载的本地 Markdown 风格，保留三种内置风格并对自定义风格排序，跳过空内容、无效 frontmatter、越界符号链接；SDK initialize 读取实际 `ATLAS_OUTPUT_STYLE` 并在启用项不可用时给出明确错误。
- 验证：已增加目录排序、无效文件、越界符号链接、每个返回风格都可成功加载的回归测试。尚未恢复原版所有用户级风格目录与交互配置入口。
