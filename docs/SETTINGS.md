# Atlas Agent 设置文档

本文档覆盖 Atlas Agent 的全部用户可配置项：配置文件、权限体系、CLI 参数与环境变量。

## 配置文件总览

| 文件 | 默认位置 | 覆盖环境变量 | 职责 |
| --- | --- | --- | --- |
| `config.json` | 系统配置目录下 `atlas-agent/config.json` | `ATLAS_CONFIG` | 模型供应商、调优、MCP、LSP、hooks、搜索后端 |
| `settings.json`（用户级） | 系统配置目录下 `atlas-agent/settings.json` | `ATLAS_USER_SETTINGS` | 个人权限规则与插件 |
| `settings.json`（项目级） | `<工作区>/.atlas/settings.json` | 无 | 团队共享权限规则 |
| `settings.local.json`（本地级） | `<工作区>/.atlas/settings.local.json` | 无 | 个人本地覆盖，通常不进版本库 |
| `managed-settings.json`（管理策略） | 见下表 | `ATLAS_POLICY_SETTINGS` | 企业统一策略，优先级最高 |

管理策略文件默认路径：macOS 为 `/Library/Application Support/AtlasAgent/managed-settings.json`，Windows 为 `C:\Program Files\AtlasAgent\managed-settings.json`，Linux 与其他平台为 `/etc/atlas-agent/managed-settings.json`。

配置根目录可用 `ATLAS_CONFIG_HOME` 整体重定向。四个 settings 来源按「管理策略、用户级、项目级、本地级」叠加生效；同一规则冲突时高优先级来源胜出。单个来源的 `permissions` 结构不符合 schema 时仅忽略该来源，不影响其他来源继续加载。

## settings.json：权限与策略

### permissions

```json
{
  "permissions": {
    "allow": ["Bash(git status)", "Read"],
    "deny": ["Bash(rm -rf *)"],
    "ask": ["WebFetch"],
    "defaultMode": "ask",
    "disableBypassPermissionsMode": true,
    "additionalDirectories": ["../shared-libs"]
  }
}
```

- `allow` / `deny` / `ask`：规则数组，语法为 `工具名` 或 `工具名(内容)`，内容支持通配。示例：`Bash(git status)` 精确放行该命令；`Read` 放行整个读取工具。规则在配置与界面之间往返转换时保持原样。
- `defaultMode`：默认审批模式，遵循来源优先级叠加，管理策略最后生效。
- `disableBypassPermissionsMode`：置为 `true` 后禁止进入不受限模式，常用于企业策略。
- `additionalDirectories`：额外授权目录，与 CLI `--add-dir` 传入的目录合并生效。

### 插件

- `enabledPlugins`：启用的插件清单，管理策略来源中同样生效。
- `pluginConfigs`：按插件 ID 的专属配置对象。

## config.json：模型与运行配置

### 供应商体系（providers）

```json
{
  "default_provider": "deepseek",
  "providers": [
    {
      "id": "deepseek",
      "kind": "openai-compatible",
      "base_url": "https://api.deepseek.com",
      "model": "deepseek-flash",
      "api_key_env": "DEEPSEEK_API_KEY",
      "stream": true,
      "tuning": { "temperature": 0.6 }
    }
  ]
}
```

- `kind` 取值：`openai-compatible`（OpenAI 兼容 Chat Completions）、`gemini`（Gemini 原生）、`chatgpt-plan`（ChatGPT 套餐 OAuth 模式）。
- `api_key_env`：密钥所在环境变量名，凭据不落配置文件。
- `stream`：SSE 流式开关；OpenAI 兼容线支持流式，Gemini 暂回退非流式。
- 预置中文厂商配置（DeepSeek、Kimi、智谱 GLM、MiniMax）与 OpenAI、Gemini、ChatGPT 套餐内置；`models` 子命令可查看、切换、调优，详见 README 模型章节。

### 模型调优（tuning）

| 参数 | 约束 | 行为 |
| --- | --- | --- |
| `temperature` | 有限值且 0–2 | 显式设置才随请求发送 |
| `top_p` | (0, 1] | 同上 |
| `max_output_tokens` | 正整数 | 同上 |
| `context_window` | 正整数 | 本地上下文容量提示，不作为 HTTP 参数发送 |
| `reasoning_effort` | 字符串档位 | 同上 |

