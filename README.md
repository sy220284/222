# Atlas Agent

**独立 Rust 多模型编程智能体**。不需要特定厂商登录，不捆绑官方 SDK、专有 API、品牌资源或账号体系。使用自己的模型接口或本地模型即可。

本项目从零编写 Rust 代码，用通用 Agent 概念实现对话、工具调用、工作区操作、审批与会话恢复。**当前为可运行核心版本，不是任何第三方产品的全功能等价移植。**

## 已实现

- 多模型提供商：OpenAI 兼容 Chat Completions（OpenAI、OpenRouter、DeepSeek、Qwen、vLLM、LM Studio 等需各自兼容函数调用）；Ollama 本地兼容接口；Gemini 原生 generateContent。
- 通用模型消息格式，支持模型请求多次工具调用、工具结果回填与继续决策。
- 文件读取、递归文件列表、正则全文检索、文件写入、唯一文本替换、带超时的 Shell 命令；以及 Git 只读工具。
- 项目技能（`.atlas/skills/<名称>/SKILL.md`）列表与加载，模型可选择使用相关指导。
- 手动 `/compact` 及历史记录超过字符阈值时的自动摘要压缩；摘要随会话保存。
- 多模型 Token 使用量归一化（以提供商返回的字段为准），会话累计保存，交互模式通过 `/usage` 查询。
- 默认审批写操作与 Shell；非交互模式默认拒绝，需明确设置 `--approval full`。
- 文件工具工作区限制与符号链接越界检查；命令执行按明确授权处理。
- `AGENTS.md` / `.atlas/instructions.md` 项目指导文件、UUID 会话保存与继续。
- MCP stdio 外部工具服务器：工具发现、JSON-RPC 调用与结果回填，执行前沿用审批。
- 持久化任务列表：创建、更新状态和查询；任务跟随当前会话恢复。
- 本地 Hooks 生命周期：会话开始/结束、用户输入、工具执行前后、停止检查、上下文压缩前后；支持匹配、阻断、输入更新与附加上下文。
- Git 仓库只读操作：状态、差异和提交摘要，使用 Git 子进程执行而不是拼接 Shell。
- Jupyter Notebook 单元格插入、替换、删除（支持 cell-N、既有 ID 和读后编辑一致性检查）。
- Glob 文件名匹配与 Grep 多结果模式（文件名、匹配数、内容、上下文、分页、文件类型及通配符过滤）。
- LSP 语言服务协议：支持可配置本地语言服务器，定义／引用／悬停／符号／调用层级 9 类查询，文档同步及 JSON-RPC 标准消息帧。
- 经授权的 WebFetch：读取公开 HTTP(S) 文本、限制跳转和响应体积、阻止本地/内网目标；不使用原厂代理。
- 本地文件修改检查点：写入/替换/笔记本编辑前保存旧内容、查看修改历史、校验文件未被后续修改后回退。历史默认保存在工作区 `.atlas/history/`，不会自动同步。
- SDK JSON Lines 标准输入输出（`sdk` 命令）：结构化用户消息、模型消息、工具结果、请求结果及控制响应；不使用第三方私有传输。
- SDK NDJSON 安全输出对 U+2028 与 U+2029 进行转义；移植数组基本工具及 16ms UI 动画帧间隔。
- Shell 命令生命周期支持单个全局监听器、开始/完成通知、替换与注销；通知已接入真实命令工具入口。
- 底层通用状态 Store 支持变更回调、订阅和注销；清理注册表支持异步清理函数的注册、注销及并发执行。两者尚需与完整终端生命周期继续对照。
- 本地插件加载器：仅在 `enabled_plugins` 明确启用后加载本地 `plugin.json`，合并本地 MCP、Hooks 和 `plugin:skill` 技能，不连接专有插件市场。
- MCP 进程配置环境变量展开：支持 VAR 和默认值占位语法，保留缺失变量报告；支持自定义 MCP 环境字典。
- Bash 工具第一行注释标签和默认启用的工作树模式。
- 18 项原有工具名常量已转为公开 Rust 常量；对已实现工具增加外部名称兼容入口。
- 标准零用量对象与可订阅的压缩警告状态，MCP 服务名使用通用 ASCII 归一化（原厂专属规则不保留）。
- 多搜索服务 WebSearch：支持用户指定 SearXNG 自建实例或 Brave 搜索接口，不依赖任何固定 AI 服务登录。
- 后台 Shell 执行：支持 `run_command.run_in_background`、`task_output` 状态和阻塞读取、`task_stop` 停止（兼容历史别名 `KillShell` 和 `shell_id`），以及 `background_list`/`/tasks` 列表；日志保存在 `.atlas/background`，进程仅在当前 Agent 存活期间运行。
- 后台任务输出格式：支持 TASK_MAX_OUTPUT_LENGTH 默认 32,000、上限 160,000 UTF-16 单位，尾部截断时附带本地日志位置；极端 Unicode 代理项及输出盘容量仍待原版对照。
- 项目长期记忆：自动载入工作区 `.atlas/memory/MEMORY.md`（200 行/25 KB 上限），扫描最多 200 个 Markdown 主题文件，由当前所选模型筛选最多 5 个相关记忆，严格限制文件路径；不使用原厂记忆服务器或遥测。
- OpenAI 兼容 SSE 流式生成：文本增量、分片函数调用重组、结束标识及 Token 用量；可在提供商配置添加 `stream:true` 开启，SDK 输出 `assistant_delta` 事件（Gemini 暂时回退非流式）。
- `spawn_agent` 子智能体工具：独立会话、模型可覆盖、最多一级子任务；可选择临时 Git 工作树隔离，未修改自动清理，有修改保留路径与分支供复查。
- `spawn_team` 多智能体并行任务：1—4 个成员独立上下文与模型选择、并发执行、结果按成员输入顺序汇总；可逐成员建立 Git 工作树。
- 独立编写的单元测试和 GitHub Actions 编译测试。

