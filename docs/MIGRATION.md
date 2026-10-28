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
| JSON Lines SDK 标准输入输出 | `main.rs` / `agent.rs` | 多轮输入、工具结果、控制响应、用量；取消与远程 SDK 功能未齐 |
| Web 搜索、Web 抓取 | — | 未实现 |
| 流式执行和并发编排 | — | 未实现 |
| 会话摘要压缩 | `agent.rs` / `session.rs` | 已实现手动及字符阈值触发 |
| 长期记忆检索 | — | 未实现 |
| MCP stdio 工具连接 | `mcp.rs` | 已实现工具发现、调用、权限接入 |
| 项目 Skills | `tools.rs` | 已实现 SKILL.md 发现和读取 |
| 本地同步 Hooks | `hooks.rs` / `agent.rs` | Pre/PostToolUse、Stop、用户输入、会话与压缩事件 |
| 本地插件装载与技能组合 | `plugins.rs` | 启用式插件装载、合并 MCP/Hooks、技能发现与读取 |
| 插件安装更新管理、异步 HTTP/Prompt/Agent Hooks | — | 未实现 |
| Git 只读操作 | `tools.rs` | status / diff / log 已实现 |
| Jupyter Notebook 编辑 | `notebook.rs` / `tools.rs` | 单元格 ID / 索引、插入/替换/删除、读后修改检测（输出与编码仍待对照） |
| Glob / Grep 高级检索 | `search.rs` | 文件模式、分页、内容/计数/路径输出；ripgrep 各项语义仍需逐项核实 |
| LSP 标准查询与文档同步 | `lsp.rs` | 9 类请求和消息帧已实现，服务器兼容未完全验证 |
| Git 修改与工作树管理 | `worktree.rs` | 工作树生命周期部分实现 |
| WebFetch 直接公网访问 | `webfetch.rs` | 公网文本/HTML 获取、用户权限和防内网重定向；原版网页处理/提示词应用待进一步对齐 |
| WebSearch 多后端搜索 | `websearch.rs` | SearXNG 与 Brave 可配置搜索、结果标准化、审批；官方搜索排序和过滤协议未全等 |
| 文件修改历史和回退 | `history.rs` | 文字文件的检查点、修改后哈希、拒绝覆盖后来修改；多消息粒度和 Bash 自动跟踪未对齐 |
| 一级子 Agent 委派与工作树隔离 | `agent.rs` / `worktree.rs` | 已实现独立会话、隔离、清理 |
| 完整多 Agent 团队、任务消息与并行管理 | — | 未实现 |
| 背景任务、任务恢复、远程会话 | — | 未实现 |
| 终端渲染/主题/快捷键/SDK/IDE | — | 未实现 |

## 验收原则

- 编译和测试必须基于真实源文件，通过 `cargo check --all-targets` 与 `cargo test --all-targets`。
- 对任一模型的真实端到端调用需要对应服务或测试端点，不能以协议适配器存在代替运行验收。
- 不把单 provider 兼容协议误认为所有模型自动支持工具调用；具体取决于模型本身。
- 仅在功能完整并通过测试之后更改“未实现”状态。

## 全量文件级验收门槛

**必须按用户原始要求核算到每个文件的语义/行为，而非仅看核心模块能运行。**

- 原始解析源码：1,906 个文件（1,902 个应用层 + 4 个 vendor 包装层）。
- 明确排除：22 个遥测/诊断回传相关文件路径（按照上传包路径的首轮静态分类；对于同时承担本地功能的混合代码，仍需保留通用功能部分）。
- 其余 1,884 个路径：必须逐文件核对函数入口、调用链、副作用、边界、错误处理、取消行为、权限和测试；可由多个 Rust 文件共同承载，但每个原始路径都须有完整追溯。
- `docs/FILE_PORT_MATRIX.md` 的“部分实现”只表示已存在相似功能的 Rust 模块，并**不表示该源文件已达到语义等价**。
- 目前缺失完整原生组件的源文件以及原产品远程服务协议源码，不能声称这些行为在未获得足够依据时已经全等。
- 登录/专属模型端点改为可配置多模型；原厂远程桥接须改造为自建协议；自动遥测删除，不引入隐式联网补偿。
- 合格需要源码核对、编译、单元、集成和实际工作流验收全部通过。当前仅完成新增 Rust 行为的编译与测试，未完成 1,884 项文件级等价验收。
