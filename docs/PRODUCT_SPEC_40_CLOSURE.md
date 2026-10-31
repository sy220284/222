# Claude Code 2.1.88 增强规格逐文档闭环进度

核对源包：`claude-code-2.1.88-main(2).zip`。规格归属表 1,902 个第一方源文件的 SHA-256 已与源包逐个比对，全部一致。

本表为增强规格初始归属快照；后续每项真实开发状态继续以 `docs/FILE_PORT_MATRIX.md` 为唯一事实源。

> 依据增强版规格归属表和 docs/FILE_PORT_MATRIX.md 生成；数字仅为原路径台账，不等于真实行为验收。
> 源码校验、运行入口、错误/取消/恢复和双端测试均通过后才可关闭一份文档。

| 文档 | 源路径 | 已登记验收 | 部分实现 | 未迁移 | 批准排除 | 文档闭环 |
|---|---:|---:|---:|---:|---:|---|
| 01_启动入口与运行模式.md | 30 | 0 | 16 | 6 | 8 | 未闭环 |
| 02_交互输入、消息队列与命令分派.md | 10 | 0 | 5 | 5 | 0 | 未闭环 |
| 03_会话主循环、流式事件与工具往返.md | 15 | 1 | 13 | 1 | 0 | 未闭环 |
| 04_模型 API、流式传输与自动重试.md | 26 | 1 | 12 | 1 | 12 | 未闭环 |
| 05_模型选择、思考档位和请求预算.md | 22 | 0 | 12 | 10 | 0 | 未闭环 |
| 06_系统提示词与上下文构建.md | 17 | 0 | 12 | 5 | 0 | 未闭环 |
| 07_上下文压缩、微压缩与恢复.md | 14 | 0 | 11 | 3 | 0 | 未闭环 |
| 08_工具注册、可见性和并发调度.md | 36 | 4 | 29 | 3 | 0 | 未闭环 |
| 09_终端命令执行与后台输出.md | 39 | 3 | 33 | 3 | 0 | 未闭环 |
| 10_文件读取、媒体类型和读取缓存.md | 24 | 0 | 15 | 8 | 1 | 未闭环 |
| 11_文件写入、编辑匹配和安全预览.md | 14 | 0 | 13 | 1 | 0 | 未闭环 |
| 12_文件搜索与代码检索.md | 8 | 0 | 6 | 2 | 0 | 未闭环 |
| 13_Notebook 与结构化文档编辑.md | 9 | 1 | 6 | 2 | 0 | 未闭环 |
| 14_联网搜索、抓取与网页内容转化.md | 9 | 0 | 9 | 0 | 0 | 未闭环 |
| 15_权限规则、用户审批与自动授权.md | 32 | 0 | 27 | 5 | 0 | 未闭环 |
| 16_沙箱、进程约束与安全边界.md | 54 | 1 | 38 | 15 | 0 | 未闭环 |
| 17_计划模式、待办事项与任务列表.md | 26 | 7 | 18 | 1 | 0 | 未闭环 |
| 18_子代理启动、任务执行与结果回收.md | 23 | 0 | 23 | 0 | 0 | 未闭环 |
| 19_团队代理、消息传递与协调.md | 15 | 3 | 11 | 1 | 0 | 未闭环 |
| 20_MCP 服务发现、连接与资源.md | 43 | 2 | 34 | 5 | 2 | 未闭环 |
| 21_MCP OAuth、服务授权与 Elicitation.md | 5 | 0 | 5 | 0 | 0 | 未闭环 |
| 22_插件市场、安装、启停与缓存.md | 29 | 0 | 25 | 4 | 0 | 未闭环 |
| 23_技能发现、斜杠命令与自定义 Agent.md | 65 | 0 | 43 | 19 | 3 | 未闭环 |
| 24_Hooks 生命周期、拦截与命令执行.md | 90 | 1 | 46 | 38 | 5 | 未闭环 |
| 25_会话落盘、恢复、分支与回滚.md | 32 | 1 | 19 | 7 | 5 | 未闭环 |
| 26_配置、策略、信任与环境变量.md | 43 | 0 | 24 | 11 | 8 | 未闭环 |
| 27_登录认证、账户信息与订阅限额.md | 21 | 1 | 13 | 1 | 6 | 未闭环 |
| 28_终端界面、渲染、滚动与对话显示.md | 496 | 1 | 294 | 184 | 17 | 未闭环 |
| 29_快捷键、Vim 编辑与命令输入历史.md | 23 | 0 | 20 | 3 | 0 | 未闭环 |
| 30_IDE、LSP、编辑器事件与代码定位.md | 20 | 0 | 14 | 5 | 1 | 未闭环 |
| 31_远程会话、桥接、桌面与浏览器协作.md | 60 | 0 | 5 | 9 | 46 | 未闭环 |
| 32_语音、图片、PDF 与多模态输入.md | 281 | 4 | 175 | 83 | 19 | 未闭环 |
| 33_后台任务、计划调度与通知.md | 13 | 0 | 5 | 6 | 2 | 未闭环 |
| 34_记忆目录、长期记忆与自动整理.md | 24 | 1 | 19 | 3 | 1 | 未闭环 |
| 35_观测指标、费用、日志和诊断.md | 19 | 0 | 8 | 1 | 10 | 未闭环 |
| 36_版本更新、安装、平台和原生组件.md | 10 | 0 | 6 | 3 | 1 | 未闭环 |
| 37_Git 操作、工作树与文件差异.md | 15 | 2 | 12 | 1 | 0 | 未闭环 |
| 38_结构化结果、问答互动与扩展输出.md | 9 | 0 | 8 | 1 | 0 | 未闭环 |
| 39_文件变更监控、项目监听与热更新.md | 5 | 0 | 4 | 1 | 0 | 未闭环 |
| 40_内置命令完整注册与行为入口.md | 176 | 4 | 115 | 12 | 45 | 未闭环 |