## 快速开始

安装 Rust 1.82+ 后在仓库执行：

```bash
cargo build --release
cargo run -- models
cargo run -- init
```

默认使用本地 Ollama。准备模型（需要支持函数调用）：

```bash
ollama serve
ollama pull qwen2.5-coder:7b
cargo run -- chat
```

选择其他模型：

```bash
export OPENAI_API_KEY="your-api-key"
cargo run -- --provider openai --model gpt-4.1-mini run "检查目录内的代码结构"

export GEMINI_API_KEY="your-api-key"
cargo run -- --provider gemini chat
```

无需固定账号也可使用自建兼容服务：运行 `cargo run -- init`，编辑配置文件中 `base_url`、`model`、`api_key_env`，或添加新的 `openai-compatible` 配置。配置默认在系统配置目录下的 `atlas-agent/config.json`；可以用环境变量 `ATLAS_CONFIG` 指定位置。

## 本地 Hooks（不上传事件）

可在配置 JSON 的根对象加入 `hooks` 数组，规则包含 `event`、`matcher`（可选正则）、`command`、`timeout_seconds`（秒）。默认不运行任何 Hook。

```json
"hooks": [
  {
    "event": "PreToolUse",
    "matcher": "^run_command$",
    "command": "my-local-policy-check",
    "timeout_seconds": 10
  }
]
```

Hook 经 stdin 接收 JSON，返回 JSON（`decision:block`、`hookSpecificOutput.additionalContext` 等）；退出码 2 可阻断执行。用户配置的命令在本地运行，必须自行确认其可信度和对外联网行为。

## LSP 语言服务（无需绑定任何厂商）

可在配置 JSON 的根对象添加：

```json
"lsp_servers": [
  {"id":"rust","command":"rust-analyzer","args":[],"extensions":["rs"],"language_id":"rust"}
]
```

