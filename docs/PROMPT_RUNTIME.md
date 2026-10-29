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
8. MCP stdio/HTTP 初始化的 `instructions` 通过 `McpManager::instruction_context` 带来源标记与 JSON 转义加入 system，不能授予权限；未连接或失效的服务器不注入。
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
| `src/constants/outputStyles.ts` | `output_style_label`、`output_style_instructions` | 3 种内置风格及工作区自定义 Markdown 指令接入真实 system；全量原版目录发现及设置交互仍缺 |
| `src/utils/messages.ts` 与新版来源区分 | 主提示词、`sdk_queue::notification_model_context`、SDK 事件结构 | 后台任务通知作为低信任状态数据接入；其余来源标记/调度权限仍需补齐 |
| `src/constants/prompts.ts` MCP 指令 | `McpManager::instruction_context`、`Agent::ask_with_blocks` | stdio/HTTP 原始初始化说明进入真实 system；MCP 动态变更和所有安全来源验收待核实 |
| 原版自动模式权限分类器与外部规则模板 | `permission_rules.rs`、`permission_mode.rs` | 静态权限规则运行；模型分类器缺失，Auto 模式保持拒绝，防止误授权 |
| Dream、旁路 Observer、即时提示、Artifact 评论 | 对应功能尚未实现 | 不将未调用的提示词文本冒充运行功能 |

## 验收规则

- 对任何提示词，依次确认：源码对照 → 在 Rust 中存在真实文本/模板 → 对应角色或运行条件可达 → 构造进 `ModelRequest.system` 或实际工具定义 → 模拟提供商捕获断言 → 行为测试/失败边界 → 最新 CI 验证。
- `src/generated_ports` 中的静态 AST 字面量及目录扫描结果都不能直接算作生产接入。
- `cargo fmt --all -- --check`、`cargo check --all-targets`、`cargo test --all-targets` 均以最新 Head 对应 CI 为证据；不能复用被新提交取消的构建。

最近验证：模拟 MCP stdio 初始化及 Agent 模型请求测试均在 CI `40891272435` 中通过；该运行的完整测试失败原因是历史台账部分实现计数与矩阵不一致，已按当前矩阵修正，须以新 HEAD 的 CI 重新验收。此次接入没有启用自动权限模式、后台观察者或自动记忆整理。

本文件只记录已真正接线的内容与真实缺口，不将目前的增强提示词声明为两版所有原文的完全等价移植。