原版规格路径 1902；仓库冻结矩阵 1906（包含 4 条未纳入第一方文档的路径）。

## 2026-10-31 本轮源码对照与真实实现记录

本节只追加可复核证据，不改变上表由唯一源路径台账生成的原始数字。文档维持「未闭环」，各函数的当前 HEAD CI 和跨模块验收仍须单独检查。

| 功能文档 | 原源码入口 | 真实 Rust 入口 | 本次修复及测试 | 遗留验收 |
|---|---|---|---|---|
| 02 交互输入、消息队列与命令分派 | `messageQueueManager.ts::isSlashCommand`；`processUserInput.ts::processUserInputBase` | `message_queue.rs::Command::is_slash/is_input_slash`；`sdk_queue.rs::control` | 队列判定允许前导空格 trim；SDK 模型输入必须从第一个字符开始为斜杠，图文末文本块亦遵循，含非文本尾块/跳过标记测试，`4c00fcc` | 命令注册及弹窗、粘贴/@附件、SDK全模式 |
| 02 交互输入、消息队列与命令分派 | `cli/print.ts::drainCommandQueue` | `sdk_queue.rs::latest_batch_checkpoint_uuid`、`Agent::ask_with_blocks_for_message` | 批量提示合并采用最后一条存在的有效 UUID，保留历史别名，新增多条及缺失 UUID 回归，`4c00fcc` | 完整 SDK 同步/异步回放、取消、端到端恢复 |
| 03 会话主循环、流式事件与工具往返 | `services/tools/toolOrchestration.ts::runToolsConcurrently`；`utils/generators.ts::all` | `agent.rs::finalize_readonly_result`；`tool_orchestration.rs::completed_as_ready` | 并行只读工具谁先完成先回填独立 tool ID、PostToolUse 警告、会话记录与持久化，增加受控快慢完成先后测试，`f9502fd`、`c15b8e` | 真实并发 Hook/MCP/TUI 集成、Stop Hook/异常/取消/重启、工具上下文修改 |
| 40 内置命令完整注册与行为入口 | `commands.ts::formatDescriptionWithSource`（728-754）；`getCommand/findCommand`（704-719）；`clearCommandsCache` 五连清（523-539） | `command_manifest.rs::commands/lookup/visible`；`plugins.rs::command_descriptors/list_commands` | 实锤缺口：插件命令描述缺 `(pluginName) ` 来源前缀，已按原版 formatDescriptionWithSource 实装并新增描述符前缀回归测试；未知命令经 lookup 不命中按原版 print 语义落普通消息（`sdk_queue.rs:190` 同口径）；无 memoize 架构下缓存清理链由 reload-plugins 真实重载等价（`main.rs` `/reload-plugins` 重读 hooks+MCP+命令发现）；MCP_SKILLS 为编译期 define 源码包不可证实默认值，记待验不冒修 | 88 注册对象在默认/简化/非交互/禁用开关四环境可达性矩阵；prompt 命令完整展开链 |
| 39 文件变更监控、项目监听与热更新 | `utils/hooks/fileChangedWatcher.ts::initializeFileChangedWatcher/resolveWatchPaths`（28-108）；`hooks.ts::executeEnvHooks` FileChanged input（4289） | `file_changed_watcher.rs::spawn/resolve_watch_paths/diff_events`（全新）；`hooks.rs::HookRunner::file_changed_rules`；`main.rs chat()` 接线 | FileChanged 监听链全新落地：matcher 管道分隔清单→会话内 mtime 轮询（离线无 chokidar 等价 crate）→change/add/unlink→hook 执行→system 消息在下一输入边界显示，Drop 自动清理句柄；6 测试（清单解析三形态/空清单不启动/他事件不贡献/快照 diff 三事件/无变化零事件/input 形状）；HookEvent 补 PartialEq | CwdChanged/ConfigChange 尚无触发点（TUI 无 cd、settings 运行中不重读，属后续任务）；轮询延迟与 chokidar 事件驱动的可观察差异；watcher 不随 reload-plugins 重建 |

