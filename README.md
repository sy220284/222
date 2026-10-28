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
- 本地插件加载器：仅在 `enabled_plugins` 明确启用后加载本地 `plugin.json`，合并本地 MCP、Hooks 和 `plugin:skill` 技能，不连接专有插件市场。
- 多搜索服务 WebSearch：支持用户指定 SearXNG 自建实例或 Brave 搜索接口，不依赖任何固定 AI 服务登录。
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

服务器从用户指定的本地可执行程序启动，不会自行下载；按文件后缀选择语言服务。仅当前 Rust LSP 客户端协议与消息帧实现通过自动测试，不等同所有语言服务器都已端到端验收。

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
| 6 类基础代码工具 | 已实现 |
| 会话恢复、项目规则与任务状态 | 已实现 |
| 文件路径隔离与审批 | 已实现（Shell 权限依赖用户授权） |
| 历史摘要压缩 | 已实现（基于字符阈值，不是精确 Token 管理） |
| 已知 Token 使用量统计 | 已实现 OpenAI 兼容与 Gemini 用量统一 |
| 流式文本和价格成本统计 | 未实现 |
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