厂商专属校验在启动与调用时执行：DeepSeek 思考模式忽略 temperature（需 `reasoning_effort` 设为 `none` 才允许设置）；DeepSeek Flash 思考模式要求 `top_p` 不低于 0.95；Kimi K3.1 Pro 仅支持思考模式；GPT-6.1 Sol 不支持 `reasoning_effort` 设为 `none`。校验失败会给出明确错误。

### 评审者与杂项

- `advisor_model`：评审者模型，独立于主循环供应商选择。
- `env`：键值映射。仅 `NODE_EXTRA_CA_CERTS` 在进程启动时应用；其余键作为存储的设置保留，不自动注入环境。
- `fileCheckpointingEnabled`：文件检查点开关，默认开启（别名 `file_checkpointing_enabled`）。
- `editor_mode`：编辑器模式，默认 `normal`。
- `additional_directories`：配置文件层面的额外授权目录。
- `default_shell`：默认 Shell 选择；`deep_link_terminal`：深链打开的目标终端。
- `github_repo_paths`：仓库路径映射。

### 指令源控制

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `instructionExcludes` | glob 数组 | 从模型上下文排除的项目、用户或本地指令文件 |
| `instructionSources` | 对象 | 按来源开关：`user`、`project`、`local`，默认全部启用 |
| `approvedExternalInstructionIncludes` | 路径数组 | 逐路径批准外部文件通过引用进入上下文 |
| `managedInstructionFile` | 路径 | 管理员提供的全局指令文件 |
| `managedRulesDirectory` | 路径 | 管理员规则目录 |

### MCP 服务器（mcp_servers）

```json
{
  "id": "my-server",
  "command": "npx",
  "args": ["-y", "my-mcp-server"],
  "env": { "KEY": "value" },
  "transport": "stdio"
}
```

- 远程形态：填 `url` 并设 `transport`，`headers` 提供静态认证头。
- `headersHelper`：Shell 命令，输出 JSON 对象作为每连接的动态 HTTP 头，适合短期令牌场景。
- 环境值支持变量展开与默认值占位语法；缺失变量会明确报告。

### LSP 服务器（lsp_servers）

```json
{ "id": "rust", "command": "rust-analyzer", "args": [], "extensions": [".rs"], "language_id": "rust" }
```

### hooks

| 事件 | 触发时机 |
| --- | --- |
| `SessionStart` / `SessionEnd` | 会话开始与结束 |
| `CwdChanged` | 工作目录变更 |
| `UserPromptSubmit` | 用户提交输入 |
| `PreToolUse` / `PostToolUse` / `PostToolUseFailure` | 工具调用前后与失败 |
| `Stop` | 响应停止 |
| `PreCompact` / `PostCompact` | 压缩前后 |
| `TaskCreated` / `TaskCompleted` | 任务创建与完成 |

hook 规则字段：`event`、`command`、`matcher`（工具事件按工具名正则过滤）、`if`（权限风格条件，仅工具事件）、`async`（后台执行）、`asyncRewake`（ implies async；以退出码 2 结束时请模型在下次唤醒继续）、`shell`、`timeout_seconds`。

### 其他

- `search_backend`：`kind`、`base_url`、`api_key_env`。
- `permission_rules`：与 settings 权限规则同语法，可置于 config.json。
- `model_prices`：自定义每 token 价格表，用于用量统计。

## CLI 参数

