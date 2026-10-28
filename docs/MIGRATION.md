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
| 项目提示词 | `agent.rs::system_prompt` | AGENTS.md / 本地 instructions |
| 模型选择与配置 | `config.rs` | 可自定义提供商配置 |
| CLI 和交互 REPL | `main.rs` | run/chat/models/init |
| Web 搜索、Web 抓取 | — | 未实现 |
| 流式执行和并发编排 | — | 未实现 |
| 自动压缩与长期记忆检索 | — | 未实现 |
| MCP / Plugin / Skills / Hooks | — | 未实现 |
| 内建 LSP / Git 交互 | — | 未实现 |
| 工作树与多 Agent 团队 | — | 未实现 |
| 背景任务、任务恢复、远程会话 | — | 未实现 |
| 终端渲染/主题/快捷键/SDK/IDE | — | 未实现 |

## 验收原则

- 编译和测试必须基于真实源文件，通过 `cargo check --all-targets` 与 `cargo test --all-targets`。
- 对任一模型的真实端到端调用需要对应服务或测试端点，不能以协议适配器存在代替运行验收。
- 不把单 provider 兼容协议误认为所有模型自动支持工具调用；具体取决于模型本身。
- 仅在功能完整并通过测试之后更改“未实现”状态。