### 工具参数校验与发现链的源码级补充（第03文档跨第08、14、23模块）

| 原版代码位置 | Rust生产路径 | 已提交行为与回归 | 仍未闭环 |
|---|---|---|---|
| `toolOrchestration.ts::partitionToolCalls` | `tool_orchestration.rs::partition_tool_calls` → `tools.rs::validate_call_before_hooks` | 原版工具 `inputSchema.safeParse` 先于并发判定，畸形JSON及 schema 失败隔离为单独调用，`9c30863` | 所有工具参数 schema、MCP与安全审批全矩阵 |
| `WebFetchTool.ts::inputSchema` | `tools.rs::validate_web_tool_input_schema` → `webfetch.rs::definition` | 严格对象、URL/提示必填、未知字段拒绝；模型公开参数同步，`c5c8c41`、`07556b1` | 网页摘要、权限动态交互、错误协议 |
| `WebSearchTool.ts::inputSchema` | `tools.rs::validate_web_tool_input_schema` → `websearch.rs::definition` | 查询长度按UTF16、严格可选域名数组、拒绝额外字段及移除原版不存在的limit；`c5c8c41`、`07556b1`、`4da27a0` | 原版搜索返回结构、来源引用、全平台联网链 |
| `ToolSearchTool.ts::inputSchema/searchToolsWithKeywords` | `tools.rs`、`tool_search.rs::execute` | 数字类型支持小数、零、负值并按JS slice裁切；保留原始 query；`2588957` | 延迟工具完整原版tool_reference与动态同步行为 |

### 第04份模型API与重试边界的源码级对照