### 全局参数（所有子命令）

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--provider` | 配置的 `default_provider` | 供应商 id，如 `openai`、`gemini` |
| `--model` | 供应商配置的模型 | 覆盖本次调用的模型 id |
| `--workspace` | 当前目录 | 文件工具与 Shell 执行的根目录 |
| `--add-dir` | 无 | 额外授权目录，可多次传入 |
| `--approval` | `ask` | `readonly` 只读、`ask` 交互审批、`full` 不受限 |
| `--max-steps` | 24 | 单次请求的模型与工具循环上限 |

非交互模式默认拒绝写操作与 Shell，需显式设置 `--approval full`。

### 子命令

`version`、`init`、`doctor`、`models`（列表、切换、调优、probe）、`chat`、`run`、`chatgpt`（login、status、models、logout）、`oauth`（pkce、listen、login、refresh）、`mcp`、`project-init`、`agent-definition`、`keybindings`、`stats`、`git-config`、`git-state`、`git-ignore`、`resolve-deep-link`。模型与账号相关子命令的用法见 README 对应章节。

## 环境变量

### 行为开关

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `ATLAS_DISABLE_AUTO_COMPACT` | 未设置 | 禁用自动压缩 |
| `ATLAS_DISABLE_COMPACT` | 未设置 | 完全禁用压缩 |
| `ATLAS_DISABLE_FILE_CHECKPOINTING` | 未设置 | 禁用文件检查点 |
| `ATLAS_DISABLE_PROJECT_INSTRUCTIONS` | 未设置 | 不加载项目指令 |
| `ATLAS_DISABLE_GIT_INSTRUCTIONS` | 未设置 | 不加载 Git 指引 |
| `ATLAS_DISABLE_ADVISOR_TOOL` | 未设置 | 禁用评审者工具 |
| `ATLAS_DISABLE_FAST_MODE` | 未设置 | 关闭快速模式 |
| `ATLAS_ENABLE_TASKS` | 未设置 | 启用任务功能 |
| `ATLAS_PLAN_MODE_REQUIRED` | 未设置 | 强制计划模式 |
| `ATLAS_SKIP_PROMPT_HISTORY` | 未设置 | 不记录提示历史 |
| `ATLAS_DEMO_MODE` | 未设置 | 演示模式 |
| `ATLAS_DISABLE_NONESSENTIAL_TRAFFIC` | 未设置 | 隐私分级：禁用全部非必要网络流量，见 `docs/PRIVACY.md` |

### 性能与并发

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `ATLAS_MAX_TOOL_USE_CONCURRENCY` | 内置默认 | 工具并发上限 |
| `ATLAS_STREAM_IDLE_TIMEOUT_MS` | 内置默认 | 流空闲超时 |
| `ATLAS_STREAMING_TOOL_EXECUTION` | 内置默认 | 流式工具执行开关 |
| `ATLAS_SESSIONEND_HOOKS_TIMEOUT_MS` | 内置默认 | 会话结束 hook 超时 |
| `ATLAS_CONTEXT_WINDOW_TOKENS` | 模型目录或内置默认 | 上下文容量提示 |
| `ATLAS_TOKEN_BUDGET` | 内置默认 | token 预算 |

### 工具行为

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `ATLAS_GLOB_HIDDEN` | 未设置 | glob 包含隐藏文件 |
| `ATLAS_GLOB_NO_IGNORE` | 未设置 | glob 忽略 ignore 规则 |
| `ATLAS_GLOB_TIMEOUT_SECONDS` | 内置默认 | glob 超时 |
| `ATLAS_GIT_BASH_PATH` | 自动探测 | Windows 下 Git Bash 路径 |
| `ATLAS_PDF_UNSUPPORTED_MODELS` | 内置清单 | 禁用 PDF 输入的模型列表 |
| `ATLAS_OUTPUT_STYLE` | 内置默认 | 输出风格 |

### 路径重定向

| 变量 | 说明 |
| --- | --- |
| `ATLAS_CONFIG` | config.json 完整路径 |
| `ATLAS_CONFIG_HOME` | 配置根目录 |
| `ATLAS_USER_SETTINGS` | 用户级 settings.json 路径 |
| `ATLAS_POLICY_SETTINGS` | 管理策略文件路径 |
| `ATLAS_POLICY_DROPIN_DIR` | 管理策略投递目录 |
| `ATLAS_SKILL_DIR` | 技能目录 |
| `ATLAS_PLUGIN_ROOT` / `ATLAS_PLUGIN_CACHE_DIR` / `ATLAS_PLUGIN_DATA` | 插件根、缓存与数据目录 |
| `ATLAS_ADDITIONAL_DIRECTORIES_INSTRUCTIONS` | 额外目录指令 |

### 调试

| 变量 | 说明 |
| --- | --- |
| `ATLAS_DEBUG_LOG_LEVEL` | 调试日志级别 |
| `ATLAS_DEBUG_LOGS_DIR` | 调试日志目录 |
| `ATLAS_DIAGNOSTICS_FILE` | 诊断输出文件 |
| `ATLAS_SUBPROCESS_ENV_SCRUB` | 子进程环境清洗 |

## 隐私与网络

- 全部配置存储于本地文件；无遥测回传、无远端配置源、无 A/B 实验。
- 需要限制网络行为时设置 `ATLAS_DISABLE_NONESSENTIAL_TRAFFIC`，详见 `docs/PRIVACY.md`。
- 密钥一律通过 `api_key_env` 指向的环境变量提供，配置文件与设置文件不保存密钥明文。
