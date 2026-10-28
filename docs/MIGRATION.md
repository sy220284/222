# Rust 重写功能对照

## 总体目标

将终端编程智能体拆分为可独立测试的 Rust 模块。模型协议是可替换层，不应存在全局硬编码模型厂商、账号验证或品牌绑定。工程工作均使用独立编写的实现。

## 当前代码映射

| 原有功能类别 | Rust 实现 | 当前状态 |
| --- | --- | --- |
| 模型 API 请求 | `provider.rs` | OpenAI-compatible 与 Gemini |
| 模型消息 / 工具调用 | `protocol.rs` | 统一文本、函数调用、工具结果 |
| Agent 查询循环 | `agent.rs` | 多轮模型决策和工具反馈 |
| 工具注册及参数 schema | `tools.rs::definitions` | 六类内置工具 |
| 工具权限 | `tools.rs::Approval` | read-only / ask / full |
| 文件、搜索与命令 | `tools.rs` | 有限制的工作区文件工具、明确授权 Shell |
| 历史记录及恢复 | `session.rs` | UUID/JSON 文件 |
| 会话任务管理 | `task.rs` | 创建、状态更新、查询及持久化 |
| 项目提示词 | `agent.rs::system_prompt` | AGENTS.md / 本地 instructions |
| 模型选择与配置 | `config.rs` | 可自定义提供商配置 |
| Token 用量归一化及持久化 | `protocol.rs` / `provider.rs` / `session.rs` | OpenAI 兼容与 Gemini 原生用量字段 |
| CLI 和交互 REPL | `main.rs` | run/chat/models/init |
| Web 搜索、Web 抓取 | — | 未实现 |
| 流式执行和并发编排 | — | 未实现 |
| 会话摘要压缩 | `agent.rs` / `session.rs` | 已实现手动及字符阈值触发 |
| 长期记忆检索 | — | 未实现 |
| MCP stdio 工具连接 | `mcp.rs` | 已实现工具发现、调用、权限接入 |
| 项目 Skills | `tools.rs` | 已实现 SKILL.md 发现和读取 |
| 本地同步 Hooks | `hooks.rs` / `agent.rs` | Pre/PostToolUse、Stop、用户输入、会话与压缩事件 |
| 插件管理、异步 HTTP/Prompt/Agent Hooks | — | 未实现 |
| Git 只读操作 | `tools.rs` | status / diff / log 已实现 |
| LSP、Git 修改与工作树管理 | — | 未实现 |
| 一级子 Agent 委派与工作树隔离 | `agent.rs` / `worktree.rs` | 已实现独立会话、隔离、清理 |
| 完整多 Agent 团队、任务消息与并行管理 | — | 未实现 |
| 背景任务、任务恢复、远程会话 | — | 未实现 |
| 终端渲染/主题/快捷键/SDK/IDE | — | 未实现 |

## 验收原则

- 编译和测试必须基于真实源文件，通过 `cargo check --all-targets` 与 `cargo test --all-targets`。
- 对任一模型的真实端到端调用需要对应服务或测试端点，不能以协议适配器存在代替运行验收。
- 不把单 provider 兼容协议误认为所有模型自动支持工具调用；具体取决于模型本身。
- 仅在功能完整并通过测试之后更改“未实现”状态。
