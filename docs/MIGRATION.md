# Rust 重写功能对照

## 总体目标

将终端编程智能体拆分为可独立测试的 Rust 模块。模型协议是可替换层，不应存在全局硬编码模型厂商、账号验证或品牌绑定。工程工作均使用独立编写的实现。

## 当前代码映射

| 原有功能类别 | Rust 实现 | 当前状态 |
| --- | --- | --- |
| 模型 API 请求 | `provider.rs` | OpenAI-compatible 与 Gemini |
| OpenAI 兼容 SSE 流式文本/函数调用 | `sse.rs` / `agent.rs` | 事件分片重组、增量通知、Token 用量；Gemini 和 UI 实时绘制尚未完成 |
| 模型消息 / 工具调用 | `protocol.rs` | 统一文本、函数调用、工具结果 |
| Agent 查询循环 | `agent.rs` | 多轮模型决策和工具反馈 |
| 工具注册及参数 schema | `tools.rs::definitions` | 基础及扩展工具已注册；未达到原版工具全集 |
| 工具权限 | `tools.rs::Approval` | read-only / ask / full |
| 文件、搜索与命令 | `tools.rs` | 有限制的工作区文件工具、明确授权 Shell |
| 历史记录及恢复 | `session.rs` | UUID/JSON 文件 |
| 会话任务管理 | `task.rs` | 创建、状态更新、查询及持久化 |
| 项目提示词 | `agent.rs::system_prompt` | AGENTS.md / 本地 instructions |
| 模型选择与配置 | `config.rs` | 可自定义提供商配置 |
| Token 用量归一化及持久化 | `protocol.rs` / `provider.rs` / `session.rs` | OpenAI 兼容与 Gemini 原生用量字段 |
| CLI 和交互 REPL | `main.rs` | run/chat/models/init/sdk |
| JSON Lines SDK 标准输入输出 | `main.rs` / `agent.rs` | 多轮输入、工具结果、控制响应、用量；取消与远程 SDK 功能未齐 |
| Web 搜索与网页抓取 | `websearch.rs` / `webfetch.rs` | 可配置搜索与有权限网页读取，原版过滤和摘要仍待对齐 |
| 模型流式输出 | `sse.rs` / `agent.rs` | SSE 文本增量、函数调用分片、SDK 事件已实现 |
| 工具级流式并发编排 | — | 未实现；模型流式输出不等于流式工具执行 |
| 会话摘要压缩 | `agent.rs` / `session.rs` | 已实现手动及字符阈值触发 |
| 本地长期记忆读取与相关性检索 | `memory.rs` | 200 个文件扫描、200 行/25 KB 索引、当前模型选取最多 5 项；自动提取/梦境同步仍未实现 |
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
| 并发 Agent 团队执行 | `agent.rs` | 1—4 成员独立查询与结果汇总；任务消息与持久团队仍缺 |
| 后台 Shell 任务与输出恢复 | `background.rs` | 启动、输出落盘、等待、停止、列表、状态持久化；Agent 后台和远程任务未等价 |
| 完整团队持久通信、Agent 后台任务和跨进程恢复 | — | 未实现 |
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

## 独立兼容性契约

`runtime_contracts.rs`：零用量对象和上下文压缩提醒的订阅状态；MCP 名称保留通用 ASCII 归一化，不复刻原厂专属前缀处理。

`tool_names.rs`：已逐项核对 18 个只包含固定公开工具名的源码文件；只代表**常量文件自身**的迁移完成，不能把对应工具行为视作验收完成。

## 2026-10-29：多模型费用与 Token 统计主链

对照原始 src/cost-tracker.ts 及 src/utils/modelCost.ts，新增 src/cost.rs。单价由用户明确配置，不沿用特定厂商默认价格，未知费用不自动记为零。累计输入、输出、缓存读取、缓存创建与检索请求；从输入 Token 中扣除缓存读取 Token 后分别计价。不同提供商/模型独立建账，会话恢复后历史费用继续保留。普通响应、SSE、记忆和摘要统一接入计数；/cost 和 SDK 本地 control cost 可查询。

原始 API/墙钟/工具耗时、修改行数、退出时展示及其他跨模块调用尚未全部移植，因此相关源文件维持“部分实现”。新增费率配置示例见 README。

## 通用输入语义与文件编辑（2026-10-29）

`src/semantic_inputs.rs` 对应两条原始 Zod 预处理路径，严格转换 JSON 字面值：只接收确切 `"true"` 和 `"false"`，数字只接受 `^-?\\d+(\\.\\d+)?$` 且有限。内建工具数字字段还应满足各自整数类型要求，否则拒绝，避免老实现错误降级。`src/tools.rs` 在真实工具调用前应用转换，文件编辑支持原版 `replace_all` 选择并保留现有权限和修改历史。两条输入文件仍标注部分实现，Zod 组合 schema 与全调用方未完整映射。

`src/xml_escape.rs` 独立验收原始 `src/utils/xml.ts` 的两个转义函数；验收仅限该纯函数原路径，不延伸声称应用 UI/提示词处理完备。

### JSON 兼容与斜杠指令解析

对照原始 `src/utils/jsonRead.ts`、`src/utils/slashCommandParsing.ts`；`src/json_read.rs` 保留单次 BOM 剥离规则，并接入真实 Config 和 Session 读路径。`src/slash_command.rs` 保留按字面量空格分词、MCP 第二词标识和输入 trim 的语义，CLI 模型切换复用该解析；MCP 斜杠指令分发仍未等价。这两项仅宣称源函数自身等价。

### 模型成本与真实工具改动持续对照

`src/tool_metrics.rs` 按原始 `countLinesChanged` 的插入/删除行计数实现新建、覆盖和编辑文件统计，使用文本差异算法从实际修改前后内容计算，而不是按工具调用次数估算；`src/agent.rs` 在模型/工具/交互执行路径记录可恢复的耗时指标。`CostLedger` 现在支持模型价格字符串、直接 Token 计价以及附加持续统计。`src/utils/diff.ts` 只迁移统计子集，差异补丁的其余输出仍未迁移，保持原台账的严格标注。

### 子代理与并发团队会话费用合并

原始 `cost-tracker` 维护进程内会话累计用量；Rust 子会话之前单独记账、父会话漏算。新 `CostLedger::merge` 在真实委托和并行团队回收路径合并账本，保留同模型聚合、不同模型独立计数、未知费率显式状态；文件增删行指标同步合并而不覆盖。真实三轮模型委托测试验证根会话计入子会话 Token 与成本。异常退出和跨会话重试时长未声称等价。

### 模型缓存计费来源差异

原版计费将 input、cache_read、cache_creation 作为三类费用来源分别计算；部分通用 OpenAI 兼容服务却把缓存 Token 计入 prompt_tokens。费用配置按模型新增 `cache_read_included_in_input` 与 `cache_write_included_in_input` 标志：计入输入的缓存部分先减掉，再按缓存价格计费；独立上报时不减。普通 HTTP、SSE、Gemini 扩展用量解析统一记录 cache_creation 和 web_search_requests。既有费率 JSON 不提供这些字段时保持默认向后兼容。