| 原版代码位置 | 实际Rust运行入口 | 已修复并提交 | 仍未闭环 |
|---|---|---|---|
| `services/api/withRetry.ts::withRetry/getDefaultMaxRetries` | `provider.rs::send_model_request/model_retry_attempts_from` | 原版默认十次重试加首次请求；HTTP 429 Retry-After=0连续十一请求后原样返回最终429；通用配置 `ATLAS_MAX_RETRIES` 支持源版 parseInt 数字前缀且无效配置显式报错，`8a1bf92` | 529依来源重试策略、可取消重试等待、流式事件断连恢复、完整重试进度与错误分类 |
| 38 结构化结果、问答互动与扩展输出 | `SyntheticOutputTool.ts::buildSyntheticOutputTool`（127-163）；`AskUserQuestionTool.tsx::check/validateHtmlPreview/isEnabled`（33-265）；`mapToolResultToToolResultBlockParam`（224-244） | `ask_user_question.rs::validate/execute/format_result_for_model`；`provider.rs::set_response_schema`（OpenAI strict json_schema + Gemini responseJsonSchema 双接线）；`agent.rs::set_sdk_response_schema` | 实锤协议缺口：SDK result 事件缺 `structured_output` 字段（coreSchemas.ts SDKResultSuccessSchema:1421），已新增 structured_result_event 统一三处 turn 结果组装——schema 启用时解析 answer 附加 structured_output、非 JSON 显式失败（structured_output_payload 含 StructuredOutput schema mismatch 消息）；1 测试。问答校验链逐字对齐验证通过：问题去重（精确字符串不 trim）、选项 label 精确去重、HTML 预览三错误文本逐字一致、1-4 题边界、非交互显式拒绝 | 原版工具注入路径（SyntheticOutputTool 模型主动调用）与 222 provider 强制路径的 A/B 差异记录；schema 级 ajv 校验由服务端 strict 承担；error_max_structured_output_retries 重试循环（turn 级改造）记台账 |
| 37 Git 操作、工作树与文件差异 | `ExitWorktreeTool.ts::countWorktreeChanges/restoreSessionToOriginalCwd`（79-146）；`EnterWorktreeTool.ts::call`（77-119）；`fileHistory.ts` diff --cached --stat 5s 超时（756） | `worktree.rs::Worktree::create/finish`（166 行+2 端到端测试）；`tools.rs::definitions` git_status/git_diff/git_log；`git_workflows.rs::prompt`（review/commit/commit-push-pr 三动作） | 验证通过无需修复：变更计数语义逐点对齐（status --porcelain 行数+rev-list --count 自入口 HEAD、任一失败 fail-closed 不误删，222 以 bail 显式报错等价原版 null）；git 超时/失败走统一 git() 帮助函数 60s 超时+bail stderr；worktree 为团队代理隔离消费（agent.rs isolation==worktree） | EnterWorktree/ExitWorktree 模型工具入口未注册（原版 isWorktreeModeEnabled 硬编码 true 默认注册；222 机制在位但仅团队隔离自动使用，cwd 生命周期贯穿主会话的改造记后续）；finish 无原版 merge/删除交互流（222 一律保留人工处理，更保守）；restoreSessionToOriginalCwd 主会话 cwd 不切换差异 |
| 36 版本更新、安装、平台和原生组件 | `cli/update.ts::update`（30-422）；`autoUpdater.ts::acquireLock/installGlobalPackage/getVersionHistory/getNpmDistTags/assertMinVersion`（70-561） | `version.rs::running_version`（CARGO_PKG_VERSION+构建时间戳，1 测试）；`diagnostics.rs::inspect/render`（428 行 doctor：Version/Binary/Workspace/git 可执行/环境变量校验）；`portable_platform.rs`（WSL distro 匹配/路径互转/Git Bash 查找/引号转义/heredoc 检测） | 验证通过无需修复：doctor 覆盖版本与依赖诊断；平台转换面与原版 WSL/Windows 工具链对齐；搜索为 walkdir 纯 Rust 内置实现（原版嵌入 ripgrep 二进制——222 原生组件可用性影响面结构性消除，"tool accepts --type names without requiring an external rg"） | update/autoUpdater npm 自更新管线不适用 cargo 二进制（无远端版本检查，与 ATLAS_DISABLE_NONESSENTIAL_TRAFFIC 非必要流量原则一致）；assertMinVersion 最低版本门为 GrowthBook 远控 kill switch（tengu_max_version_config），云端远控已排除属正确缺失；getVersionHistory/getNpmDistTags npm 生态专属不移植 |
| 35 观测指标、费用、日志和诊断 | `cost-tracker.ts::formatModelUsage/addToTotalSessionCost/formatTotalCost/getStoredSessionCosts`（87-323）；`log.ts::logError/loadLogList/attachErrorLogSink/getLogDisplayTitle`（30-352） | `cost.rs::CostLedger/ModelPrice/format_summary/cost_from_tokens`（7 测试+config model_prices 自定义价表）；`main.rs::/cost /usage /stats` 命令；`diagnostics.rs` doctor | 费用与诊断面验证通过：CostLedger 合并/时长累计/变更行数/未知价格标记/格式化摘要含测试，价格走 config.json model_prices 可配置；/cost /usage /stats 三命令在位；analytics sink（attachAnalyticsSink/logEventAsync）属已批准排除域 | **logError 等价日志模块缺失**：ATLAS_DEBUG_LOG_LEVEL/ATLAS_DEBUG_LOGS_DIR 仅存在于生成桩无真实实现——原版错误日志落盘（errors 目录 JSONL）、loadLogList 列表读取、getLogDisplayTitle 标题、captureAPIRequest 请求捕获均无对应；接线点分散（顶层错误出口多处）记后续任务 |
| 34 记忆目录、长期记忆与自动整理 | `memdir.ts::loadMemoryPrompt/buildMemoryPrompt/truncateEntrypointContent/buildAssistantDailyLogPrompt`（57-507）；`findRelevantMemories.ts::findRelevantMemories/selectRelevantMemories`（39-141） | `memory.rs::context*/scan/index_content`（582 行+2 测试）；`memory_extraction.rs::dream_if_due`（781 行）；`memory_age.rs::freshness_text`；`memory_command.rs` /memory；`agent.rs:5242` 接线 | 验证通过无需修复：索引双上限截断（行+字节+WARNING 标注）对齐 truncateEntrypointContent；相关性选择=模型 ranked up-to-5+已浮现排除+会话配额+最近工具警告章节对齐 selectRelevantMemories；dream 自动整理 24h+5 会话门+fs2 进程锁+符号链接防护+每文件 save 校验；surfaced/usage 回传 MemoryContext | KAIROS daily-log 模式（编译期 feature 门+claude.ai Assistant 入口专属形态）不移植；logMemoryDirCounts 遥测计数属排除域；tengu_moth_copse/hive_evidence 门为 GrowthBook 远控已断 |