服务器从用户指定的本地可执行程序启动，不会自行下载；按文件后缀选择语言服务。Rust LSP 客户端已通过模拟语言服务器启动、文档同步和查询的端到端测试，不等同所有实际语言服务器均已验收。

## 模型流式输出

在支持 SSE 的 OpenAI 兼容提供商配置中设置 `"stream":true`。JSON Lines SDK 可实时收到 `{"type":"assistant_delta","delta":"..."}` 事件，并在模型结束后收到完整 `assistant` 与 `result`。未开启该选项的提供商继续使用既有非流式协议，Gemini 暂使用非流式模式。

## WebSearch 接入

本项目不绑定搜索平台；可配置自建 SearXNG（无固定账户）或自行持有密钥的 Brave 搜索服务。配置根对象示例：

```json
"search_backend": {"kind":"searxng","base_url":"http://127.0.0.1:8080/"}
```

也可设置 `"kind":"brave"`、`"base_url":"https://api.search.brave.com/"` 和 `"api_key_env":"BRAVE_SEARCH_API_KEY"`。搜索工具执行同样使用联网审批。

## 插件配置

插件放在工作区 `.atlas/plugins/<插件标识>/plugin.json`，并通过配置文件中 `"enabled_plugins":["formatter"]` 明确启用。插件清单可含 `hooks`、`mcp_servers` 字段，技能放在插件目录下 `skills/<技能名>/SKILL.md`，通过 `read_skill` 的 `plugin:skill` 名称读取。

插件只从本地加载，不执行自动下载或更新；任何配置的 Hook/MCP 子进程可能具备外部访问权限，启用前应审查内容。

## 本地 SDK 标准输入输出

`cargo run -- --approval read-only sdk` 从标准输入接收每行一个 JSON 消息：

```json
{"type":"user","content":"检查当前项目"}
```

返回以 `system`、`assistant`、`tool_result`、`result` 等事件组成的 NDJSON。支持 `{"type":"control","action":"model","provider":"local","model":"..."}`、`clear`、`compact`，无需私有 SDK 或账号登录。SDK 默认非交互，写入类工具需要明确 `--approval full`。

## MCP 本地工具

在原有 JSON 配置的根对象增加 `mcp_servers`：

```json
"mcp_servers": [
  {"id":"filesystem","command":"my-mcp-server","args":["--stdio"]}
]
```

当前支持本地 stdio 传输；外部工具执行必须通过审批。

## 执行权限

- `--approval read-only`：只能读、搜索文件。
- `--approval ask`（默认）：交互式对写文件和 Shell 单次确认；`run` 非交互命令会拒绝需要确认的操作。
- `--approval full`：用户明确授予写文件和 Shell 执行权限，**Shell 可以访问工作区以外的系统资源**，不要在不可信工作目录或提示词中使用。

例子：

```bash
cargo run -- --workspace ./demo chat
cargo run -- --approval full run "修改 README 并运行测试"
cargo run -- chat --resume <上次输出的UUID>
```

聊天支持 `/model <提供商> [模型]`、`/session`、`/compact`、`/clear` 和 `/exit`。

## 功能边界与后续对照

| 能力 | 状态 |
| --- | --- |
| 多提供商配置与可切换模型 | 已实现 |
| 文本模型工具调用循环 | 已实现 |
| 基础代码与扩展工具 | 已实现部分工具集；仍需按原版全量核对 |
| 会话恢复、项目规则与任务状态 | 已实现 |
| 文件路径隔离与审批 | 已实现（Shell 权限依赖用户授权） |
| 历史摘要压缩 | 已实现（基于字符阈值，不是精确 Token 管理） |
| 已知 Token 使用量统计 | 已实现 OpenAI 兼容与 Gemini 用量统一 |
| OpenAI 兼容 SSE 流式输出 | 已实现可选文本增量、函数调用分片及用量 |
| Gemini 原生流式输出与价格成本统计 | 未实现 |
| MCP stdio 客户端 | 已实现基础工具发现和调用 |
| 项目技能读取 | 已实现基础 SKILL.md 发现和加载 |
| 本地 Hooks 基础生命周期 | 已实现（同步命令 Hook） |
| LSP 标准请求客户端 | 已实现基础查询，尚未完成各服务器端到端兼容验收 |
| 本地插件装载和组合 | 已实现启用的 Hooks、MCP、技能 |
| 插件市场安装/更新、完整异步 Hooks | 未实现 |
| 独立子 Agent 与 Git Worktree | 已实现一级委派与 Git 工作树隔离 |
| 并发 Agent 团队任务 | 已实现基础 1—4 成员执行与结果汇总 |
| 团队持久通信、后台任务和取消 | 未实现 |
| SDK JSON Lines 标准输入输出 | 已实现基础多轮交互、工具及控制消息 |
| 完整终端 UI、完整 SDK 控制协议、远程桥接、语音和 IDE | 未实现 |

具体分层及迁移清单见 `docs/MIGRATION.md`。

## 数据保护与遥测

默认不做账号登录、行为上报或环境数据回传，联网仅由用户选择的模型服务、配置的 MCP / Hooks、或授权的 Shell 命令触发。边界和测试见 [`docs/PRIVACY.md`](docs/PRIVACY.md)。

## 独立性

仓库不包含任何第三方反编译或提取源码，不包含其商标、账号登录、遥测或专有接口代码。参考的是通用编程智能体功能类别，并独立实现 Rust 版本。模型提供商名称仅用于描述可选兼容协议，不代表关联或背书。

## 完整任务管理调用链进展

新增 TaskCreate、TaskGet、TaskList、TaskUpdate 多工具联动，磁盘任务高水位编号、跨进程文件锁、双向依赖、元数据合并与删除级联、已完成依赖释放、团队任务列表共享、TaskCreated/TaskCompleted 本地 Hooks。尚缺原版任务后台所有方法及全功能对照，参见 docs/TASK_GRAPH_PARITY_AUDIT.md。

## 内存邮箱（尚未全等）

工作进程内有独立 Mailbox，提供有序队列、谓词收取、等待者、修订号和消息通知；与磁盘团队收件箱职责不同。已绑定 Agent/SDK 标准输入输出消息控制。React 交互、JS Set 迭代/异常语义尚需逐项移植。

## 用户权限规则（部分等价）

除全局工具审批模式外，配置支持以下可选权限规则数组（需放入完整配置对象）：

```json
"permission_rules": [
  {"behavior":"allow","rule":"Bash(git status)"},
  {"behavior":"ask","rule":"Bash(git reset *)"},
  {"behavior":"deny","rule":"Bash(git push *)"}
]
```

明确拒绝规则优先于允许规则；只读模式不可通过静态允许规则绕过；强制再次询问规则即使在全量授权模式下也需交互确认。子 Agent 和团队成员沿用相同策略。**尚未达到原版复杂命令分类器、全部权限来源与权限模式状态机的完整等价。**

### 多模型费用统计

配置文件中可增加 model_prices 数组。每条规则使用 provider 和 model 精确匹配；input_per_million、output_per_million 以及可选 cache_read_per_million、cache_write_per_million、web_search_per_request 均以美元为单位，费率由用户根据所接入平台自主填写。

示例：

```json
"model_prices": [
  {
    "provider": "openai",
    "model": "my-model",
    "input_per_million": 1.0,
    "output_per_million": 3.0,
    "cache_read_per_million": 0.2,
    "cache_write_per_million": 0.0,
    "web_search_per_request": 0.0
  }
]
```

运行时输入 /cost，或在 SDK 中发送 {"type":"control","action":"cost"} 查看逐模型成本与用量。费用随会话保存和恢复；未配置费率时显示价格未知，不推测第三方报价。

模型费用查询 `/cost` 还显示当前会话累计的 API/工具/交互耗时（毫秒）以及文件修改增删行数。新增与替换操作的行数来自实际文本差异，费用与统计均随会话恢复；未知模型单价仍然明确提示。

子代理 `spawn_agent` 与并行团队 `spawn_team` 的费用和 Token 会合并进入父会话总览，同时保留子会话各自的历史文件和独立执行结果。

缓存计费的输入口径可按模型配置：`cache_read_included_in_input` 默认为 `true`，`cache_write_included_in_input` 默认为 `false`。如果模型服务把缓存读取独立于 `input_tokens` 上报，将前者设置为 `false`，避免错误扣减；如缓存创建已包含在输入 Token 总量，将后者设置为 `true`。两项均属于每个 `model_prices` 条目的可选布尔字段。

增加了按插入顺序保留项目的集合操作与分组功能（`src/collections.rs`），面向 Rust `Eq+Hash` 泛型键；跨语言 JavaScript 特殊键/对象相等语义仍在迁移核对中。

### SDK 用户消息优先级队列

以 `sdk` 子命令发送 `{"type":"user","content":"任务","queued":true,"priority":"now"}` 可排队；优先级支持 `now`、`next`、`later`。发送 `{"type":"control","action":"queue_drain","max_priority":"next"}` 将主 Agent 的 `now/next` 消息按优先级和同级 FIFO 执行，其他优先级或指定给其他 agentId 的消息仍保留。`queue_snapshot`、`queue_peek`、`queue_dequeue`、`queue_pop_editable`、`queue_remove_identity`、`queue_remove_agent`、`queue_clear` 可查看和操纵本地队列。Bash、孤立权限审批、内嵌图片的队列项在该 SDK 的纯文本执行通道暂不自动执行，以免误当模型文本或丢失多模态内容。

`read_skill` 工具现在可携带 `arguments`，例如 `{"name":"review","arguments":"src/lib.rs 'security audit'"}`；会根据 SKILL.md frontmatter 的 `arguments` 参数名单替换 `$0`、`$1`、`$ARGUMENTS` 和命名占位符。未传参时返回原始内容，未使用占位符时默认附加 `ARGUMENTS: ...`。该替换不执行任何 shell 命令。

SDK 队列的普通 prompt 会在 workload 与 isMeta 相同时按到达顺序自动合并为一次模型调用。可在排队的 user 消息中增加 `workload` 区分独立任务；`queue_drain` 仍逐条返回消息 ID 的处理结果，但相同批次不会重复计费。Shell、权限审批、斜杠及图像消息不会被冒充普通文字参与合并。

SDK 排队的 `task-notification` 消息现在会在调用模型之前产生单独的 `{"type":"system","subtype":"task_notification"}` 事件，包含任务状态、任务 ID、摘要和可选使用量；无状态进度消息不会被标记为已完成。

### 通用 MCP HTTP 连接

`mcp_servers` 配置项可选使用 `{"id":"remote","type":"http","url":"https://example.net/mcp","headers":{"Authorization":"Bearer ${MCP_TOKEN}"}}`；`type` 未指定时保留原有 stdio 启动方式。HTTP 使用标准 MCP 2025-03-26 的初始化、会话 ID、SSE/JSON 响应解析、工具发现和调用，超时 60 秒，禁止带用户凭据的 URL 及覆盖协议保留 Header；HTTP 配置只返回 URL，不把自定义 Header 作为 SDK 状态回传。尚未实现通用 OAuth、HTTP 会话自动重连和完整 MCP 资源/提示协议。

MCP 支持 SDK 控制：`mcp_resources`、`mcp_resource_templates`、`mcp_prompts` 查询所有连接的 MCP 服务目录（单服务失败不影响其他成功结果）；`mcp_read_resource` 需提供 `server` 和 `uri`；`mcp_get_prompt` 需提供 `server`、`name` 和可选字符串键值 `arguments`。`control_request.request.subtype` 与 `control.action` 均支持这五项。返回完整 MCP JSON 内容块，包含资源的二进制 base64 与提示结构。该部分仍未覆盖完整 MCP 订阅/鉴权/提示作为 slash 命令等功能。
