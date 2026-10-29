# Rust 全量迁移实况（基于逐文件台账）

## 2026-10-29：自定义智能体中文与表情长度校验（源码已提交、待验收）

对照原版 `source/src/components/agents/validateAgent.ts` 的 JavaScript `.length` 语义，修复 Rust `src/agent_definition.rs::validate` 直接使用 UTF-8 字节数导致中文、表情等多字节字符说明/系统提示词过早满足最小长度或触发过长警告的问题。现在按 UTF-16 单元分别执行描述 10/5000 与系统提示词 20/10000 的原版阈值判断。提交 `ed961085`。另对照 `source/src/tools/AgentTool/agentToolUtils.ts::resolveAgentTools`、`source/src/utils/permissions/permissionRuleParser.ts`，将 `Bash(git *)`、`Read(path)`、`Agent(Explore)` 等权限格式的工具规格先解析为基础工具名，匹配 Rust 厂商中立工具别名；保留 Agent 子代理类型规则的原版有效性，避免错误拒绝自定义智能体。提交 `012ebf7a`。真实 CLI 智能体定义校验入口保持接线；源码已提交待行为验收，未运行 Cargo/测试/CI。

## 2026-10-29：任务列表异常记录恢复与编号一致性（源码已提交、待验收）

对照原版 `source/src/utils/tasks.ts` 的 `getTask`、`findHighestTaskIdFromFiles`、`resetTaskList` 和 `TaskGetTool` / `TaskUpdateTool` 调用链，补齐 `src/task_graph.rs`：

- **任务列表异常记录容错（1 项）**：损坏 JSON、超长或溢出的数字文件名不会让 TaskList / 团队状态读取整组失败；TaskGet、TaskUpdate、认领在无效编号下按原版返回任务不存在。代码提交 `8739c3b7`、`bff79ed7`。
- **任务列表重置与编号保护（1 项）**：重置新团队时清理所有任务 JSON，包括损坏或遗留记录；重置前先保存高水位；扫描文件名计算最高编号，避免损坏记录导致后续任务 ID 复用。代码提交 `8739c3b7`、`62e919b0`；高水位持久化改为先完整写入临时文件、同步落盘再原子替换，避免重置中断造成编号回退。

本次两条可感知业务链已写入并推送，均为**已提交、待行为验收**；未运行 Cargo、测试或 CI，Issue #3 冻结条目不勾选。原版跨进程通知、完整任务状态机、任务文件级并发、完整 SDK/UI 仍待补齐。本轮 100 项独立缺口的逐项确认和方案尚未全部完成，100 项硬目标未达成；不将本次局部源码修改标为全部等价。

## 2026-10-29：本地记忆与文件上下文真实调用链补齐（本任务本轮 5/100，已提交待验收）

原始源码：`source/src/memdir/memoryScan.ts`、`source/src/memdir/findRelevantMemories.ts`、`source/src/utils/fileStateCache.ts`、`source/src/commands/files/files.ts`。Rust 可达入口：`src/agent.rs` → `src/memory.rs::context_with_recent_tools`；`/files` → `src/files_context.rs::paths`。

1. **记忆主题扫描与容错（1 项）**：扫描不再被五层目录深度和 100 KB 文件大小限制截断；只读前 30 行 frontmatter，单文件读取失败不影响其他主题。提交 `409202b2`。
2. **记忆选择故障回退（1 项）**：多模型选择请求失败或返回工具调用时，保留本地 MEMORY.md 索引，正常处理用户主请求；选中主题消失时跳过该主题继续读取其余文件。提交 `409202b2`。
3. **记忆相关性与工具历史（1 项）**：主题清单附带原版 ISO 修改时间，Agent 从真实历史工具调用提取最近 10 种工具供记忆选择器过滤重复 API 文档，保留警告与经验记录。提交 `409202b2`、`39d5265a`、`2db1722b`、`2fb3645a`。
4. **多模型结构化记忆选择（1 项）**：兼容 SDK 文本内容块的 JSON 选择结果，不再仅依赖普通文本字段。提交 `a2eb96dc`。
5. **SDK 多模态会话文件上下文恢复（1 项）**：`/files` 识别工具调用与结果的结构化内容块，过滤失败结果、去重并维持最近访问顺序，修复恢复会话后遗漏成功读取文件的问题。提交 `1fa93c31`、`fe14c67e`。

期间发现记忆检索提示的字符串转义错误，已继续修正并重新读取远端确认，修复提交 `123a8bcb`、`2fb3645a`。本轮实际涉及 **5 项独立用户业务行为**；尚差 95 项，未完成预先逐项核实的 100 项功能清单，不把文件路径数量当作功能数。原版完整 YAML frontmatter 解析、实时文件缓存/25 MiB LRU、记忆已呈现去重、完整取消/并发、SDK UI 与行为等价仍待实现或验证。按本任务现行规则未运行 Cargo、测试或 CI；Issue #3 冻结清单不勾选，以上均为**源码已提交、待验收**。

## 2026-10-29：WebFetch 与 macOS 深链终端源码补齐（本任务本轮 9/100，待验收）

原始 ZIP 源码依据：source/src/tools/WebFetchTool/utils.ts（50–95、262–329、347–482）、source/src/tools/WebFetchTool/WebFetchTool.ts（214–295）、source/src/utils/mcpOutputStorage.ts（61–135、148–174）、source/src/utils/deepLink/terminalLauncher.ts（26–120）。Rust 可达链：src/tools.rs → src/webfetch.rs；src/main.rs → src/terminal_launcher.rs → src/config.rs。

1. WebFetch 二进制附件：抓取后将 PDF、Office、图片等原始字节按 MIME 扩展名保存在工作区 .atlas/tool-results，结果包含可读取的路径及大小；附件写入失败仍保留抓取文本。提交 74dd607d、c3902378、5223732。
2. WebFetch 内容缓存：按原始 URL 与工作区隔离缓存成功响应，15 分钟 TTL、50 MiB LRU，已删除的二进制附件会使缓存失效。提交 74dd607d、c3902378。
3. HTML 转 Markdown：保留标题、链接、列表、强调、代码块、图片描述、引用及常见实体；原先会直接丢弃标签语义。提交 849fd92e。仍缺 Turndown 完整 DOM/复杂表格/全部嵌套结构。
4. 跨域跳转继续请求：重定向消息保留原始 prompt 和目标 URL，给出再次授权调用参数，补齐请求耗时回执。提交 de9a9dc1。
5. WebFetch HTTP 行为：60 秒超时、10 次重定向、301/302/307/308 白名单及 Markdown 优先内容协商。提交 112d8e1c。
6. macOS 深链终端发现：持久偏好、TERM_PROGRAM、Spotlight、Applications 顺序选择，保留原有 Terminal.app 失败回退。提交 44f3c38d、3219e15f、5df20c62。
7. 用户终端偏好设置：真实 CLI terminal-preference 命令支持查看、保存和 auto 恢复，供浏览器无环境启动使用。提交 fc19dd1c。

8. URL 校验与网络出口诊断：按原版 UTF-16 单元限制 2,000 字符及公网主机结构，区分 403 blocked-by-allowlist 代理阻断和普通 HTTP 错误，返回 HTTP 状态说明。提交 1fa2f9ad。
9. 大型 WebFetch 结果持久化：实际工具执行器不再将超过 16,000 字符的抓取结果静默截断，完整 JSON 保存在工作区 .atlas/tool-results，返回保存路径、预览和分页读取说明；拒绝跨工作区符号链接。提交 e1a1fe02、4c10643e。

严格状态：9 项独立用户业务行为已有新 Rust 代码并推送，均为“已提交、待验收”；没有运行 Cargo、测试或 CI。本次未建立完整且逐项核实的 100 项独立业务缺口清单，未达到 100 项硬目标，差 91 项。WebFetch 仍缺二次模型摘要、完整取消状态机、会话级附件目录及完整 DOM 等价；macOS 终端缺完整交互偏好 UI 和跨平台行为证据。Issue #3 冻结 362 项仍开放、未勾选；动态矩阵不升级为已验收。


## 2026-10-29：WebFetch 大页面与深链终端发现（本任务本轮 2/100，已提交待验收）

- **网页抓取（1 项）**：原版 `source/src/tools/WebFetchTool/utils.ts:112,128,275–279,375–379,492–496` 对普通 HTTP 初始请求先升级 HTTPS，响应上限 10 MiB，模型处理前按 100,000 个 UTF-16 单元截断并附提示。Rust `src/webfetch.rs::fetch_with_redirect_policy` 已补入真实网络执行链：生产请求升级 HTTPS（本地测试专用 override 保持 HTTP）、10 MiB 限制及内容截断提示。提交 `82c280778deb0069057c2f9276451359ee1de79b`。**仍缺**原版 HTML→Markdown、缓存/TTL、二进制附件落盘、模型摘要与完整取消时序；本轮未运行测试或 CI，未验收。
- **深链打开终端（1 项）**：原版 `source/src/utils/which.ts::which` 与 `source/src/utils/deepLink/terminalLauncher.ts::detectLinuxTerminal` 只选可执行程序；Rust `src/terminal_launcher.rs::which` 原先仅检查 `is_file`，可能把无执行权限的假终端当作首选，导致浏览器深链无法打开。已在真实 `detect_terminal → build_terminal_plan → launch_in_terminal` 链上按 POSIX 可执行位过滤，提交 `f94e5c57bc5a584054dee7c3368d36485e3b051c`。**仍缺**macOS 持久偏好与安装探测、Windows 完整回退；本轮未运行测试或 CI，未验收。
- **严格计数**：本任务本次可证实的独立用户功能新代码提交 **2/100**；**完整验收 0**，未达到 100 项硬目标。原始 ZIP `claude-code-2.1.88-main(1).zip` 已在运行环境校验完整；未完成 100 个独立可处理业务缺口的完整去重清单与逐项补齐，不能用原始源路径数替代。Issue #3 冻结 362 条继续开放。当前共享分支有其他任务并发提交，提交前均复核 HEAD，未覆盖同期工作。

## 2026-10-29：Grep、网页重定向与工作树退出的源码级缺口补齐（已提交待验收）

- **Grep 搜索主链**：对照原版 `src/tools/GrepTool/GrepTool.ts:104–126, 314–570`，修复 Rust `src/search.rs` 的跨行正则匹配、匹配行数统计（同一行多次出现不重复计数）、`context/-C` 对 `-A/-B` 的优先级、`-n` 行号开关、`head_limit=0` 不限量及分页后计数。真实模型工具定义同步公开 `-n`。提交 `19073b725e1eb942a2829ca52daad215e2d6764e`。仍缺原版 ripgrep 忽略规则、超长行、取消及全部匹配边界。
- **网页抓取跨域跳转**：对照原版 `src/tools/WebFetchTool/WebFetchTool.ts:214–249`、`utils.ts:205–243`，Rust `src/webfetch.rs` 现在对非同来源（含协议/端口变化）跳转返回目标 URL 而非自动抓取，要求 Agent 新一轮工具调用重新授权；保留显式拒绝优先，并把经过公网检查的 DNS 地址绑定到实际请求，避免解析后重绑定。提交 `0d70b71c3e44e569ba980ba2a05da794a497bdf6`、`eeac41f1338d6a5dc185a2f833e429c9e6ba4e21`。仍缺原版 Markdown 提取、缓存和完整网页摘要链。
- **工作树退出保护**：对照原版 `src/tools/ExitWorktreeTool/ExitWorktreeTool.ts:79–112, 190–225`、`src/components/WorktreeExitDialog.tsx:39–79`，Rust `src/worktree.rs` 保存创建时 HEAD 基线，在退出时统计新提交；即使目录没有未提交文件，只要分支存在新提交也保留工作树，避免自动删除用户成果。提交 `f8d8840fb9aca6dfffda78be47cd80d9e77e946a`。仍缺原版保留/删除交互、命名复用、恢复和沙箱/Hook。
- **工具发现与关键词检索**：对照 `src/tools/ToolSearchTool/ToolSearchTool.ts:129–160, 218–300`，Rust `src/tool_search.rs` 接入原版完整名称回退评分及 searchHint 高优先级匹配，使名称分词不完整但提示明确的工具可被发现。提交 `d974b491b2bbd59c6c6e90ec91b7c23c901819d0`。仍缺完整原版 deferred/tool_reference 协议。
- **PowerShell 模型输入兼容**：对照 `src/tools/PowerShellTool/PowerShellTool.tsx:230–233`、`src/utils/semanticNumber.ts` 与 `semanticBoolean.ts`，Rust `src/semantic_inputs.rs` 将 PowerShell 的字符串毫秒超时及后台执行布尔值转换为实际执行器可读类型；同次补齐 Grep 数字/布尔参数的源语义输入。提交 `a7db37058dfe1262907afa0e5d80b5e319000b45`。完整 PowerShell AST/沙箱和后台任务生命周期仍缺。
- **Grep 后续补齐（不重复计数）**：`src/search.rs` 增加多模式通配和排除过滤、Git 原生忽略规则、原版 `numLines` 回执；`src/tools.rs` 修正跨域网页跳转只检查显式拒绝，避免提前触发两次授权。提交 `e47a87bb31f85b113e17b1b848ae918537813ea3`、`6f55049701cd5ab17710e5b2cdf09e766fb9afe7`、`50b8cbe16b51633f6c69b237678f578cd8ef280f`、`876474e449123dd9f9bc7adea5cf88fe6b6a9f98`。
- **Glob 文件发现**：对照原版 `src/utils/glob.ts:66–129`、`src/tools/GlobTool/GlobTool.ts:154–171`，Rust `src/search.rs` 支持经授权搜索目录内的绝对路径通配模式、`ATLAS_GLOB_HIDDEN` 隐藏文件开关、`ATLAS_GLOB_NO_IGNORE` 是否遵循 Git 忽略规则，保留原版默认隐藏文件可见、默认不受 .gitignore 限制。提交 `091d2fe8f44d8f85b9f2310f1a7f41d47cafdc18`。完整动态数量限制、权限排除和插件缓存过滤仍缺。
- **多模型网页搜索域名范围**：对照原版 `src/tools/WebSearchTool/WebSearchTool.ts:25–36, 76–84`，Rust `src/websearch.rs` 在 SearXNG/Brave 通用后端公开 `allowed_domains`/`blocked_domains`，按域名及子域名筛选结果，禁止黑名单结果进入 Agent，并校验原版最短查询长度。提交 `d620fcb88bb556ab55a2d605fce877d3d9a828c2`、`966e590e67a2527abc505e2c6843a1971d1dee84`、`8360caae8d120c76479bac7bf48172f447a9a032`。原版模型原生检索/引用协议及服务端域名过滤仍未等价。
- **插件本地安装与启用**：对照原版 `src/services/plugins/pluginOperations.ts:321–417`、`src/utils/plugins/pluginInstallationHelpers.ts:333–460`，Rust `src/plugin_manager.rs` 在落盘前校验指定插件 ID 与 manifest 名称，拒绝名称不匹配而遗留已安装插件；成功安装后同步写入启用配置，使插件技能/命令在下一会话可用，保存失败回滚新安装目录。提交 `d41341b7e898e4dc11f4f1e33f3722dd0ca12c96`、`687aa4a8b36eeb14b48155b057576f144925584f`。原版插件依赖闭包、可信市场和多设置来源仍缺。
- **严格计数**：本轮完成 8 条互不重复的用户业务主链的真实源码修改与提交，**8/100**，其余 92 条未完成；未运行 Cargo、单元/集成测试或 CI，均仅“已提交待验收”，不新增已验收源路径，Issue #3 冻结 362 项保持开放。原始源码 ZIP 可用；本轮尚未完成 100 条独立用户业务缺口的全部源码级确认和实施方案，不能用路径候选替代。


## 2026-10-29：本地原版源码对照——会话上下文与项目引导（代码已验证，非文件级全等价）

对照用户原始 `source/src/context.ts` 与 `source/src/projectOnboardingState.ts`：
- `src/conversation_context.rs` 在 Agent 会话初始读取 Git 分支、默认分支、未提交状态、最近五次提交及配置用户名，限制 Git 状态 2000 字符，追加本地当天日期至真实模型 system 消息，启动后固定快照、不再逐轮刷新。缺完整原版记忆文件自动发现/注入过滤、设置栈和特性门控。
- `src/project_onboarding.rs` 在交互 `chat` 正式入口完成工作区是否为空、AGENTS 项目指令探测、初始化建议、每工作区最多四次展示、持久化已完成标记；`/init` 和普通 prompt 后检查指令文件。用户设置保存在配置目录而非污染 Git 工作区，原版 React 视觉引导仍待补齐。
- 使用原版 ZIP 的真实源码对照。离线 Cargo 实测：三项 Git/日期单测、两项引导状态/持久化单测、一项真实 CLI 多次进入与指令创建回归通过；整体测试已运行，其他并发改动的台账状态一致性也需同步核对。此两条源路径从未迁移调整为部分实现，不计已验收，Issue #3 冻结项仍保持开放。



## 2026-10-29：权限遮蔽告警、MCP 连接诊断与 Bash 危险命令提示（源码已提交、待验收）

- **权限规则可用性诊断（1 项）**：对照 `source/src/utils/permissions/shadowedRuleDetection.ts`、`doctorContextWarnings.ts`，在 `src/diagnostics.rs` 的真实 `/doctor` 入口读取用户、项目、本地、托管和全局有效权限，识别特定允许规则被同工具全局拒绝/询问规则遮蔽，拒绝优先，并给出处理建议。提交 `d9873b96`。当前 Rust 无沙箱自动放行来源模型，原版按来源和沙箱状态排除的特殊情况尚未等价。
- **MCP 远程连接诊断（1 项）**：对照 `source/src/services/mcp/envExpansion.ts`、`src/mcp_http.rs::connect_with_root`，`/doctor` 先展开 `${NAME}` 和 `${NAME:-fallback}`，对缺失变量提示，拒绝 URL 内嵌账号密码及与实际 HTTP 握手冲突的保留请求头，不再误判正常动态 URL 为无效。提交 `9c9df945`、`0ce1edb2`。动态助手生成的请求头、网络可达性和完整 OAuth 仍未诊断。
- **Bash 高风险操作交互提醒（1 项）**：对照 `source/src/tools/BashTool/destructiveCommandWarning.ts`，在 `src/bash_permission_suggestions.rs` 实现 Git 强制改写、删除、数据库清空、Kubernetes/Terraform 销毁等危险命令的原版告警与 `git clean --dry-run` 例外，并接入 `src/tools.rs::require_approval` 的真实交互授权界面；仅提示，不改变显式拒绝、非交互策略或已有安全审查。提交 `79e789cc`、`2a13eff5`、`1dbbdb64`。完整 Shell AST、所有复合命令与原版 UI 布局仍未等价。
- **并行冲突处理**：曾调整权限保存接口返回值，静态复查发现其既有布尔值表达“策略允许写入”而非“本次内容发生变化”，且原有重复规则调用依赖该语义；已通过 `aa2336df` 恢复，不计有效补强。
- **Bash 命令结果语义（1 项）**：对照 `source/src/tools/BashTool/commandSemantics.ts`，`src/shell_runtime.rs` 依据实际命令末段区分 `grep`/`rg` 无匹配、`diff` 文件不同、`find` 部分目录不可访问、`test` 条件为假与真正的退出失败；`src/tools.rs::run_command_with_shell` 将解释写入真实命令输出，保留 stdout/stderr 和退出码。提交 `b1d1a019`、`add9f36b`；复杂 Shell AST、Windows PowerShell 与所有输出 UI 尚未等价。
- **本次严格计数**：4 个互不重复、用户可感知功能的源码补强已提交，**4/100**，尚差 96 项。没有执行 Cargo、单元/集成测试或 CI，以上均为“已提交、待验收”，Issue #3 冻结 362 条不勾选。已从 Issue #3 和迁移矩阵抽取 100 条部分实现**源路径候选**，但源路径不能替代 100 个独立业务功能，尚需继续逐功能源码级核查、去重、明确 Rust 调用链及补齐；不能当作本轮 100 项清单完成证据。


## 2026-10-29：交互问答注释与诊断警告源码补齐（已提交、待验收）

- **交互问答（1 个独立业务功能）**：对照原版 `src/tools/AskUserQuestionTool/AskUserQuestionTool.tsx` 的严格输入 Schema、`annotations`、`mapToolResultToToolResultBlockParam` 和 `validateHtmlPreview`。Rust `src/ask_user_question.rs` 从真实终端选择收集所选预览及用户备注，不信任模型伪造的答案/注释，按原版问答顺序构建供模型继续推理的文本；`src/agent.rs` 已接通结果回传。新增可选 HTML 预览片段校验（`ATLAS_QUESTION_PREVIEW_FORMAT=html`）。提交 `2da9a86b`、`742306f0`、`3c65520d`、`948cef64`、`63656e5f`、`a2272dae`；仍缺 SDK 外部问答回执、完整预览格式配置传播和 TUI，**未做行为测试，不能标记等价**。
- **Doctor 诊断（1 个独立业务功能）**：对照 `src/screens/Doctor.tsx`、`src/utils/envValidation.ts`、`src/utils/shell/outputLimits.ts`、`src/utils/task/outputFormatting.ts`、`src/utils/doctorContextWarnings.ts`、`src/utils/claudemd.ts`，在 `src/diagnostics.rs` 补充 Bash/任务输出环境变量的无效值、上限截断警告及超过 40,000 UTF-16 字符的指令文件告警；提交 `82c8e968`、`2baec2e4`。原版诊断的其他面板、安装渠道、版本锁、MCP 工具上下文和 UI 仍缺，**未做行为测试，不能标记等价**。
- **本次严格计数**：实际提交 2 个独立用户业务功能的源码补齐，**2/100**；其他提交属于同功能完善和台账更新，不另计数。Issue #3 冻结 362 条继续开放；只在现有 Draft PR #2 分支提交，未执行测试或跟进 CI。


## 2026-10-29：MCP HTTP 动态请求头、凭据助手及远程地址变量展开（源码已提交，待验证）

- **原版依据**：`source/src/services/mcp/headersHelper.ts:30–138`、`envExpansion.ts:7–35`、`config.ts:556–615`、`client.ts:800–805`。原版在远程 MCP 连接时合并静态请求头与脚本生成的动态请求头，动态值覆盖静态值；脚本有 10 秒超时，输出必须为字符串键值 JSON 对象，失败不阻断静态连接；远程 URL 与请求头均支持环境变量替换。
- **Rust 实际补齐**：`src/config.rs` 新增 `headersHelper` 可选配置和远程 URL 环境变量校验；`src/mcp_http.rs` 在真实 HTTP MCP 握手前执行工作区内的动态凭据助手，注入中立的 `ATLAS_MCP_SERVER_NAME`/`ATLAS_MCP_SERVER_URL` 上下文、设置 10 秒限时和退出时终止子进程，校验 JSON 字符串字典，失败退回静态请求头；按 HTTP 大小写不敏感规则让动态头覆盖静态头，并展开 URL/头字段中的环境变量。`src/mcp.rs` 同步实际 MCP 配置构造字段。
- **状态与剩余差异**：代码已提交，按本轮要求未运行 Cargo、CI 或行为测试，故不登记等价验收。当前仅有用户级配置路径，原版 project/local 来源的工作区信任门禁、SSE/WS 传输、完整 OAuth 刷新、全部连接重试及动态认证生命周期仍缺。Issue #3 冻结项目不勾选，相关源文件保持部分实现。

## 2026-10-29：OAuth 浏览器回调异步时序与非法 HTTP 请求恢复（源码已修复，文件未验收）

- **原版源码对照**：用户上传 `claude-code-2.1.88-main(1).zip` 中 `source/src/services/oauth/auth-code-listener.ts` 的 `startLocalListener`（先安装请求监听、`void onReady()`，不等待浏览器启动结束）、`handleRedirect`、`validateAndRespond`、`close`；关联 Rust 实际入口 `src/main.rs` 的 `oauth-listen` 命令。
- **真实代码补齐**：`src/oauth_callback_listener.rs` 通过独立异步启动任务与 `tokio::select!` 同时等待浏览器回调，解决原 Rust 先 `await on_ready()` 再 `accept()` 导致的授权死锁；即使回调先完成，启动任务也不被取消。请求头无效、超长、超时或 URI 无法解析时发送 400/431/408 并继续等待后续合法回调，不让恶意或误发请求中断授权；16 KiB 边界按真实字节检查。保留 CSRF state 校验、回环地址限制、延后成功 302。
- **实际验证**：`oauth_callback_listener::tests::browser_callback_is_accepted_while_on_ready_is_still_pending` 使用真实 TCP 回调与未完成的浏览器启动任务证明并发与后续继续执行；`malformed_http_and_oversized_headers_do_not_abort_browser_authorization` 注入非法 UTF-8、超长头并随后用正确 code/state 完成 302。完整 GitHub Actions [40852563169](https://github.com/sy220284/222/actions/runs/40852563169) 在代码 HEAD `79333e56f758e0672d3acdc692fac8de80f29e4f` 通过格式、静态分析、依赖检查、编译和全部测试，两个新增测试均通过。
- **验收限制**：原版自定义成功回调、完整 token 客户端、SDK/UI 与取消生命周期仍缺，`src/services/oauth/auth-code-listener.ts` 继续“部分实现”，不计源文件完整验收。此轮按两个独立用户可感知的异常恢复/并发授权场景补齐，**100 项硬目标未完成**；Issue #3 冻结 362 项不勾选。


## 2026-10-29：OAuth 用户资料查询的真实 HTTP 移植

原 `src/services/oauth/getOauthProfile.ts` 两条真实请求路径迁为 `src/oauth_profile.rs`：账户 UUID + API key 查询和 OAuth Bearer 用户资料查询，10 秒超时、成功 JSON 返回、4xx/5xx、网络失败/无效 JSON 返回无资料。原固定厂商域名和请求扩展头改成用户可配置的多提供商端点。通过 Tokio 本机 TCP 服务器确认实际请求头、查询参数、真实响应与错误，不伪造网络结果。原账号资料持久化及 OAuth 主客户端尚未对接，该源文件仍是部分实现。



## 2026-10-29：OAuth 授权码回调服务的真实 HTTP 实现

对照原始 `src/services/oauth/auth-code-listener.ts`，新增 `src/oauth_callback_listener.rs`。系统临时分配本地 IPv4 回环端口，校验 GET 回调路径、授权码和 state，拒绝错误路径及 CSRF，成功时暂存连接直到上游处理授权码后回送 HTTP 302；错误时完成 HTTP 400。真实 Tokio TCP 测试覆盖路径筛选、浏览器响应先后顺序、错误 state、缺失 code 和 onReady 异常。新 `oauth-listen` CLI 接入这条真实监听路径并输出明确的授权回调数据，但多模型 OAuth 客户端的授权链接、令牌交换、刷新、持久化和各客户端连接仍在待迁移文件中，此文件暂记部分实现。



## 2026-10-29：深链来源告警与 FETCH_HEAD 核验

对照原始 `src/utils/deepLink/banner.ts` 124 行：新增 `src/deep_link_banner.rs`，使用真实工作树目录及主仓库共享目录中的 FETCH_HEAD 最近修改时间判断是否长期未拉取；按预填文本长度展示普通或滚动阅读警告，区分本地克隆匹配与外部提供的错误仓库路径；原厂 AGENTS 文档名称按要求中性化。新 `deep-link-banner` CLI 和独立集成测试验证真实 Git 仓库、长提示、未拉取、错误仓库。外部协议启动及终端唤醒链尚未全量迁移，保留部分实现标签。



## 2026-10-29：OAuth PKCE 加密与运行版本入口源码迁移

原始源文件 `src/services/oauth/crypto.ts` 的三个功能已迁入 `src/oauth_crypto.rs`：32 字节系统安全随机数、无填充 Base64URL、SHA-256 challenge 与独立 state，含 RFC 7636 校验向量和跨进程 CLI 随机性验证；`oauth-pkce` 显式调用可以直接用于多模型 OAuth 客户端，但 OAuth 授权服务完整回调、令牌保存尚未完成。

原始 `src/commands/version.ts` 已迁为 `src/version.rs`：读取当前运行二进制编译版本、可选 `ATLAS_BUILD_TIME`，`version` CLI 与聊天 `/version` 都走本地真实函数。原版专属用户开关按要求去除。两条源路径仍标注部分实现，未冒称全功能等价验收。



## 2026-10-29：会话自动命名原版近期上下文与多模态文本补全（保持部分实现）

实际阅读 2.1.88 原始 `src/commands/rename/generateSessionName.ts:10–15` 与 `src/utils/sessionTitle.ts:26–58`：原版生成名称时读取用户和助手文本、提取多模态消息中的 text 块，排除工具消息，并只向模型提供对话最后 1000 个 UTF-16 代码单元。此前 Rust `src/agent.rs::rename_session_or_generate` 取前 50 条、截取最前 8000 字符、遗漏多模态文本，导致长会话按过时话题命名，纯多模态文本对话无法自动命名。

现由 `src/session_control.rs::conversation_text_for_name` 统一提取用户/助手纯文本及多模态 text 块、跳过工具输出、保留最近 1000 UTF-16 单元以内的有效 Unicode 文本，并接入真实聊天 `/rename` → 模型请求 → 持久化标题链。单元测试覆盖工具结果隔离、多模态、长对话最近上下文及 Unicode；真实 CLI→模拟 HTTP 模型端到端测试验证最新多模态文本送达、工具内容不泄露、会话消息不额外增加。仍缺原版 `isMeta` / 非人工 `origin` 元数据过滤（当前 Rust Message 无对应字段）、异步取消与原版独立小模型路由/结构化输出协议，继续记为**部分实现**，不提升最终等价验收数。CI 以最新 PR HEAD 的实际结果为准。


## 2026-10-29：会话搜索源码对照补全（保持部分实现）

对照 2.1.88 `src/utils/agenticSessionSearch.ts` 的 `logContainsQuery`、`extractMessageText`、`extractTranscript` 与会话排序规则，修复 Rust `src/session_control.rs` 只能按标题、标签、UUID 查找，无法通过摘要和真实对话内容找到历史会话的问题。现有 `sessions QUERY` 和恢复选择器复用搜索入口：标签精确匹配优先于部分标签、标题及摘要；读取首尾各 50 条消息，提取用户/助手文本及多模态文本块，跳过工具结果，按原版 UTF-16 单元限制搜索文本为前 2000 单元，截断时追加省略号（保留 Rust 合法 Unicode 边界）。增加真实持久化会话、标签优先级、摘要、多模态文本、工具输出隔离、短会话去重及长会话边界回归。

仍缺原版小模型语义关联搜索、Git 分支字段、轻量会话异步全文补载及中断/错误回退；该源路径由未迁移转为**部分实现**，不计为完整等价验收。Rust 代码提交 `616395c`、`56d660a`；最终 CI 以当前 HEAD 实际结果为准。

## 2026-10-29：未迁移 Git 配置与忽略规则代码落地

原版对照范围：`src/utils/git/gitConfigParser.ts` 全部对外函数、节/大小写/转义/注释处理，`src/utils/git/gitignore.ts` Git 原生规则检测与全局忽略写入。新增独立原生 Rust `src/git_config_parser.rs`、`src/git_ignore.rs`，接入真实命令入口 `git-config` 和 `git-ignore`（写入要求明确 `--add-global`），仓库识别在 Git 命令失效时读取本地配置中的 origin。测试包括实际磁盘配置、Git 临时仓库、嵌套忽略、缺失文件与幂等追加。源码状态保守从“未迁移”转“部分实现”，Git worktree 文件布局、设置自动忽略和插件动态技能调用仍未完整验收，不提升已验收数。



## 2026-10-29：/clear 后输入历史会话归属修复

在此前的 SessionEnd→新 UUID→SessionStart 实现基础上，补齐 `src/main.rs` 聊天循环的输入历史重新绑定：原 `PromptHistory` 长期保留清空前的会话 ID，导致清空后输入仍记到旧会话。现以新会话 UUID 重建 `PromptHistory`，后续输入独立归属新会话，并向用户明确显示可恢复的新 ID。真实 CLI 子进程测试输入 `/clear`、`/session`、`/quit`，验证原/新历史各自 UUID 和两个会话文件持续存在。此项仍需原版缓存和异步任务全语义补齐，不升级源文件已验收数。



## 2026-10-29：/clear 会话持久化与 Hook 生命周期源级修复

对照原始 2.1.88 `src/commands/clear/clear.ts`、`conversation.ts`、`caches.ts`：Rust 原先清空并覆盖同一 UUID 的旧会话，破坏恢复入口。现由 `src/agent.rs`、`src/main.rs` 改为 SessionEnd Hook → 保存旧会话 → 新 UUID/新任务图/空消息并保留权限模式 → SessionStart Hook；聊天与 SDK 清除入口共享。保留后台 Shell 管理器，不覆盖旧历史；原源码原厂遥测不保留。单测覆盖旧记录可恢复、新记录为空、模式延续、任务图隔离、两个生命周期 Hook 顺序。缓存全量清理、MCP 连接热重置、前台/后台队员差异及原版 UI 仍待补齐，不计源路径完整验收。



## 2026-10-29：/files 读写编辑缓存可见性与最近使用顺序（未等价验收）

直接对照 2.1.88 原版 `src/commands/files/files.ts:11–18`、`src/utils/fileStateCache.ts:17–22,31–49,78–80`、`src/tools/FileWriteTool/FileWriteTool.ts:329–337`、`src/tools/FileEditTool/FileEditTool.ts:517–525`：原版 `/files` 展示文件状态缓存键；成功写入与编辑也更新该缓存，最近使用优先，默认 100 项。Rust 的 `src/files_context.rs` 此前仅记录成功读取并按字母排序，遗漏实际写入与编辑。现从会话持久化的工具调用及结果重建成功读/写/编辑/NotebookEdit 的文件键，最近使用优先、去重、100 项上限，失败不入缓存；文件在会话恢复前删除时仍可显示先前成功操作的记录。增加成功写入/编辑、失败隔离、删除后恢复和超过 100 项淘汰的回归测试，保留现有真实 CLI HTTP 文件读取及恢复测试。

**仍未等价**：原版独立缓存的 25MB 内容上限、时间戳、读范围、部分视图标记、文件编辑冲突保护、压缩/清空后的缓存生命周期及全部并发边界。此项保持“部分实现”，不升级任何已验收文件；最终 CI 以本轮 HEAD 的实际 Actions 结果为准。


## 2026-10-29：原版命令 Hook 非零退出与 JSON 优先级修复（冻结项未验收）

原版证据：用户提供 2.1.88 源包 `src/utils/hooks.ts` 2499–2533、2543–2613、2647–2695 行。原版先解析有效 JSON，即使命令退出非零也应用 JSON 决策；没有有效 JSON 时，只有退出码 2 阻止工具，其他非零退出作为可见的非阻断错误；语法错误的 JSON 按普通输出的退出码处理，格式合法但 Schema 不合法的情况仍待补齐完整校验。原 Rust 在解析 JSON 前先拒绝所有非零退出，导致 PreToolUse 的普通 Hook 失败误阻断真实工具、退出码 2 的有效 JSON 决策被忽略。

现修改 `src/hooks.rs` 的命令 Hook 状态处理，记录非阻断错误并继续后续 Hook；`src/agent.rs` 向终端和结构化 SDK 输出 `hook_warning`，不伪造工具失败。回归包括真实 Hook 子进程、后续 Hook 改写输入后实际 Shell 创建文件、退出 2 有效 JSON 决策、无效 JSON 和明确退出 2 阻断。尚缺原版全部 Hook 事件的完整调度、并发、取消、HTTP Hook 和反馈 UI；冻结 `src/utils/hooks.ts` 继续未验收，362 项不勾选。CI 结果以本提交实际运行结果为准。


## 2026-10-29：权限规则来源接入 Agent 执行链

已对照原版 `permissionsLoader.ts`、`settings/constants.ts`、`settings/settings.ts`、`permissionRuleParser.ts`，在 `src/permission_settings.rs` 实现用户/项目/本地/配置/托管规则加载、托管策略独占、规范化规则持久化增删、跨进程锁和原子写入，并从 `src/main.rs` 接入 Agent 工具权限执行。新增真实 Shell 文件副作用、16 线程并发、损坏 JSON、符号链接越界和来源优先级回归。未实现原版全部设置来源/热更新/审批 UI，**冻结 362 项不勾选，已验收数不变**。CI 以最新 HEAD 为准。


## 2026-10-29：PowerShell 独立模型工具名接通真实执行器

对照原版 `PowerShellTool.tsx`、`toolName.ts`、`powershellPermissions.ts`，补齐此前原版 `PowerShell` 工具名不在模型工具清单且执行器返回 Unknown tool 的主链缺口。PowerShell 会话现可发现原版工具定义，显式工具调用使用 PowerShell 进程、PowerShell 规则、毫秒超时及前后台任务生命周期；后台进程逐次指定 shell，不更改共享默认值，避免并发任务互相污染。测试覆盖真实工具入口、缺少 PowerShell 安装时不回退 Bash、权限拒绝及后台 Shell 隔离。代码涉及 `src/agent.rs`、`src/tools.rs`、`src/background.rs`。

**未完成完整等价**：原版 PowerShell AST、沙箱、命令语义、全部输出格式／图片、取消／重启／后台通知和审批 UI 尚需对照，冻结 362 项全部保持未验收；迁移分类和验收计数不增加。


## 2026-10-29：PowerShell 复合脚本权限绕过修复（Issue #3 持续开放）

对照原版 `PowerShellTool/powershellPermissions.ts` 的子命令提取、规则优先级与 `utils/powershell/parser.ts`，修复 Rust PowerShell 规则只检查整条脚本、允许 `Get-ChildItem:*` 可能连带放行后续 `Remove-Item` 的问题。Rust 现在分别评估分号／管道／换行／逻辑连接符后的命令，拒绝优先；脚本块、插值执行、重定向和反引号等无法安全拆解的语法需单次人工审批。新增规则单测及 `ToolExecutor::approve_external` 实际授权入口回归，覆盖 Bash/PowerShell 隔离。代码提交 `e1131d8`、`daffb48`。

**状态：原冻结 362 条均保持未验收。** 仍缺 PowerShell 完整 AST、路径安全和权限审批 UI 等原版行为；未调整 `FILE_PORT_MATRIX.md` 的文件分类，也不增加任何已验收数。

**目标：1906 个原始文件的功能逐项重写、等价验证。**

- 明确归入环境/行为遥测删除项：22 个原始文件路径（仅是第一轮整文件分类；其他混合文件也需排除隐式上报）。
- 现有 Rust 模块覆盖部分行为、但未完成逐文件等价：1058 个原始文件路径。
- 当前台账仍标注“未迁移”的文件路径：797 个（与部分实现项互斥，按当前状态字段分类）。
- **达到原始文件纯函数行为/边界等价且有验证证据的路径：29 个（包括独立纯函数、状态模块与固定常量文件；完整调用链仍需逐项验证）；其上下游业务逻辑仍需完整验证。** 其他当前已实现的功能仅在特定测试场景验证，不能把“部分实现”升级为“完成”。
- 非排除项的原始文件级等价验收仍未闭环：1855 个。

## 本轮已进入 Rust 实现的领域

Notebook 编辑、Glob/Grep、LSP、WebFetch、WebSearch、文件检查点与回退、SDK 标准输入输出、本地插件组合、并发 Agent 团队、SSE 模型流式输出与工具调用分片重组、后台 Shell 任务生命周期、项目记忆索引与模型相关性筛选。

上述能力的部分行为有真实模拟服务或文件系统测试。按功能有测试，不等于原始各路径语义完全一致。

## 继续推进必须满足的门槛

1. 每个原始文件逐函数核对输入、输出、异常、同步/异步时序、副作用、权限和持久化行为。
2. 原版不同工具间的关联行为也要对照，而不是只复刻单个函数。
3. 只有代码实现加对照测试和实际流程验证齐备，才把台账行改为“已验收”。
4. 原厂账号/API 绑定与环境行为上报继续保持排除，远程功能需由非原厂自建协议承载。
5. 最新 PR 保留草稿直到全量完成；不能凭通过 Cargo CI 将未实现的文件标完成。

完整原路径清单及逐项状态见 [FILE_PORT_MATRIX.md](FILE_PORT_MATRIX.md)。

## 2026-10-28：任务图与团队消息调用链复核

原始对照：`src/utils/tasks.ts`、`src/utils/teammateMailbox.ts`、`src/utils/swarm/teamHelpers.ts`、`src/tools/SendMessageTool/SendMessageTool.ts` 与相邻 Agent/Stop hook 调用路径。原厂源码未上传仓库。

Rust 实现：`src/task_events.rs` 订阅通知，`src/task_graph.rs` 无效记录降级与任务变更通知；`src/team_mailbox.rs` 持久化收件、跨线程加锁、未读与按条件原子确认；`src/team_registry.rs` 团队注册、持久化、并发更新锁、单发/广播、关停与计划回复；`src/agent.rs` 成员身份、共享邮箱根目录、自动收件、模型工具回填、退出后释放未完成任务。

测试入口：`team_mailbox::tests`、`team_registry::tests`、`team_registry::protocol_tests`、`agent::tests::team_message_tool_dispatch_and_peer_inbox_delivery`、`agent::tests::team_inbox_enforces_member_isolation_and_protocol_retention`、`tests/integration_cli.rs::model_driven_team_create_message_receive_and_cleanup`。是否通过以最新 HEAD 的 CI 实际结果为准。

剩余差异：团队配置全部字段与后端、持续运行成员的异步调度和消息唤醒、权限/沙箱申请响应、模式同步、计划审批状态机、终止/重启协商、跨进程收件通知、UI、工作树清理、远程会话路由仍未等价。原始 `src/utils/mailbox.ts` 的内存队列和 waiter 语义仍未迁移。**本轮新增 2 条部分实现，已验收数不变。**

### 后续同一主链补强

- `src/session.rs` 与 `src/agent.rs`：团队名称和成员身份写入会话文件，重启恢复后继续使用同一团队任务图；历史会话文件缺少新字段时仍可加载。会话写入改为独立临时文件、同步落盘、原子替换。
- `src/tools/TaskUpdateTool/TaskUpdateTool.ts` 对照：任务负责人实际变化后，向对应成员邮箱写入 `task_assignment` 消息；重复设置相同负责人不会重复通知。权限与模式控制消息保留给独立处理链，避免被普通聊天消费。
- `src/task_events.rs` / `src/task_graph.rs`：任务变更订阅在磁盘锁释放后派发，修复订阅者回调重新修改任务时的死锁；补充重入回归测试。
- `src/team_registry.rs` / `src/agent.rs`：成员退出后允许同名重新加入；成员真正结束工作树处理、任务收回和通知尝试后才标记非活跃，避免清理与写入竞态。

以上仍属于既有部分实现路径的深化，**不新增文件级验收数量**。未补齐的团队权限请求/响应、异步常驻成员、原版全部配置字段及终端 UI 继续保留为未等价。

### 邻接消息基础设施的保守登记

`src/in_memory_mailbox.rs` 与 `src/event_signal.rs` 新增内存消息队列、等待者匹配、取消保护、修订号和事件订阅；`useMailboxBridge` 的空闲消费行为有单元测试。对照原始 `src/utils/mailbox.ts`、`src/utils/signal.ts`、`src/hooks/useMailboxBridge.ts`，三条路径仅由未迁移改为部分实现：React 渲染生命周期、监听集合在派发中增删的语义和完整调用方尚未等价，**没有新增已验收项**。

## 内存邮箱与终端消息路径（2026-10-29）

依据 2.1.88 原始 `src/utils/mailbox.ts`、`src/utils/signal.ts`、`src/context/mailbox.tsx`、`src/hooks/useMailboxBridge.ts` 的具体实现，现加入独立 `event_signal.rs` 与 `in_memory_mailbox.rs`；实现消息过滤、等待者先于队列分配、立即登记等待、通知与 revision、空闲单消息消费和取消恢复。Agent 对每个会话持有实例；SDK 提供 `mailbox_send` / `mailbox_poll`，对应真实多消息端到端测试。尚未实现 React Provider / useSyncExternalStore / useEffect 帧时序及 JS Set 迭代时动态订阅语义，因此这四项继续计作**部分实现**，不算已验收。

事件信号 `src/event_signal.rs` 已进一步实现原版 JavaScript Set 的实时遍历：派发期间删除尚未轮到的监听会跳过，新增监听可在同轮执行，并补充回归测试。该路径仍保守标注部分实现，等待跨模块调用方核对。

### 任务认领与退出恢复的补充验证

对照原版 `TaskUpdateTool`：队员把无负责人任务设为 `in_progress` 时自动填入自己的成员名，并经持久化邮箱发送 `task_assignment`；重复设置相同负责人不重复发送。团队删除现在连同任务记录、高水位编号和锁文件清理；若后续任务目录清理失败，可在修复目录问题后重试，不会因团队配置已删除而永久卡住。会话文件的读写路径已校验工作区边界，拒绝通过符号链接转向外部目录。相关测试见 `agent::tests`、`task_graph::tests` 和 `session::tests`，仍未升级原文件验收状态。

## 2026-10-28：团队关闭握手与失败清理顺序

沿既有团队主链继续补全 `src/tools/SendMessageTool/SendMessageTool.ts`、`src/utils/teammateMailbox.ts`、`src/utils/swarm/teamHelpers.ts` 相邻调用行为。Rust `src/team_registry.rs` 在持久邮箱中核对关闭请求的接收者、请求编号和发起者，拒绝伪造、跨成员冒用及重复响应；同一毫秒的请求编号加入唯一标识；向离线成员直接发送消息时返回错误，广播只面向活跃成员。跨 `Teams` 实例的磁盘消息与注册表锁共同验证重放防护。

`src/agent.rs` 将队员退出顺序固定为：收回未完成任务 → 向队长邮箱写入退出通知 → 最后将成员标记为非活跃。前两步失败时保留活跃状态，阻止团队删除掩盖未完成清理，修复存储故障后可重试。新增模拟失效锁、重试恢复、伪造响应、重复确认和 Agent 工具调用链测试。

本次仍只深化既有部分实现路径，**已验收文件数不变**。尚未完成团队后台常驻调度、权限和沙箱请求响应、计划审批的完整状态机、跨进程实时通知、远程传输与原版全部异常时序。完整验收须对照原始 2.1.88 文件逐函数复核；CI 通过仅证明当前 Rust 测试通过。

## 权限规则链路复核（2026-10-29）

从原始 `src/utils/permissions/permissionRuleParser.ts`、`shellRuleMatching.ts` 和 `permissions.ts` 检查真实调用链。`src/permission_rules.rs` 实现规则解析、特殊字符转义、legacy 名称、精确/前缀/通配 Shell 规则及 deny/ask/allow；主 Agent、子 Agent 和团队成员均使用同一用户定义策略。真实模型→工具→Shell 的审批副作用集成测试覆盖非交互静态允许与全量授权下的明确拒绝。原版分类器、模式切换、规则来源和完整 1487 行权限引擎仍未逐项移植，**继续计为部分实现**，不因为测试通过标记原文件完整等价。

### Agent 委派权限链补强

`src/tools.rs` 增加无工作树委派也必须执行的显式权限规则检查；`src/agent.rs` 的 `spawn_agent`、`spawn_team` 进入子任务前统一调用。显式拒绝阻止启动，显式询问在非交互场景拒绝，子 Agent 仍继承原有只读/询问/完整权限级别；工作树隔离继续额外审批。新增双入口拒绝测试，修复原先仅隔离模式触发审批导致的规则绕过。权限分类与原始完整 Agent 工具语义仍标为部分实现。

## 2026-10-29：任务状态机与完成 Hook 的行为差异修正

直接对照原始 `src/tools/TaskUpdateTool/TaskUpdateTool.ts`：`TaskCompleted` Hook 拒绝完成时，原版返回成功调用通道中的普通工具反馈（`success:false`），不会升级成执行器异常导致同轮兄弟工具取消。Rust `Agent::run_task_graph` 现在保留未修改的状态/字段，并返回原版前缀消息。新测试 `blocked_completion_is_non_error_feedback_and_does_not_mutate_fields` 覆盖状态、字段、元数据未被提交。

原版设置新负责人后 `await writeToMailbox`，若投递失败会让工具调用报错，但之前的任务更新已落盘。Rust 先前只记录日志并返回成功，属于行为差异，现改为向上传播失败，增加模拟不安全收件箱的 `failed_task_assignment_delivery_surfaces_error_after_task_persistence` 回归测试。未更改原始路径验收数量：其他任务状态、竞争时序、收件端和跨模块生命周期仍待核验。

### SDK 控制请求的兼容与关联（2026-10-29）

参考原始 `src/utils/controlMessageCompat.ts`：顶层和 `response` 的 `requestId` 仅在 `request_id` 不存在时转换，后者已存在则保留全部字段，原地对象之外的数组/基础值不受影响。Rust `control_message_compat.rs` 实现并接入 SDK，`control_request` 与 `control` 两种包型共享处理，成功/失败的 `control_response` 回带关联 ID；该路径只标部分实现，`replBridge` 等后续协议仍缺。

### 2.1.88 SDK 原始控制协议包络（2026-10-29）

本次进一步对照原始 `src/entrypoints/sdk/controlSchemas.ts`：`control_request` 内层 `request.subtype` 与 `request_id`、`control_response.response.subtype` 成功/失败结构已接入实际 SDK stdin/stdout。多模型 `set_model`、权限上限校验后的 `set_permission_mode`、后台 Shell `stop_task` 调用实际 Agent；失败仍返回对应 `request_id`，不吞错误。新增独立 CLI 进程端到端测试，覆盖请求编号兼容、模型切换、权限越权拒绝及未知 subtype。其他 SDK 子类型、异步取消、初始化/远程 MCP/Hook 回调仍未完成，因此不升级文件级已验收状态。

## 2026-10-29：跨模块计价与模型工具输入

- 按用户配置的提供商、模型与费率记录多模型成本，区分输入、输出、缓存读取/写入和网页检索费用；会话恢复、SDK 查询和后台用量归集进入已有主链。两项成本源码因计时与回调差异保留部分实现。
- 对照原始 `src/utils/semanticBoolean.ts`、`src/utils/semanticNumber.ts` 新增 `src/semantic_inputs.rs`，同时改动 `src/tools.rs` 实际处理来自模型的工具 JSON 参数；`"false"` 不再被解释为真值，非法数字不再默默按默认参数处理；原始 Zod 复合 schema 尚未全量核对。
- `edit_file` 增加明确选择的 `replace_all`，未选择时仍只允许唯一匹配；错误参数在触碰文件前拒绝。
- `src/xml_escape.rs` 按原始 `src/utils/xml.ts` 完整实现 XML 文本转义和引号属性转义，纯函数测试覆盖特殊字符替换次序、Unicode、空内容和重复转义。
- 更新后的路径分类：排除 22、部分实现 353、未迁移 1,503、仅限源路径自身已验收 28。完整原版业务链路仍待逐文件核对。

### 2026-10-29：JSON BOM 与斜杠命令解析

`src/json_read.rs` 对照原始 `jsonRead.ts` 只删除一次开头 U+FEFF；真实配置及会话恢复从已有 JSON 读取函数接入，增加 Windows BOM 回归。`src/slash_command.rs` 对照原始斜杠命令词法、MCP 标签及空格保留语义，并接入 CLI `/model`。两条纯函数源路径自身验收，MCP 斜杠工具的后续路由仍需迁移。台账合计：22 排除、353 部分实现、1,501 未迁移、30 独立已验收。

### 2026-10-29：费用时长和文件编辑增删行

沿原始 `src/cost-tracker.ts` 继续补充 `CostLedger`：模型单价格式、Token 原始数量价格计算、API 请求耗时、工具耗时、交互墙钟耗时和新增/删除代码行数；会话序列化和 `/cost` 统一输出。`src/tool_metrics.rs` 的行级 diff 接入 `write_file` 和 `edit_file` 实际写文件路径；拒绝操作不记改动，新文件尾随换行的统计遵循原版 `split(/\\r?\\n/).length`。增加模拟多轮 HTTP 工具调用的回归测试。未覆盖所有原始补丁格式、异常调用计时与重试时长，两个源路径保持“部分实现”。总分类更新：排除 22、部分实现 354、未迁移 1,500、独立已验收 30。

### 2026-10-29：子代理与团队费用归集

`src/cost.rs` 的 `CostLedger::merge` 保留每个 provider/model 独立统计，累计 Token、已确认价格、未知价格标记及计时、行修改数量。`src/agent.rs` 的真实 `spawn_agent` 和 `spawn_team` 返回后将各子会话账本并入父会话；`src/tool_metrics.rs` 同时累加子代理真实文件改动计数，避免后续工具覆盖父会话行数。端到端测试验证子代理调用前后的父会话 Token 与费用汇总，team 原先的任务退出/隔离语义维持。异常和失败工具的全部计量时序仍在部分实现项中。

### 2026-10-29：修复缓存计费服务口径差异

原厂专属计价移除后，不能把“缓存读取包含在输入总量”硬编码到所有模型。新增按模型费率配置 `cache_read_included_in_input`（默认 true）和 `cache_write_included_in_input`（默认 false），在计费时只扣除已包含在输入 Token 中的缓存类别。普通 HTTP 响应与 SSE 统一读取缓存创建、缓存读取和服务器检索请求计数；Gemini 兼容扩展字段按同类语义解析。单元测试覆盖包含/独立两种服务口径、老配置默认值、SSE 帧用量解析。这是源码 `modelCost.ts` 的进一步通用适配，不宣称剩余调用链已完整迁移。

### 2026-10-29：防止迁移数量和实际代码脱节

`tests/migration_matrix.rs` 不再把每次变动的部分实现/未迁移/独立验收数写死在断言里，而是逐行检查 1,906 条唯一原路径，保留 22 条明确排除项，强制所有状态恰好归入唯一分类。对于标记“已验收”的路径，必须记录证据且所指 Rust 目标文件真实存在。同时将动态汇总与本页最新部分实现、未迁移、已验收、剩余未闭环数量交叉验证；若分类或本文数字不同步即阻止 CI。已修正剩余未闭环数为 1,854，未扩展“已验收”的语义范围。

### 2026-10-29：继续处理尚未迁移的集合和分组原路径

对照原始 `src/utils/set.ts` 的全部四个导出函数和 `src/utils/objectGroupBy.ts` 的全部分组逻辑，新增 `src/collections.rs`。采用有序集合/有序映射以保留插入顺序，并在函数测试中覆盖空集、子集、差集、交集、并集、重复键、回调从零开始计数与 `__proto__` 键安全性。因 JavaScript 的 SameValueZero、对象引用身份、Symbol 键及数字属性枚举次序与 Rust 泛型 `Eq+Hash` 存在边界差异，先均标为部分实现，不把 Rust 泛型通过测试冒充原路径全部等价。台账调整为排除 22、部分实现 356、未迁移 1498、已验收 30，仍有 1854 条非排除原路径未完成全量等价验收。

### 2026-10-29：失败团队任务仍产生费用与源格式显示

修复并行团队回收异常：若成员模型请求已经计费，但结束时因输出超限失败，父会话仍须累计其 Token、费用及文件改动，不能把失败归类为空成本；成员清理和状态保持失败状态。新增真实 HTTP 三轮团队成员失败测试，对照父/子会话计数、价格及错误反馈。`src/cost.rs` 的金额展示继续对照原始 `cost-tracker.ts` 的 `formatCost`：不超过 0.5 美元时默认四位小数，超过 0.5 美元按分取整并展示两位小数，保留未知模型价格警告。

### 2026-10-29：队列优先级与查询生命周期主链

逐函数审阅原始 `src/utils/messageQueueManager.ts` 和 `src/utils/QueryGuard.ts`。新增 `src/message_queue.rs`、`src/query_guard.rs` 与 SDK 桥接 `src/sdk_queue.rs`，在真实 stdin/stdout SDK 上提供优先级排队、最高优先级查看/移除、原插入顺序整体出队、消息身份、按 Agent 过滤和输入恢复。`now > next > later`，同优先级先进先出；后台成员消息不会被主 Agent 错误消费；原始纯函数恢复可编辑文本时按 JS UTF-16 单位计算光标偏移并保留粘贴图片 ID。SDK 排队执行使用 QueryGuard 预约、启动、结束保护异步空档，真实模拟模型 HTTP 端到端断言调用次序和剩余消息。未实现的图像直接传输与非 prompt 通道会保留在队列并明确报告原因，拒绝悄悄丢消息。原始 UI 订阅、桥接可见规则和操作日志仍待全量对照。分类更新为排除 22、部分实现 357、未迁移 1496、源路径自身已验收 31；总未闭环 1853。

### 2026-10-29：技能替换与安全 Shell 词法进入真实执行链

依据原始 `src/utils/argumentSubstitution.ts` 和 `src/utils/bash/shellQuote.ts` 新增 `src/skill_arguments.rs`、`src/shell_quote.rs`：保留引号组合、Shell 单引号字面语义、转义、操作符排除、禁止变量实际展开；对命名位置参数、`$ARGUMENTS[n]`、`$n`、`$ARGUMENTS` 依原版次序替换，支持 frontmatter 参数名单/列表及剩余参数提示。实际 `read_skill` 工具无论是工作区技能还是明确启用的插件技能，收到 arguments 都执行替换；未传参保留原文，错误类型直接拒绝。新增模拟模型发出 read_skill 工具调用、读取替换结果继续下一轮对话的 HTTP 集成测试。Shell 安全检测覆盖异常引号/不平衡片段及原版单引号转义差异风险。shell-quote 第三方解析库的全部边界、Glob/复杂注释、前端键盘提示/UI 尚未完成，不按已验收记。分类：22 排除、359 部分、1,494 未迁移、31 源路径独立已验收；仍有 1,853 条最终等价未闭环。

### 2026-10-29：Shell 安全审查接入实际执行前门禁

原始 `tools/BashTool/bashSecurity.ts` 对 `hasShellQuoteSingleQuoteBug` 返回人工审批（而非自动批准），并要求非打印控制字符单独确认。Rust `shell_quote::security_review_reason` 新增对应风险判断，实际接入 `ToolExecutor::require_approval` 的 `run_command`/Bash 执行前门禁。即使配置 `--approval full`，非交互场景也必须拒绝存在引号歧义的高风险命令；交互场景需要单独人工确认。新增真实 `ToolExecutor::execute` 测试，构造“先触碰标记文件、后跟单引号反斜杠解析歧义”的命令，验证被拒绝时标记文件未产生，并测试未闭合引号、正常命令。其他原始 Bash 安全校验超过此范围，继续保持部分实现，不表示已经完成完整命令安全审核。

### 2026-10-28：SDK 队列错误边界与 QueryGuard 验收证据复审

逐函数复核原始 2.1.88 的 `src/utils/messageQueueManager.ts`、`src/utils/QueryGuard.ts`，并检查 `src/sdk_queue.rs`、`src/main.rs` 的实际 SDK 消费路径。修复非法优先级排队消息此前通过 `?` 中断整个 SDK 进程的问题，现在向调用方返回错误并继续处理后续消息。修复 `queue_drain` 先执行成功的模型请求、后遇到 Bash/权限/图片/斜杠命令时整个响应报错并丢失已完成结果的问题：现在返回已有结果和 `blocked` 队列身份/原因，保留尚未执行的消息；首条不支持时仍拒绝执行。斜杠命令不会被误发给模型。新增真实 SDK 子进程与模拟模型 HTTP 测试 `sdk_queue_drain_preserves_results_before_blocked_item`、`sdk_invalid_queue_input_and_slash_routing_keep_session_alive`，CI 以最新提交结果为准。

重新检查原版 QueryGuard 的 `createSignal` 订阅及 React 生命周期，Rust 当前只有同步状态转换和基础订阅的局部证据，缺少动态订阅、UI 和跨模块取消时序的等价证明，因此将该路径由“已验收”降为“部分实现”。当前台账：排除 22、部分实现 360、未迁移 1494、源路径自身已验收 30；剩余 1854 条非排除源路径未闭环。

### 2026-10-29：Headless 合并请求与消息幂等计价

对照原始 `src/cli/print.ts` 的 `canBatchWith`、`joinPromptValues` 和队列 `drainCommandQueue` 路径，新建 `src/queued_prompt_batch.rs` 并接入 `src/sdk_queue.rs`。普通 prompt 仅当 workload 与 isMeta 相同、顺序相邻且允许处理时合并成单个 ModelRequest；字符串以换行合并，结构化块数组按原始顺序拼接，跨 Agent 消息和高危命令不错误混入。每条排队消息仍保留各自 identity、响应结果；同批模型费用/Token 只累计一次。另修复 `src/message_queue.rs` 的同步订阅、频道可见性和动态嵌入图片 ID、数字图片粘贴键枚举次序。增加一轮 HTTP 处理多条提示的真实进程测试。原始无界面模式 5,595 行中仍有大量 SDK、远程 IO、Hooks、取消与任务通知功能未移植，所以新增源路径只计部分实现。更新分类 22 排除、361 部分、1493 未迁移、30 源路径独立已验收；1854 条未完成整体等价。

### 2026-10-29：任务通知不再丢失结构化状态

对照原始 `cli/print.ts` 的 `drainCommandQueue` 中 `task-notification` 专用分支，SDK 队列在转发通知文本给模型前，先向 stdout 发出 `system / task_notification` 独立事件；包含任务 ID、工具调用 ID、停止/失败/完成状态（`killed` 映射 `stopped`）、输出文件、摘要和可选 usage（Token、工具次数、耗时）。不含 `<status>` 的进度流消息不再误判为完成。真实 SDK 进程+模拟模型 HTTP 测试验证事件出现在模型结果之前，内容结构正确并保留后续模型继续分析。原版其他事件与远端队列协议仍待移植，源文件保留部分实现。

### 2026-10-29：SDK 队列模型错误停止与结构化内容完整性

继续对照原始 `src/cli/print.ts` 的 `drainCommandQueue` 异常传播、`joinPromptValues` 与 `src/utils/messageQueueManager.ts` 消费时序。`src/sdk_queue.rs` 在模型调用失败时保留该批每条消息的错误结果、返回 `halted.reason=model-error` 和受影响身份，立即停止后续排队请求；后续消息保持在队列中，避免失败后继续向模型发送下一批。SDK 文本适配器对每个结构化块逐一检查，非文本块在出队前拒绝并保留原消息，不再静默丢弃 document/tool-result 等数据；合法文本块按顺序合并进入模型。

新增真实 SDK 子进程与本地模拟 HTTP 服务测试：`sdk_failed_model_turn_stops_drain_without_consuming_following_work`、`sdk_unsupported_content_blocks_are_preserved_until_explicitly_removed`、`sdk_direct_structured_text_is_delivered_and_nontext_is_rejected_without_http`。SDK `user.content` 和 `user.message.content` 现接受文本块数组，直接提问或排队均经过相同的完整性检查；非文本块在模型请求前明确拒绝，排队场景保留原消息。SDK 队列改动自身不新增文件级验收；其后的压缩状态复审单独降级 1 项，最新分类为排除 22、部分实现 362、未迁移 1493、独立已验收 29，仍有 1855 个非排除路径未完成完整等价验收。多模态原生请求、跨进程队列取消、异步回放及 UI 调用链仍待补全；CI 结果以对应 HEAD 的实际工作流为准。

### 2026-10-29：已验收路径的订阅行为复审

复核原始 `src/services/compact/compactWarningState.ts` 与其依赖 `src/state/store.ts`，发现 `createStore` 通过 `Object.is` 忽略相同布尔值的重复赋值，且通过同步 JavaScript Set 向监听器通知；Rust `watch::send_replace` 之前会对相同值重复发送变更。现使用 `send_if_modified` 修复重复通知并加入测试。但 Rust watch 接收端的合并通知、同步回调和动态订阅仍未与原版逐项验证，因此将该源路径从“已验收”降为“部分实现”。更新 1,906 路径台账与汇总，验收数由 30 调整为 29，不以局部测试冒充全链等价。

### 2026-10-29：SDK 消息 UUID、原始内容回放与合并批次确认

对照 2.1.88 原始 `src/cli/print.ts` 的 `drainCommandQueue`、`batchUuids`、`replayUserMessages` 和 `src/utils/messageQueueManager.ts` 的队列身份语义。修复 Rust SDK 入队 `user` 事件丢弃 `uuid`、`origin`、`pastedContents`、`preExpansionValue` 和 `bridgeOrigin` 的问题；合并同 workload/isMeta 提示时保留每条原始消息及身份。启用 `replay_user_messages`（兼容 `replayUserMessages`）后，在模型请求前按实际出队次序为每条有 UUID 的消息输出原始内容的 `user/isReplay` 事件，避免合并请求只确认最后一条；直接 SDK 提问也使用同一回放格式。保留原始内容块数组，不将回放替换为文本扁平化结果。

真实进程与模拟模型 HTTP 回归：`sdk_queued_uuid_replays_survive_prompt_batching_and_preserve_source_content`、`sdk_direct_structured_text_is_delivered_and_nontext_is_rejected_without_http`，检查错误 UUID 输入不终止 SDK、批次 HTTP 调用次数、每条 UUID、原始结构化内容、来源信息和回放先于结果的时序。**当前仅补充现有部分实现的 SDK/Headless 调用链，1,906 条分类仍为 22 排除、362 部分实现、1,493 未迁移、29 源路径独立已验收；1,855 条未完成全量等价验收。** 当前仍缺 QueryEngine 原生消息确认回调、失败重试全时序、远端传输、全量权限/Hook 交互和非文本内容模型传输。最新 CI 以对应 HEAD 工作流为准。


### 2026-10-29：原版 SDK 异步取消与 MCP 真实状态调用链

对照原始包 `src/cli/print.ts` 的 `cancel_async_message`（按 UUID `dequeueAllMatching`）、`mcp_status`（读取当前连接池）及 `src/entrypoints/sdk/controlSchemas.ts`；在 Rust SDK 正式 `control_request/request.subtype` 包络接入：

- `cancel_async_message` 从共享优先级队列按 UUID 原插入顺序移除全部未执行消息；再次取消返回 `cancelled:false`；错误 UUID 按原始请求编号返回错误。已出队的模型请求不追溯取消。
- `mcp_status` 从实际 stdio MCP 初始化、tools/list 发现结果和本地子进程存活状态返回名称、工具、版本、退出失败状态；不对 SDK 暴露服务器环境变量。没有配置 MCP 时返回空数组。
- 无额外 SDK 自定义配置的 `initialize` 返回本地现有运行时模型列表和标准控制响应，不伪造原厂账户。对于尚未实现的 SDK hooks、SDK MCP、系统提示词覆盖等初始化请求明确报错，避免虚报配置生效。
- Rust 单元测试覆盖 MCP 实际服务启动、工具发现、进程退出后状态变化。CLI 子进程集成测试验证重复 UUID 取消、请求编号、队列持久身份、仅存活提示实际模型 HTTP 请求与回放顺序、MCP 两次状态查询和机密环境隔离。

这些是原有部分实现的主链补强，**不增加任何已验收文件数**。仍缺原版同时运行时 interrupt 取消、Hook callback/权限询问桥、SDK MCP 动态变更、远端传输和完整初始化控制。台账维持 **22 排除、362 部分、1,493 未迁移、29 已验收，未闭环 1,855**。测试通过状态须以本次提交 GitHub Actions CI 为准。


### 2026-10-29：Agent 多工具错误隔离与后续模型请求恢复

对照 2.1.88 原始 `src/services/tools/toolExecution.ts::runToolUse` 的异常转为 `tool_result(is_error)`、`StreamingToolExecutor.ts` 非 Bash 工具错误不取消无关同轮工具，以及 `src/query.ts` 在下一轮前收齐工具回填：修复 `src/agent.rs::ask` 在模型输出非法工具参数 JSON 时调用 PreToolUse 钩子直接终止整个 Agent 的缺陷。现在在实际执行前验证 JSON 类型，非法语法和非对象参数变成对应 tool ID 的错误结果，不触发工具、文件或权限副作用；同轮其余合法工具继续执行并持久化，模型获得完整工具结果再继续下一轮。

新 CLI 模拟模型 HTTP + 文件系统集成测试 `agent_invalid_tool_json_preserves_sibling_calls_and_model_followup` 核查一轮三个工具（非法 JSON、数组参数、合法写文件）以及第二轮模型真实接收对应全部三个工具 ID 和结果、合法文件落盘及会话保存。此变更只补充原有 `src/query.ts` 部分实现，不提升文件级验收。尚未实现安全并行工具批次、流式工具执行、用户中断与兄弟 Bash 子进程取消；原始 `StreamingToolExecutor` 仍未迁移，保留原有分类。**台账不变：22 排除、362 部分、1,493 未迁移、29 已验收，尚缺 1,855 条完整等价。**


### 2026-10-29：Agent Hook 命令失败不再破坏多工具调用完整性

继续对照原始 2.1.88 `src/services/tools/toolExecution.ts::runToolUse` 的异常转 `tool_result(is_error)` 和 `src/query.ts` 的工具结果汇总：此前 Rust 在 `PreToolUse` 本地命令异常退出或 `PostToolUse` 退出失败时用 `?` 终止整个 Agent 请求，漏回填其他工具。现在 PreToolUse 命令异常作为当前工具失败反馈，不执行被阻止的工具；PostToolUse 命令异常也作为工具失败反馈，同时保留实际已发生的工具输出和文件副作用，避免模型错误重试；其余同批工具及下一轮模型请求继续。成功/明确阻止 Hook 的既有处理不变。

新增真正模型 HTTP、实际 Hook 子进程、工作区文件系统集成测试 `agent_pre_and_post_hook_execution_failures_preserve_sibling_results`：PreToolUse exit 7 不泄露未执行读取内容，PostToolUse exit 9 对已执行写入给出明确回填，第三个合法工具继续，下一轮模型收到全部三个 ID 和错误内容。此测试与修复仍归于 `src/query.ts` 原有部分实现，保持全部路径分类不变。其他 Hook 事件、并发时序、中断、权限询问等仍未等价；CI 状态以本提交实测为准。

### 2026-10-29：工具 Hook 生命周期与实际输入协议补齐

沿已有 Agent 多工具主链，修复 `src/agent.rs` 中工具被 PreToolUse 拦截、Hook 启动失败或模型参数非法时，仍错误执行 PostToolUseFailure 的问题：这些工具没有实际执行，不得产生“执行后”事件。真实执行失败的工具仍触发 PostToolUseFailure，已执行成功的工具仍触发 PostToolUse。修复 `src/hooks.rs::after_tool` 把工具输入以 JSON 字符串发送给 Hook 的问题，现传递结构化对象，包含 PreToolUse 修改后的实际参数，保持工具调用 ID 不变。

新增 `tests/integration_hook_lifecycle.rs`，使用独立 CLI、模拟模型 HTTP、真实本地 Hook 子进程和文件系统同时验证：阻止读取不触发事后 Hook、不泄露敏感内容；修改后的参数实际写文件且 PostToolUse 收到对象；真实工具失败触发 PostToolUseFailure；同轮合法兄弟工具继续，全部结果进入下一轮模型请求。此项将原 `src/services/tools/toolExecution.ts` 从未迁移改为部分实现，**没有新增任何已验收路径**。分类：22 排除、363 部分实现、1492 未迁移、29 已验收，仍有 1855 条非排除原路径未完成等价验收。原版完整并行工具执行、Hook 竞态、中断、工具调用取消仍待核验。

同一调用链的 CI 复核发现新增真实子进程测试虽然通过，但运行耗时恰好约 60 秒：两个使用 `cat` 读取标准输入的 Hook 各等到默认 30 秒超时，暴露了 `run_command` 中写入端 `ChildStdin` 未及时释放的问题。已将 stdin 所有权移入写入 Future 并在写入后显式关闭；集成测试将 Hook 超时缩短至 3 秒，并断言工具结果中绝不包含事后 Hook 超时/失败。后续以新 HEAD 的 CI 实测耗时和测试结果确认修复。

### 原始 2.1.88 Hook 协议逐行核验（2026-10-29）

已从资料库取回 `claude-code-2.1.88-main.zip`（SHA-256：`23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`），逐段对照 `src/services/tools/toolExecution.ts` 第 800–861、995–1050、1483–1493、1696–1713 行和 `src/utils/hooks.ts` 第 3418–3424、3460–3467、3509–3517 行。原版阻止执行时不触发执行后 Hook；成功 Hook 标准输入为顶层 `tool_input` 对象和 `tool_response`，失败 Hook 为顶层 `tool_input`、`error`、`is_interrupt`，均含 `tool_use_id`。Rust 已修正此前的额外 `input` 包装和错误事件字段，独立 CLI/HTTP/Hook 子进程集成测试同时核验前置、成功、失败三种真实标准输入结构。另复审 `src/utils/commandLifecycle.ts` 原始全部导出，未发现当前已验收登记与其单监听器行为冲突；其他已验收行仍须持续核查。未实现原版全部 Hook 事件、异步取消和并行工具时序，保持部分实现。

### 2026-10-29：通用 MCP Streamable HTTP（统一任务 #3）

对照 2.1.88 源码 `src/services/mcp/client.ts` 的 HTTP 2025-03-26 连接、请求超时、JSON/SSE 返回、session ID 和 `headersHelper`/`types.ts` 配置，新建 `src/mcp_http.rs` 并在真实 `McpManager::connect`、工具发现/调用、SDK 状态查询中接入 `type=http`。支持 POST JSON-RPC、SSE 分片消息、会话 Header、协议版本、单请求 60s 超时、安全禁止带账号密码的 URL 与覆盖协议控制 Header、拒绝跨站重定向，不在状态 API 泄露 HTTP Header 密钥。新增本地 TCP 模拟 MCP 服务器全握手、tool discovery、SSE、session header 和工具调用测试；验证是否成功以最新 CI 结果为准。原厂 OAuth/身份登录被要求排除，其厂商无关身份验证机制、SSE 旧式传输、WS、重连、资源/提示、列表更新、OAuth 通用流程仍需要逐源码实现，四条原路径一律保留部分实现。不得将 HTTP 部分验收说成 3,349 行原始客户端已等价。

#### MCP 多服务容错与完整工具结果

原始源码 `services/mcp/client.ts` 的连接失败返回状态而不崩溃其他服务：
- `McpManager::connect` 现在逐服务捕获启动、握手及 `tools/list` 发现失败；失败标记 `failed`，成功实例继续可调用。拒绝在分页失败中发布一半工具和路由；MCP status 按原服务器配置顺序对应健康或失败的连接，而不是与压缩后的成功客户端错位。
- `tools/call` 强制输入 JSON 对象，`isError` 转 `ERROR:`；包含结构化内容、图片或资源块时返回完整 JSON，而不是只拼文本丢数据；HTTP 状态将端点查询参数移除以免泄露密钥。
- 新增本地 stdio mock 的失败连接隔离、无效工具定义回滚、错误与结构化内容保留、非法输入回归。完整断开重连、动态工具发现通知、资源/提示/订阅和 OAuth 仍是本任务 #3 的未闭环范围。

#### MCP 运行时工具目录更新与 RPC ping

对照原始 MCP SDK 客户端工具目录变化通知，新版 Rust stdio 连接在等待工具 RPC 回复时识别 `notifications/tools/list_changed`、对 server-initiated `ping` 回 `{}`，不会误作普通工具结果。`McpManager::refresh_changed_tools` 逐连接重新发现和分页，发生错误时保留原有效工具表并标记稍后重试；只有全部页成功才原子替换该服务的工具定义/路由/状态。Agent 每个模型请求前刷新，下一轮模型获得最新工具 schema。新增连接后工具表变化的真实 stdio mock 回归。SSE/HTTP 长连接通知、子代理多连接刷新以及完整重连策略未实现，不升级源路径验收状态。

### 2026-10-29：统一工单 #3——MCP HTTP 流式响应与工具目录通知

对照原始 2.1.88 `src/services/mcp/client.ts` 的 StreamableHTTPClientTransport、工具目录变更通知及连接关闭/错误恢复调用链，修复 Rust HTTP MCP 把 SSE 响应完整读至 EOF 才返回的问题：长连接不关闭时，已到达的 tools/call 结果此前会被阻塞直到超时。现按 HTTP 分块增量解析完整 SSE 事件，跨 chunk/CRLF 边界保持事件结构；收到目标 JSON-RPC ID 即交付工具结果，不等待服务器断开；先于结果到达的 `notifications/tools/list_changed` 被记入实际 HTTP 客户端状态，并在下一轮模型请求前走已有的分页原子刷新机制，失败保持旧目录待重试。保持 2 MB 响应边界与原有单请求超时。

新增真实本地 TCP HTTP MCP 服务器测试，模拟 chunked SSE、分裂的事件边界、同一流的目录变更通知、工具响应和延迟 1.3 秒才关闭的连接；验证结果先于断连返回、后续 tools/list 重新发现并替换模型工具 schema。该补齐仍归 Issue #3 固定的 MCP 部分实现路径，**不提升任何文件为已验收**。长连接独立 GET 通知、服务器主动请求、HTTP 会话失效自动重连、旧 SSE 传输及完整 MCP 资源/提示协议仍未等价。CI 结果以实际 HEAD 验证为准。

### 2026-10-29：MCP 工具身份安全与多服务器动态刷新隔离

对照原版 `src/services/mcp/mcpStringUtils.ts::buildMcpToolName`、`normalization.ts` 与 `client.ts` 的工具列表刷新语义，将 Rust MCP 工具名从易重排的数字槽位改为 `mcp__规范化服务器名__规范化工具名`。此前动态刷新后同一数字槽可能指向不同真实工具，导致旧模型工具调用意外执行新工具；现旧名字被撤销、新工具必须使用新名字。连接与刷新阶段校验规范化名称冲突，拒绝重复路由，防止跨服务器覆盖。刷新某服务失败时保留其旧目录并标记重试，同时继续处理其他健康服务的变更，最后汇总错误；不再因首个失败阻塞全部 MCP 服务。

真实 stdio 子进程测试覆盖两个服务同时通知目录变化、首个服务发现失败但第二个仍成功、失败服务再次发现恢复、旧工具路由撤销及新工具身份不串用。现有真实 stdio/HTTP 工具调用测试同步校验原版命名约定。权限完整匹配、重复规范化别名在所有入口的行为、远程重连及全 MCP 能力仍需进一步对照，Issue #3 的 362 条未勾选，台账数量不变。

### 2026-10-29：统一 Issue #3，MCP 资源/提示/模板与 SDK 实际控制链

本轮继续对照原始 `services/mcp/client.ts` 的 `fetchResourcesForClient`、`fetchCommandsForClient`、`resources/read`、`prompts/get`。新增 MCP 资源、资源模板、提示目录的逐服务器分页读取，目录项保留全部服务器元数据与服务名且单服务器失败隔离；支持 URI 资源内容（保留图片/二进制 blob 的结构）和命名提示（限定字符串参数），直接连接 Agent 与 SDK 嵌套/平铺控制指令 `mcp_resources`、`mcp_prompts`、`mcp_resource_templates`、`mcp_read_resource`、`mcp_get_prompt`。加 20 页/千项上限、重复游标检测与超时。真实 stdio 服务通过 SDK 进程完成两页资源、模板、提示内容读写和非法参数检查。原版 MCP Prompt 到 slash command 的动态挂载、资源订阅、完整重连/认证与 UI 生命周期仍未实现，保持 4 个源路径部分实现。

### 2026-10-29：原始子代理生命周期 Hook 接入真实 Agent/团队执行路径（统一任务 #3）

对照原始 `utils/hooks.ts` 中 `executeSubagentStartHooks`（agent_id/type、按 agent_type matcher）、`SubagentStop`（最后回复/日志位置）及 `executeTeammateIdleHooks`（队员转为空闲）。Rust `HookEvent` 扩展常见原始 Hook 事件名称；非仅枚举：`run_subagent` 先执行 SubagentStart，阻止时不发起子模型请求；完成后即使模型失败仍尝试 SubagentStop；始终做 SessionEnd 收尾并汇总费用。`run_team` 同样在并发成员启动/结束/空闲之前调用对应生命周期 Hook，并让阻断状态体现在成员结果里而不是虚报成功。新增真实 HTTP 模拟模型驱动的父->子->父请求集成测试、SubagentStart 否决导致子模型不执行测试、事件参数与 matcher 测试。原始 Hook 串并联调度、停止恢复重试、全部事件触发点、HTTP/Agent/Prompt 类型与 UI 状态尚未迁移，不能升级验收。

## 2026-10-29：团队计划审批请求—队长回执—成员权限模式闭环补强

已从资料库实际提取 `claude-code-2.1.88-main(1).zip` 原始源码，直接核对
`src/utils/teammateMailbox.ts`（计划审批消息字段与分类，约 681–710、950–1095 行）、
`src/hooks/useInboxPoller.ts`（队长自动审批 599–660 行、成员验证队长来源与模式转换 156–195 行），
并核对 Rust 现有 `team_registry.rs`、`team_mailbox.rs`、`agent.rs` 上下游链路。

- `SendMessage` 团队工具现在可以将计划审批请求以源协议字段持久投递到队长邮箱；计划回复须有真实请求、正确成员身份及唯一请求编号，拒绝伪造、错投和重复回复。
- 队长进入下一模型轮次前扫描未读请求，按原版默认行为自动批准，并继承队长模式（队长处于 plan 时转为 default）。回执先落盘，再将原请求交给模型；重启后重复扫描不会重复发送。
- 成员在 plan 模式下仅接受来自 team-lead 且可与自身请求关联的批准，权限转换仍受 CLI 的最高授权约束。失败时回执不被标记已读，便于修复后重试；未验证/伪造回复不改变权限。
- 新增真实磁盘团队邮箱、重启复读、并发队长扫描、故障注入与伪造权限升级回归测试。测试及 GitHub CI 状态以对应最终 HEAD 为准。

**保守分类：** 新增 `src/hooks/useInboxPoller.ts` 一条部分实现；原冻结 Issue #3 的 362 条保持不变、全部仍未勾选。现为 **22 排除、364 部分、1491 未迁移、29 已验收，非排除未完整验收 1855**。

**未闭环：** 原版每秒 UI 轮询与 React 状态提交时序、计划编辑器与完整审批展示、常驻队员运行时、权限/沙箱询问分发、跨进程通知、远程连接、SDK 事件流及完整原始消息异常边界仍需逐函数补齐。现有代码和测试不构成该文件或 Issue #3 的完整验收。

### 同一团队权限控制链的继续补齐

继续核对原始 `src/utils/teammateMailbox.ts::createModeSetRequestMessage/isModeSetRequest`（1034–1062 行）
和 `src/hooks/useInboxPoller.ts`（549–597 行），新增真实 `mode_set_request` 消息投递。
仅 team-lead 可向活跃成员发送有效模式；队员在真实邮箱扫描中验证消息来源、按本地 CLI 授权上限应用模式，成功后原子确认消息，伪造与越权通知不改变权限。发送端同样校验队长授权上限。

`Session` 新增可选 `permission_mode`，模式变更同步写入磁盘，会话恢复继续采用已批准模式；使用更低授权启动旧会话时拒绝越权恢复。历史没有该字段的会话保持兼容。新增真实邮箱消息、伪造来源、拒绝升级和重启恢复测试。原版团队 config.json 的成员模式同步、SDK 远程回调和 UI 实时轮询仍待实现。此补强不新增验收路径，仍按 **22/364/1491/29** 保守登记。

### 终端模式切换与持久化一致性

`/mode` 现复用 SDK/团队同一权限校验和会话保存路径，避免终端显示已切换但重启丢失；只读启动跳过会授权文件编辑的 acceptEdits，依次使用可用模式。新增磁盘会话重读回归。该路径仍属部分实现，原版完整交互、自动分类器和终端 UI 尚未等价。

权限状态更新已进一步改为**先保存会话，再切换内存授权**：模拟会话目录被不安全链接替换时，模式变更失败且旧权限保持不变；修复存储后可重试，避免持久化失败导致权限状态与磁盘不一致。该边界有文件系统故障注入测试。

### 2026-10-29：MCP 资源订阅与推送事件回环（统一 Issue #3）

对照原始 `services/mcp/client.ts` 中的 `capabilities.resources.subscribe` 及 JSON-RPC 资源订阅通用协议：在实际 stdio 与 HTTP MCP 客户端记录 server capabilities，允许明确 SDK 请求资源订阅/取消，并拒绝不支持订阅的服务。订阅按 server/URI 去重，实际发送 `resources/subscribe`/`resources/unsubscribe`；等待 RPC 回复时穿插到达的 `notifications/resources/updated` 和 `notifications/resources/list_changed` 会保留为结构化事件，附来源服务名并支持 SDK 一次性取走，避免消息丢失或重复消费；HTTP SSE 同样收集在响应中收到的事件。新增 SDK 进程+本地 MCP stdio 服务端到端测试，覆盖响应前通知、重复订阅、取消、取走后队列清空；尚待独立 HTTP GET/SSE 长连、重连恢复订阅、持久化和 UI，仍为部分实现。

### 2026-10-29：MCP 单服务断线恢复和工具请求安全重试（Issue #3）

原版 `services/mcp/client.ts` 对 SSE/进程断开后清除失效连接和工具缓存，下一次操作应重新建立连接；且原样重放已发送的 `tools/call` 可能产生重复副作用。Rust `McpManager` 保存每个配置，新增逐服务 `reconnect_failed`：运行时检测子进程退出、HTTP MCP Session 404，将该服务标记失效；下一模型轮次独立重建会话、完成工具重新发现、校验 alias 冲突并恢复显式订阅。所有 I/O 与校验通过后再一次性替换目标客户端、路由/工具定义和 SDK 状态；其他健康服务不受影响。执行过但未收到结果的 `tools/call` 不自动重放，防止未知远端副作用重复发生。新增真实 stdio 进程首轮退出后重新启动并识别新工具 schema 的集成测试；原版完整退避、并发重连去重、HTTP GET 长连订阅与 OAuth 等仍待实现，源路径保持部分实现。

### 2026-10-29：原始 MCP Prompt 命令至真实模型请求的调用链（统一 Issue #3）

对照原始 `src/services/mcp/client.ts` 第 2033–2096 行 `fetchCommandsForClient`：新增 `src/mcp_prompt.rs`，只识别显式 `/server:prompt (MCP) 参数` 标记；按原版 `args.split(' ')` 的位置顺序绑定 `arguments` 名称。`Agent::ask` 在 UserPromptSubmit Hook 和会话持久化前，从 MCP `prompts/list` 验证显式请求的来源/名称，再调用 `prompts/get`，将文本消息（按 role 顺序）作为普通用户上下文交给现有多模型 Agent。禁止将服务端 Prompt 升格为系统指令；图片/音频/嵌入资源等目前 ModelRequest 不支持的内容显式拒绝而非静默丢弃。SDK 和终端均通过此 Agent 主链使用，增加 `/mcp-prompts` 与 SDK `mcp_prompt_commands` 的列表、描述与 argNames。新增真实 stdio MCP 子进程+HTTP 模型模拟服务+SDK 输入的端到端集成测试，断言参数、模型请求内容和命令可见性；另有空格、非法名称、非文本拒绝单测。仍需多模态消息、完整 Prompt 命令生命周期、远程认证/重连等完整等价；**没有升级任何源文件验收状态**。


### 2026-10-29：Shell 复合命令权限边界与 MCP 工具后置 Hook 结果回写（统一 Issue #3）

直接从用户 2.1.88 原始 ZIP 复核 `src/tools/BashTool/bashPermissions.ts`、`src/utils/permissions/shellRuleMatching.ts`、`src/utils/bash/commands.ts`、`src/services/tools/toolHooks.ts` 和 `src/utils/hooks.ts`，并对照现有 Rust Agent→ToolExecutor→Shell/MCP→Hook→模型历史的上下游调用链。

- `src/permission_rules.rs`：修复单条 `Bash(printf *)` 等前缀/通配规则直接覆盖 `printf ok && touch forbidden` 的授权缺口。现在逐段识别 Shell 控制操作符，`deny`、`ask` 在任一子命令上优先；普通 `allow` 必须覆盖所有可分析子命令，无法可靠分析的命令替换、重定向、后台执行和解释器 `-c` 不自动获得宽泛授权。明确精确匹配整条命令的用户规则仍可生效。测试 `compound_shell_permissions_cannot_hide_unapproved_commands` 与 `compound_permission_decisions_reach_real_shell_and_filesystem` 覆盖拒绝后没有产生磁盘副作用、允许后实际 Shell 运行、子命令 deny/ask 和命令替换。
- `src/hooks.rs`、`src/agent.rs`：支持原版 `PostToolUse.hookSpecificOutput.updatedMCPToolOutput`，连续 Hook 的下游收到前一 Hook 修改后的工具输出；Agent 仅对真正 MCP 工具将替换结果写入模型下一轮上下文，普通本地工具不能借此改写输出。新增 `post_tool_mcp_output_is_replaced_before_next_hook` 和 CLI→模拟模型 HTTP→真实 stdio MCP 子进程→Hook 子进程→下一轮模型请求端到端回归 `mcp_post_tool_hook_replaces_model_visible_output_end_to_end`，确保敏感原始工具输出不会绕过替换直接进入下一轮模型请求。

以上属于**既有部分实现主链的补齐**，不能因为新增测试通过就把 362 冻结路径勾选为完成。原版 Bash AST、重定向安全、完整 Shell 语义、全部 Hook 类型/权限决策/取消/并行时序和 MCP 多传输仍未闭环。1,906 条路径分类及 Issue #3 冻结范围保持不变；本次不虚增已验收或未迁移闭环数。


## 2026-10-29：按功能地图推进 /add-dir 工作目录授权

对照原版 `src/commands/add-dir/validation.ts`、`index.ts`、`add-dir.tsx` 和目录权限判定，新增 Rust 目录校验、路径边界与符号链接检查，接入 CLI `/add-dir PATH [--remember]` 用户确认、`/dirs` 查询、文件工具读写、搜索、检查点及配置文件原子持久化，启动恢复已保存的额外目录。原版 JSX 表单、沙箱刷新、SDK 跨入口和团队子代理继承权限仍缺，因此 3 条原路径由未迁移改为部分实现，未提升已验收数。

### 2026-10-29：SDK 文本与图片混合消息贯通到多模型及会话（Issue #3）

对照原始 2.1.88 SDK/Headless 用户消息数组、模型请求和消息恢复路径：新增 `src/multimodal.rs`，验证 image/png、jpeg、gif、webp 的本地 base64 图片，限制大小/数量，拒绝远程 URL、未知块及无效编码，不在文本预览中暴露原始 base64。`protocol::Message` 新增可选结构化块并存入既有会话，`Agent::ask_with_blocks` 让 SDK 直接消息和排队图片单项实际进入模型 API。OpenAI 兼容非流式和 SSE 分别使用同一 `image_url` data URI 转换，Gemini 非流式与流式共用 `inlineData` 转换；其他角色及工具调用不受影响。队列中的图片不会和旁边不同内容强行合批造成类型丢失，并继续保留原始 UUID 回放。新增真实 SDK 进程/模拟模型 HTTP 的直接+排队图片请求测试，同时验证原始图片块落盘及请求内容保真；另有两种模型编码适配、非法数据边界测试。当前仅支持本地 base64 图片和文本，语音、PDF、非图片资源、终端多模态 UI、全部 SDK Hook/权限/复杂附件仍未等价。本改动不升级任何已验收计数，CI 以最新提交检查结果为准。

### 2026-10-29：补齐 Headless 图片与文本同负载合批回放

进一步对照 `src/cli/print.ts` 的 `canBatchWith` 与 `joinPromptValues`：当排队消息 `workload` 和 `isMeta` 相同且为同组 `prompt`，允许图片和文本块跟随原队列顺序在一个 ModelRequest 中合并，不丢失图片、文本段或 UUID；既有 `skipSlashCommands`/Bash/权限通知的阻断规则保留。加入真实 SDK 进程与 HTTP 模拟模型单次请求测试：两条原始 UUID 分别回放/分别返回结果，模型仅接到一次包含文字+原始图片的图文请求，且本次队列排空；完整原厂多模态 UI/图片编码及 SDK 异步状态机仍待等价验收。本轮修复将旧文本数组测试从扁平字符串改为线协议中的结构化文本块逐项对照，验证拒绝非文本文件类型仍生效。以上不构成源路径逐文件完整验收。

### 2026-10-29：MCP Prompt 图片与 SDK 多模态模型通道贯通

原版 `services/mcp/client.ts::fetchCommandsForClient` 使用 `transformResultContent` 处理 MCP `prompts/get` 的图片内容；此前 Rust `/server:prompt (MCP)` 遇到图片会直接报错。本轮在 `mcp_prompt.rs` 中保留每条消息的 role 与原内容顺序，将 `mimeType`、base64 图片作为结构化图片块传给已验证的 `multimodal` 校验与模型转换，不把远端内容提升为系统指令；文本命令继续保留原有纯文本处理。`Agent` 在 UserPromptSubmit Hook 和会话保存前获得展开后内容及图像块，并通过 OpenAI/Gemini 通用模型请求链发送。新增真实 MCP stdio 子进程、SDK 和 HTTP 视觉模型全调用链集成测试，以及格式缺失/非法编码单测。MCP 音频、完整嵌入资源、历史多模态恢复等仍不等价，固定 Issue #3 保持开放，不将相关文件标已验收。

## 会话查询、持久命名与全文导出（2026-10-29）

对照原版 `commands/export/export.tsx`、`commands/rename/rename.ts`、`commands/resume/resume.tsx` 及 `utils/exportRenderer.tsx`，增加 `session_control.rs`，实现实际历史会话扫描及排序、独立标题持久化、team 身份隔离、原会话恢复标识及含工具调用/结果的 UTF-8 文本导出，CLI 和交互聊天入口均已连接；导出拒绝访问工作区外路径和 symlink，进程级回归测试覆盖历史文件与跨进程标题保持。原版 React/Ink 精确渲染、剪贴板对话框、空参数 AI 自动生成标题、同/跨项目模糊检索和交互中直接切换活跃会话尚未等价，上述原路径只列部分实现，不新增已验收。

会话历史扩展：按照原版 Resume 用户语义实现大小写无关标题/UUID 搜索、精确优先、歧义拒绝、CLI 直接 `chat --resume 搜索词`，以及真实进程恢复回归；仅完成当前项目范围搜索，原版跨项目与 React 交互选择器仍缺。

会话自动命名调用链：`/rename` 不传参数时，使用配置的当前模型单独生成 2-4 词 kebab-case JSON 名称；验证输出/工具请求、保存标题，不增加聊天消息、单独计入模型用量。真实 CLI→HTTP 模型→标题落盘与重启恢复测试。原版专有 Haiku 请求和异步中断/timeout 仍有差异，只登记部分实现。


## 2026-10-29：MCP 双传输服务端主动请求与阻塞回执

原始源码对照：2.1.88 `src/services/mcp/client.ts` 的 `ListRootsRequestSchema`（返回工作目录 file URI）、`ElicitRequestSchema` 初始化阶段安全取消，以及 MCP SDK 双向 JSON-RPC 调用。原版的交互式 elicitation 回调仍须迁移，不以本次默认取消冒充完整功能。

Rust 修改：`src/mcp.rs` 将服务端主动请求放在客户端待决请求 ID 匹配之前处理，避免双方独立 ID 空间冲突；实现 `roots/list`、`ping`、`elicitation/create` 安全取消、未知方法 `-32601` 回执，初始化时按原版以空 `roots`/`elicitation` 对象声明能力，避免 Java MCP 服务端不兼容。`src/mcp_http.rs` 对 Streamable HTTP SSE 收到的服务端请求使用同会话、同协议头的独立 POST 回执，同时保持原始 SSE 长连接，不必等待 EOF 才取得工具结果；stdio 与 HTTP 复用请求语义。

验证：真实 stdio Shell 子进程先请求根目录、用户确认与 ping，收到回执才完成工具发现；真实 TCP HTTP 服务在保持 SSE 连接期间依次请求根目录和 ping，检查 session ID、回执及工具目录返回。补充未知方法/通知/字符串请求 ID 边界测试。中途发现一次 Rust 可变借用编译错误及一次测试错误假设 JSON 字段顺序，已分别修复。最终代码 HEAD `f6013f0c774e3940abe780dc70f7d433fc15922b` 的 GitHub Actions [40812932222](https://github.com/sy220284/222/actions/runs/40812932222) 编译及全部 268 项测试通过（212 单元、51 CLI、2 Hooks、1 台账、2 隐私）。

未闭环：HTTP 独立 GET 通知流、旧 SSE/其他传输、服务器交互式 elicitation、sampling/用户权限回调、完整异步中断/恢复和终端 UI。以上原路径继续保留“部分实现”；Issue #3 固定 362 项不勾选，迁移分类仍为 1,906 = 22 排除 + 375 部分实现 + 1,480 未迁移 + 29 已验收，非排除尚有 1,855 条未完成最终等价。


## 2026-10-29：Shell 运行与识别主链迁移

对照原版 `src/utils/shell/{resolveDefaultShell,shellProvider,powershellDetection,powershellProvider,outputLimits}.ts` 及 `src/utils/which.ts`：新增 Rust `src/shell_runtime.rs`，配置 `default_shell`（bash/powershell）、跨平台 PATH 查询、Linux snap PowerShell 回退、PowerShell Core/桌面版本识别、标准无用户配置参数、动态输出长度上限；`Agent`、前台 `run_command`、后台 Shell、子代理和团队成员共用同一 Shell 选择和权限上限，保留已有运行时返回/错误和任务生命周期。增加命令执行返回码、PATH 文件权限、配置拒绝、真实 CLI→模型→PowerShell 测试。原版沙箱 PowerShell UTF-16/Base64 命令路径、PWD 追踪、Git Bash 自动探测、Windows 实机兼容和完整脚本时序尚待验证，相关 6 条严格计入**部分实现**，不提升已验收数。


### Shell 会话环境变量生产链补充

按原版 `src/utils/sessionEnvVars.ts` 的会话内增删查语义，提供交互 `/env NAME=VALUE`、`/env -u NAME`、`/env`；变量不污染主进程全局环境，也不写会话文件。Agent 负责唯一状态，前台命令、后台任务与子代理/团队成员共享其本会话变量快照；更新后后台任务新启动的进程得到新值。真实 CLI→模拟模型→Shell→模型回填覆盖设置、撤销和隔离。完整 Hook/MCP/LSP 子进程环境透传和原版交互表单待补，该路径仅计部分实现。

### 2026-10-29：Issue #3 MCP roots/list 工作区身份与重连一致性

对照 2.1.88 源码 `src/services/mcp/client.ts` 1009–1018 行的 ListRootsRequestSchema 及当前 Rust `server_request_reply`：原实现以进程 `current_dir()` 回答任何服务端 roots/list，即使 CLI 用 `--workspace` 指向不同项目，或子 Agent 在独立 worktree。此时 MCP 服务获得错误根目录，破坏路径权限和上下文隔离。改为每个 MCP 管理器持有经过 canonicalize 的当前 Agent 根目录，stdio 子进程在工作区内启动；stdio 和 Streamable HTTP SSE 服务端 roots/list 均从绑定根目录产生 URI。断线重连也继承此根目录而非回退为进程 CWD。新增真实 stdio 子进程初始化时服务端主动 roots/list 并对照期望 URI 测试，HTTP SSE 服务端主动 roots/list 检验与独立工作区相等，另加权限回归检测根目录不等于全局 CWD。仍不代表 MCP client 完整等价；交互 elicitation/独立 GET SSE/完整授权/UI 尚缺，源路径继续标部分实现。


### 2026-10-29：通用子进程环境隔离

新建 `src/subprocess_env.rs`，选择启用 `ATLAS_SUBPROCESS_ENV_SCRUB=1|true|yes|on` 时，Shell 前后台、Git、Hook、LSP、MCP stdio 真实启动入口统一过滤模型 API/云凭据、GHA OIDC、缓存运行令牌、OTLP 凭据头和其 `INPUT_*` 镜像；保留按原版约定工作流所需的 `GH_TOKEN` 与 `GITHUB_TOKEN`，且过滤发生在自定义环境变量注入之后。补充真实子进程、CLI→模型→Shell 回填集成测试；原始上游代理和动态远程环境仍未完全覆盖，因此仅登记部分实现。

### 2026-10-29：Issue #3 MCP 独立 HTTP GET 资源事件收取和 SDK 状态通道

原版 Streamable HTTP 传输可通过与 tools/call POST 分离的 GET SSE 通道获取资源通知。Rust 之前只解析 POST 的 SSE，故没有模型请求时的服务器资源更新会丢失。本轮给 `HttpClient` 新增**受限显式轮询** GET SSE：复用协议/Session-ID/认证头、以服务器事件 `id:` 生成下次请求 `Last-Event-ID`、增量/分块完整 SSE 帧读取、收到完整资源通知或工具目录变化即可返回而非等待远端关闭连接；保持响应 2 MiB 限制和最大等待时间，支持不提供 GET 的服务器返回 405。隔离不同 MCP 服务的轮询错误、会话 404 标记等待重连，开放 SDK `mcp_poll_resource_updates` 平铺和嵌套控制，`max_wait_ms` 0–30000（默认 1000）。新增真实本地 TCP HTTP 连续 GET + 事件 ID 续传、真实 CLI SDK→MCP HTTP GET→结构化更新回执测试。独立后台常驻监听、旧 SSE/WS、网络自动重连和完整 MCP UI 仍不等价，四条 MCP 原路径继续部分实现。


### 2026-10-29：原始提示历史与粘贴缓存进入交互式会话主链

对照 2.1.88 原版 `src/history.ts` 与 `src/utils/pasteStore.ts`：新增 `src/prompt_history.rs`，提供按项目隔离的全局 JSONL 历史、同一会话优先的 100 条浏览窗口、按显示文本去重的最近历史、已落盘提交撤销过滤、粘贴引用逆序展开、1024 UTF-16 单位内联与 SHA-256 长文本缓存、过期清理、跨进程文件锁。交互式 CLI 现在在调用模型前记录用户输入，并提供 `/history [query]` 真实检索。新增并发追加、错误 JSON、跨会话优先级、跨项目隔离、撤销、长粘贴恢复与占位符防重解析测试；真实 CLI 双进程与模拟模型 HTTP 集成测试验证持久化及会话隔离。尚未移植 React 上下键输入框、Ctrl-R 弹窗、异步 flush 重试与图像缓存，两个原路径只登记为部分实现；不得认定完整等价。原 Issue #3 的 362 冻结清单保持不变。


### 2026-10-29：SDK 队列操作持久化与故障边界

沿原始 `src/utils/messageQueueManager.ts` 的 `logOperation` 与 `src/utils/sessionStorage.ts::recordQueueOperation` 调用链，新增 `src/queue_journal.rs`，在真实 SDK 会话对入队、按优先级出队、批量出队、过滤移除、按身份移除和恢复编辑输入写入 JSONL 操作记录。记录包含原版 operation、时间戳、sessionId 及可选纯文本内容；跨进程写入使用文件锁、同步落盘，阻止会话日志路径通过符号链接逃逸。真实 SDK 子进程测试核查操作顺序、内容与会话关联，故障注入测试验证符号链接拒绝且不写入外部路径。原版完整 sessionStorage（5106 行）、React 队列订阅与远程桥接仍未等价，相关原路径不提升为已验收。

### 2026-10-29：MCP 服务器级权限规则回归闭环（Issue #3）

对照原版 2.1.88 的 `src/utils/permissions/permissions.ts` 233–269 行 `toolMatchesRule`：`mcp__server` 与 `mcp__server__*` 都可匹配该服务的全部真实工具，同时精确工具规则应覆盖更泛规则，`deny` 优先于 `ask` 优先于 `allow`，与同名内置 Write、Read 不发生误授权或误拦截。此前 Rust `permission_rules::policy_for` 只对工具全名逐字匹配，导致服务器级配置无法授权或屏蔽 MCP 工具。

新增受限的 `mcp_server_rule_matches`，只识别标准 `mcp__server` 和 `mcp__server__*`，且只匹配名称包含完整 server 与工具部分的 MCP 实际工具，不处理父括号含内容的规则，防止不符合原版的误命中；与已有 `ToolExecutor::approve_external` 挂钩，测试覆盖 server allow、对单个危险工具 ask、其他 server deny、跨服务隔离、内置工具同名冲突及具体 deny 不能被广域 allow 绕过。完整分类器/权限来源/沙箱模式/UI 审批仍缺，因此两条源文件严格保持“部分实现”。


## 2026-10-29：用户调整执行顺序——先写 200 条源码路径的 Rust，再统一完整验收

这次先交付实现，不把“Rust 函数存在”冒充原版全链路等价。`src/portable_source_utils.rs` 已根据原版逐一写出 24 条源路径对应的核心算法，覆盖环形缓存、二进制查找缓存、模型内容块排序、hash/DJB2、缓存路径、URI 地址、沙箱标记处理、XDG、CLI 参数、UUID/Tagged ID、临时文件、Shell 超时、键盘键位、人类对话判定、系统提示类型、通知/编辑模式、Token 预算、插件策略、技能使用评分、目录判断、团队功能闸门；其中 Shell 超时实际接通 `ToolExecutor::run_command`。对应台账状态暂记“部分实现（代码已写，待验收）”，不是已验收。后续继续按功能地图推进至用户要求的 200 项，再统一做原版语义、调用链、UI/SDK 和边界验收。

### 2026-10-29：WebFetch 细粒度主机授权规则与执行链（Issue #3）

对照原始 2.1.88 `src/tools/WebFetchTool/WebFetchTool.ts` 50–63、104–173 行，其 `webFetchToolInputToPermissionRuleContent` 使用 `domain:${new URL(url).hostname}` 并经 `getRuleByContentsForTool` 按**精确内容键**查找 deny/ask/allow。Rust 之前只支持整个 WebFetch 工具名称，不检查 `WebFetch(domain:example.com)`，导致配置的单域名拒绝规则可能失效。

修复 `permission_rules::canonical_tool` 对 `WebFetch/web_fetch` 归一化，在权限引擎中使用规范 URL.hostname（不包含端口/路径）做完全匹配，不把通配 `domain:*` 随意扩大为允许；deny 优先于 ask/allow。真实 `ToolExecutor::approve_external` 测试确保即使全局 Full 权限也不能绕过域名 deny，配置明确允许的 host 在非交互 Ask 模式下可执行，而同后缀恶意主机名不可绕过。保留原版 URL 解析失败后回退为普通审批。完整代理分类器、预批准来源、全部 Hook 和跨重定向最终主机检查仍未完全等价。


### 展示、SDK 与本地状态领域代码追加

`src/portable_ui_runtime.rs` 新增 XML/IDE 注入展示标签清理、OSC-8 链接格式、隐私限制、Git 指南环境决策、通用多模型能力覆写、SDK 任务进度消息结构、团队进程常量和插槽、内置检索闸门、记忆类型/Git 仓库发现、远程环境优先级选择、描述长度聚合、自动模式拒绝记录、插件策略、通用上下文升级提示、提示词来源分类、本地文件语种识别、跨模型团队默认值。真实 `session_control::list` 使用标题清理函数。共 18 条原路径从未迁移改为部分实现/待最终验收，**没有提升已验收数**。


### 2026-10-29：语义数字转换的 JavaScript 精度与负零差异修复

**原版证据**：用户提供的 2.1.88 `src/utils/semanticNumber.ts` 中，只有匹配 `/^-?\\d+(\\.\\d+)?$/` 的字符串才调用 `Number(v)`，且 `Number.isFinite` 后交给内部 Zod schema；`Number` 使用 IEEE-754 双精度，不能直接等同 Rust 的 `i64/u64` 字符串解析。原版对 `9007199254740993` 的结果是 `9007199254740992`，对 `-0` 保留负零。Node 22 本地运行原版等效转换已核对这两种结果。

**Rust 缺口与实际修复**：原 `src/semantic_inputs.rs::number` 优先解析 `i64/u64`，会错误保留 JS 已丢失的低位；工具整数参数还会拒绝合法的 `-0` 或直接 JSON 浮点整数（如 `30.0`）。现先按双精度转换，再转换为可精确表示该双精度结果的 JSON 数字；负零在通用语义转换中保留，在无符号整数工具参数边界规范化为零。模型直接传入的 JSON 数值字面量也按 JS Number 精度处理，避免与字符串数字走两套精度规则。

**验证**：新增 `semantic_inputs::tests::js_number_precision_and_negative_zero_match_original_preprocessor`，覆盖正负超过 2^53 的精度舍入、负零、直接 JSON 数字与整数型浮点、超出 u64 范围的拒绝。GitHub 提交 `f24f20af22abfb1149067cfc45cba37fe552ae49`；最终 CI 以对应及后续 HEAD 的实际结果为准，未将这两条原路径提升为完整验收。

**仍缺**：原版 Zod 泛型可选/默认组合、全部模型工具 schema 的真实类型边界及上下游用户行为，仍按“部分实现”追踪。

### 2026-10-29：WebFetch 重定向跨域权限重新验收

对照 WebFetchTool 原始 `domain:<hostname>` 权限：原 `ToolExecutor` 仅审查初始 URL，`webfetch.rs` 允许自主跨域 3xx 跳转；即使目标域名有显式 deny，后续 HTTP 仍可能发出。本轮对每次 URL.hostname 跨域跳转在下一请求之前调用 `ToolExecutor::require_approval`：Full 仍受显式 deny，Ask 非交互无法借初始允许域名绕行。原私网/SSRF 限制和 5 跳限制维持。新模拟 HTTP 302 跳至另一个主机的回归测试验证显式 deny 在访问目标前生效。该补齐不涵盖完整浏览器、认证、页面摘要和交互审批，原路径保持部分实现。


### 2026-10-29：异步与队列工具 Rust 实现

新增 `src/portable_async_runtime.rs`，实现 16 条原版源路径的可调用逻辑：Agent/request ID、按模式和主线程隔离的排队处理、异步串行、一次性 Promise 响应器、父子取消传播及合并超时、异步流、SDK 有界事件队列/UUID 回填、用户提示意图检测、会话 UUID/URL/JSONL 选择器、ToolPool 去重分组、按 mtime 失效的文件读取缓存、FS 文件锁、异步任务局部 workload、空闲状态及缓冲写入。当前只是源代码行为进入 Rust，并非原版所有平台/时序的最终等价验收。


### 平台和 Shell 处理 Rust 代码

新增 `src/portable_platform.rs`，实现 Windows/WSL IDE 路径转化、Windows Git Bash 查找安全判断、Shell 前缀/重定向/heredoc 引号处理、Cron 抖动配置边界校验、日期展示/locale、宽松语义化版本比较及范围判断、EPIPE 容错输出等 8 条路径的行为代码；原版全部平台及语义尚未最终等价验收。


### 2026-10-29：命令注册索引逐文件 Rust 移植（79 条）

逐文件读取原版 `src/commands/*/index`，`src/command_manifest.rs` 建立 79 条原始命令索引元数据，保留命令名、入口类别、别名、参数提示、描述、隐藏或禁用状态；原版 18 条明确 disabled stub 路径不变。已接入 `chat` 的别名规范化与 `/commands` 目录查询。特定命令原版懒加载、React/Ink 交互、复杂动态开关和外部连接服务仍须独立重写；**这是 79 个 index 元数据文件的代码实现，不等于 79 条完整业务命令已实现**，最终验收统一后置。


### 终端 Ink、提示输入、界面常量 20 条源代码

`src/portable_ink_core.rs` 新增真正的事件停止传播与事件监听注册/删除、点击和焦点事件类型、DECSET 终端焦点、Yoga 盒内容宽度与测量、ANSI/Unicode 行宽缓存、宽行选择、换行、非整数诊断、OSC 标题、Agent 来源显示、主题真彩、Diff 开关、Bash/Prompt 输入转换及三项终端常量。已写单元测试，尚未把所有原版 React 组件和终端渲染路径迁移完毕，因此 20 条原路径暂标部分实现；最终等价验收后置。


### 2026-10-29：补齐本轮200个原始文件 Rust 代码目标

按照用户“先完成200项文件代码实现，统一后置等价验收”的调整，本轮在已有 66 项代码移植之后，额外补写命令索引79项、终端与展示20项、配置/策略/提示/工具解析35项（合计200项）。其中最终35项包含：本地时间/会话起始日期、ToolResult 媒体限制和批量预算、系统 Prompt Section 缓存、XML 标签定义、凭据展示遮罩、打包/工作目录状态、子进程调用、GitHub CLI 授权检查、推理即时切换、ANSI 颜色、泛型惰性 Schema、通用模型别名、修饰键状态、安装与插件策略、用户配置/MCP 错误合并、内部写入消重、组织托管设置路径及 JSON Schema 输出、Bash/PowerShell 工具分流、独立代理身份、团队通信提示、Todo 类型/验证、工具 Schema 会话缓存、轻量 YAML 解析、Schema 转换缓存、权限判定状态、电脑输入后端校验、Git 可用性、Settings 编辑校验、Nohup/Sleep Shell 解析等。新增 `src/portable_configuration.rs` 与 `src/portable_settings_runtime.rs` 以及测试；不是最终等价验收，尚需实际上游 UI/CLI/SDK 接线、完整异常与平台语义和源码逐文件完整对照。


## 2026-10-29：多模型请求瞬态失败恢复（原版 withRetry 主链继续补齐）

对照用户原始 2.1.88 ZIP 中 `src/services/api/withRetry.ts` 的 `shouldRetry`、`getRetryAfter`、`getRetryDelay` 及 `src/query.ts` 的模型重试/回退路径，Rust `src/provider.rs` 新增 HTTP 请求开始阶段的有限重试，接入 OpenAI-compatible/Gemini 的普通请求与 `src/sse.rs`、`src/gemini_sse.rs` 的流式连接建立阶段。仅重放相同 JSON 请求体；不会在模型回复之前执行工具或向会话写入虚假回复。支持暂时性 408/409/429/500/502/503/504/529、连接失败和超时；明确 `x-should-retry: false` 不重试，整数秒 `Retry-After` 有 30 秒上限，最多四次请求。流式响应一旦开始就不自动重放，避免重复输出和工具调用。进一步对照 `src/query.ts` 的流完成状态：OpenAI-compatible SSE 在连接关闭时既没有 `[DONE]` 也没有 `finish_reason`，现在明确报错，不会把看似完整、但尚未被模型确认的工具调用执行。真实 CLI + 模拟 SSE 截断测试使用 `write_file` 生成合法工具参数但不发送结束标记，验证进程失败、工作区文件未创建、会话历史未记录未确认的工具调用。

新增真实 CLI→HTTP→工具→会话持久化测试：503→429→正常工具调用→正常模型回复，验证请求体相同、工具输出恰好回填一次；另外测试 503 显式禁止重试、400 参数错误，以及 OpenAI/Gemini SSE 首帧前的重试与增量去重。最终是否通过以对应 HEAD 的 GitHub Actions 为准。

进一步修复 `src/agent.rs`：模型调用或压缩模型请求最终失败后，先累计 API 耗时并保存会话账本；普通问答失败也累计墙钟耗时。真实 CLI 故障注入延迟 35ms 并复查 `.atlas/sessions/*.json`，避免失败调用显示为零耗时。非 JSON 的 HTTP 错误正文现在仍保留原始状态码，避免把 400/503 误报为 JSON 解析错误。上述测试的通过状态须以最终 HEAD 的 CI 为准。

**保守状态不变**：`withRetry.ts`、`errors.ts`、`QueryEngine.ts` 和 `query.ts` 均仍为部分实现。尚缺原版中止信号取消等待、按查询来源限制 529、重试期间 SDK 进度事件、完整 HTTP 日期格式 Retry-After、回退模型选择、已开始流的中断处理及可配置重试预算。不得将本次有限重试视为整条源码路径等价。


### 2026-10-29：补全终端事件和宽度已有 Rust 主链

直接扩展已有 `src/portable_ink_core.rs` 单状态来源，不引入重复终端引擎；新实现 `terminal-event.ts` 事件阶段、可取消性和 propagation，`keyboard-event.ts` 的控制键/可打印字符/修饰键，补 `stringWidth.ts` 的 emoji、组合码点基础显示宽度。保留已有事件与布局模块，暂记上述 3 条源码路径部分实现，后续 TUI Unicode 全面验收统一处理。

## Issue #3 冻结 362 项统一验收进度（PR #2）

根据 Issue #3 冻结路径与最新逐文件矩阵逐项交叉核对：362/362 原路径存在、362 项仍属部分实现、0 项具备足以关闭冻结任务的整体等价验收证据。详见 [ISSUE3_FROZEN_362_AUDIT.md](ISSUE3_FROZEN_362_AUDIT.md)。`src/permission_rules.rs` 已按 2.1.88 原版 `shellRuleMatching.ts` 189–228 行补齐权限建议对象及相应回归，完整权限执行链仍在开放状态。仓库不存在 PR #3，所有代码均只进入 Draft PR #2。


## 2026-10-29：先实现代码、后统一验收的 400 条原路径 Rust 适配

在 `src/generated_ports/` 增加 400 个原始路径独立 Rust 模块，源码来自用户提供的 2.1.88 TypeScript/JSX 语法树，转换为 serde 类型、常量、可执行简单表达式函数和基础 JSX 视图节点。增加原生 Rust `runtime.rs` 值运算、`ui.rs` 虚拟节点，以及分组导出注册表。`atlas-agent inspect-source <原路径> [--export 名称] [--args JSON数组]` 可从 CLI 调用导出和获取 JSX 视图结构，不依赖原厂服务。每条映射单独登记于 `FILE_PORT_MATRIX.md`。

这些文件只完成**首轮代码转写**。JSX 事件/Hooks、复杂异步状态、副作用、未覆盖的函数和复杂类型尚不能视作完整重写；Rust 简单表达式及静态布局的语义与原版仍有差异。因此本次 **400 条仅升级为部分实现，已验收保持原值**；合并前最终逐文件等价验收仍不可省略。

### 冻结 Issue #3 的 Bash/PowerShell 授权建议源码补齐

直接核对 2.1.88 `src/tools/BashTool/bashPermissions.ts` 161–354 行以及 `src/tools/PowerShellTool/powershellPermissions.ts` 143–159 行，移植安全环境前缀、2 词建议、heredoc 和多行分支、危险首词 UI 过滤、PowerShell 星号/换行拒绝，以及共享权限规则 JSON 对象。源码依据与回归见 `src/bash_permission_suggestions.rs`。现阶段功能函数尚未完整接通交互式权限决策 UI，也未实现完整 Shell AST 和 PowerShell 执行器，冻结条目继续保持部分实现、不得验收关闭。

### PowerShell/Bash 权限作用域纠正（Issue #3）

依据原始 `src/tools/PowerShellTool/powershellPermissions.ts` 163–326 行、`src/utils/powershell/parser.ts` 814–826/1326–1451 行与 `readOnlyValidation.ts` 976–994 行，修复之前默认 PowerShell 运行时仍把 `run_command` 的规则映射到 Bash、导致 PowerShell 专用 deny/allow 内容规则未进入执行链的问题。Rust `src/powershell_permission_matching.rs` 实现不区分大小写、按原版常用别名转换和模块前缀 fail-open 防护；`src/tools.rs` 的权限评估按实际 shell 类型分流。新增源级边界与拒绝回归。仍缺原版 PowerShell AST、路径级权限、沙箱、完整审批模型；对应冻结原路径保留部分实现。


## 2026-10-29：两批约600项回查与剩余867项继续推进

按 2.1.88 原始源码回查，发现前一轮 `generated_ports` 为 AST 首轮投影：对类 React/Ink 的交互/Hook、复杂表达式与调用副作用没有完成重写；部分 JSX 动态属性和子节点被替换为 `Value::Null`，虽通过编译但**不能视为用户可用完整 UI**。另发现通用 JS 数字转换对空字符串、数组、负号、对象 `length` 的处理错误。已按源码修改 `generated_ports/runtime.rs`，对投影节点新增缺失绑定检测并在 `inspect-source` 输出警告，阻止无声充当已完成界面。过往 400 项的已验收数仍为 0，新代码仍在待统一最终验收状态。

从剩余未迁移中继续补 `src/portable_missing_utilities.rs` 的 11 条实际 Rust 行为：Shell 内置命令规格与路径过滤、交互环境下 `@teammate` 直接写团队邮箱、文字匹配逆色显示节点、Tips 展示会话间隔、终端记忆、通用客户端身份字符串、CLI 退出消息/退出码。实际 CLI 接入 `@teammate`、`/shell-spec`。动态 Fig 规格的远端加载、完整 UI/SDK 入口、原版全边界和恢复路径留待统一验收。

### 2026-10-29：Issue #3 PowerShell 变量泄露与伪装应用授权漏洞

对照原始 2.1.88 `src/tools/PowerShellTool/powershellPermissions.ts` 539–624、932–962、1300–1315、1458–1515 行：原版在准许 cmdlet 前验证 AST 的变量/子表达式/脚本路径属性；Rust 词法权限曾将 `Get-Content $env:SECRET_API_KEY`、`Write-Output "$env:SECRET_API_KEY"`、`scripts\\Get-Content secret` 当作已授权命令执行。现补上动态/环境变量、@splat、path-qualified executable、#Requires、using 和 stop-parsing 的保守拒绝自动放行规则；拒绝仍优先，静态单引号命令保持通过。新增权限匹配回归及真实 `ToolExecutor::approve_external` 授权链回归。原版完整 PowerShell AST、控制流/脚本块、反馈 UI 仍未等价，本次不勾选 Issue #3 冻结项，自动生成模块不作为等价证据。

### 2026-10-29：Issue #3 原版 Hook 事件名协议检查

对照原版 `src/utils/hooks.ts` 581–589 行，Rust 过去接受 `hookSpecificOutput` 但不核实 `hookEventName`，可能把 PostToolUse 的响应当作 PreToolUse 输入修改或权限决策。现在非空 `hookSpecificOutput` 必须提供和当前事件完全一致的 `hookEventName`，缺失或错事件返回协议错误；真实 Hook 子进程集成用例更新为原版完整协议，并新增错误事件/缺失事件的拒绝测试。仍须完善原版 Hook 串并行调度、失败回填、各生命周期事件与 UI，不将 `src/utils/hooks.ts` 标已验收。


## 2026-10-29：按功能入口审计补齐 ToolSearch 与 AskUserQuestion 真实执行链

对应原始 2.1.88 的 `src/tools/ToolSearchTool/ToolSearchTool.ts` 和 `src/tools/AskUserQuestionTool/AskUserQuestionTool.tsx`，新增 `src/tool_search.rs` 与 `src/ask_user_question.rs`，在 `src/agent.rs` 的模型工具定义、PreToolUse/实际执行/PostToolUse、会话工具结果持久化主链接入。

- `ToolSearch`：按照当前真实工具目录检索，支持 `select:A,B`、工具名精确匹配、MCP 前缀、关键词加权及 `+required` 必须词。模型工具调用的真实查询结果进入下一模型请求，保留于会话。
- `AskUserQuestion`：仅在实际交互式会话暴露，校验问题/选项数量、重复值、多选、Other 自由回答、关闭输入；模型伪造的 `answers` 不作为用户回答，工具只使用终端实际输入。输出与对话持久化。
- 新增模块单元测试和 `tests/integration_cli.rs` 的 CLI→真实 HTTP 模型→工具→下一模型请求→会话文件两条端到端测试；专门检验模型伪造回答不渗透到工具结果。
- 代码 HEAD `1a677171e54da1277128c9f5a5a5d793a299ebb8` 的 GitHub Actions [40838300287](https://github.com/sy220284/222/actions/runs/40838300287)：格式/Clippy、编译/全部测试、依赖检查均成功。之后仅更新迁移台账，最终 HEAD 的 CI 另行判定。

**严格剩余差异**：`ToolSearch` 已补 MCP 延迟模式加载及检索后的下轮动态注入，仍缺原版 `tool_reference` 原生结果块、多供应商传输和部分动态通知语义；`AskUserQuestion` 尚未实现 React/Ink 预览与 SDK 外部审批回执，annotations 未覆盖。两条源路径仍记“部分实现”，没有任何新增跨模块全行为等价验收，不改变 1,855 条未完成等价验收的保守统计。继续保留 Draft PR #2，不合并。

### /branch 会话分叉：原版调用链核对（2026-10-29）

- 原始依据：2.1.88 `src/commands/branch/branch.ts` 中 `createFork`、`getUniqueForkName`、`call`；`src/commands/branch/index.ts` 注册入口。原版调用 `context.resume(sessionId, forkLog, 'fork')` **切换当前会话到分叉**，并保留原会话供恢复。
- Rust 入口：`src/main.rs` 的 `/branch` → `src/agent.rs::Agent::branch_session` → `src/session.rs::Session::save`。修正原先仅保存分叉而继续写入父会话的语义偏差：分叉保存成功后切换活动会话、重绑定个人任务图及 CLI 提示历史，保持父会话不变。新会话后续消息独立保存。
- 回归：`tests/integration_cli.rs::chat_branch_preserves_original_conversation_and_uses_collision_free_titles` 使用真实 CLI 子进程和模拟模型 HTTP，检查连续分叉的父子链、标题冲突处理、分叉后消息只写入活动子会话。代码提交后仍须核实最新 HEAD 的格式、Clippy、完整测试及 CI。
- 尚未等价：原版 JSONL transcript 的逐消息 `forkedFrom.messageUuid`、`parentUuid`、sidechain/progress 过滤、content-replacement 重映射、`context.resume` 的完整 UI/SDK 生命周期及相关日志元数据；Rust 当前以会话 JSON 保存，不能宣称该原路径已完全验收。

### Hook 同轮输入共享、并行执行和权限决策优先级（2026-10-29）

- 原版源码：2.1.88 `src/utils/hooks.ts` 的 `executeHooks`（1952–2010 行）从同一 `hookInput` 构建匹配 Hook；2743–2745 行通过 `all(hookPromises)` 并行运行；2820–2847 行明确 `deny > ask > allow`，而 `executePreToolHooks`（3394–3435 行）提交原始工具参数。Rust 先前把首个 Hook 改写后的参数交给第二个 Hook，并且可能让后到的 `ask` 覆盖 `deny`，与原版语义不同。
- Rust 修复：`src/hooks.rs::HookRunner::run` 使用 `FuturesUnordered` 并发执行匹配命令，所有 Hook 接收同一原始输入；结果汇总按拒绝 > 询问 > 允许，保留非阻断错误、工具参数改写、阻断和 PostToolUse 输出改写。工具执行前再应用最终参数改写。新增两个真实 Shell 进程同步并行回归、权限优先级回归，并恢复 `tests/integration_hook_lifecycle.rs` 的原始输入断言。此项为源码语义修复，**未完成原版全部 Hook 功能，不增加文件级验收数**。
- 尚缺：原版 Hook 完整配置来源/匹配、所有事件、信任门禁、取消/超时反馈、prompt/agent/http/async 类型、UI/SDK 进度及输出聚合边界；仍需逐函数对照。测试结果以最新 HEAD 的 CI 为准。

## 2026-10-29：托管权限策略分片加载与跨文件合并（源码已提交，待验证）

- **原版依据**：`source/src/utils/settings/settings.ts:63–120` 的 `loadManagedFileSettings`、同文件 `settingsMergeCustomizer` / `mergeArrays`，以及 `source/src/utils/settings/managedPath.ts`。原版先加载托管基础策略，再按文件名排序合并 `managed-settings.d/*.json`；对象递归合并、权限规则数组拼接去重，后面的标量配置覆盖前面的值；一个损坏的分片不能阻断其他有效分片。
- **Rust 可达实现**：`src/permission_settings.rs::SettingsPaths::load_policy` 从已配置的 `ATLAS_POLICY_SETTINGS` 基础文件所在目录读取 `managed-settings.d`（可用 `ATLAS_POLICY_DROPIN_DIR` 指定目录），与 `SettingsPaths::load` 的真实规则加载及 `SettingsPaths::edit` 的托管策略禁改判断接通。非 JSON、隐藏文件、非普通文件不进入分片；无效 JSON 或单分片读取失败记录并跳过。合并由 `merge_policy_values` 负责，避免只把新文件映射进台账而不执行。
- **状态**：本轮按用户最新要求只做源码审阅与代码提交，**未运行 Cargo、测试或 CI**；此项仅登记“源码已补齐并提交、待验证”，不提升任何已验收路径，也不勾选 Issue #3 冻结条目。仍缺完整跨平台 MDM/远程托管来源优先级、原版全量设置 schema 验证、策略分片符号链接兼容和权限 UI。冻结 362 项独立保持开放。

### 托管权限来源在 /status 中显示（源码已提交，待验证）

原版 `source/src/utils/settings/settings.ts:124–150` 的 `getManagedFileSettingsPresence` 将基础托管文件和策略分片分别检测，供状态界面显示 `file`、`drop-ins` 或 `file + drop-ins`。Rust `SettingsPaths::managed_file_presence` 现检查有效基础文件及符合命名条件的策略分片；`Agent::status_report` 的真实聊天 `/status` 输出显示来源类别，不展示敏感策略内容。**未运行测试/CI，待后续验证**；终端原版完整状态面板、其他管理来源及 UI 尚未等价。

### 新会话采用设置中的默认权限模式（源码已提交，待验证）

原版 `source/src/utils/permissions/permissionSetup.ts:721–799` 在 CLI 指定模式之后读取 `settings.permissions.defaultMode`，并按设置来源及组织策略限制选择启动模式。Rust `SettingsPaths::default_mode` 现按用户→项目→本地→托管策略顺序提取有效模式，托管-only 时不采用低优先级来源；`src/main.rs` 的真实 Agent 新会话入口在 `--approval ask` 上限内应用 `default/acceptEdits/plan/dontAsk`，已有会话恢复优先保留原持久化模式。原版 `auto` 需要分类器、`bypassPermissions` 不可绕过 CLI 授权上限。**源码已提交、未执行测试/CI**；完整原版模式切换、远程限制及 UI 仍待验证，冻结 Issue #3 不勾选。

### 组织策略禁用绕过审批模式进入真实启动/恢复链（源码已提交，待验证）

对照原版 `source/src/utils/permissions/permissionSetup.ts:704–710,775–799` 和 `source/src/utils/settings/types.ts:67–70`：`permissions.disableBypassPermissionsMode: "disable"` 应阻止进入 `bypassPermissions`。Rust `SettingsPaths::bypass_disabled` 现读取用户、项目、本地与托管来源，尊重托管-only 限制；`src/main.rs` 启动时即使显式 `--approval full` 也将执行上限降至 Ask，已有会话中持久化的绕过模式安全回退 default，后续 `/mode` 和 SDK 切换也受 Agent 授权上限限制。**代码已提交，未运行测试/CI**；原版动态配置重载、远程托管、所有 UI 提示仍待验证，冻结清单不勾选。

### 权限设置附加目录进入真实文件与命令授权范围（源码已提交，待验证）

对照原版 `source/src/utils/permissions/permissionSetup.ts:978–1010` 和 `source/src/utils/settings/types.ts:79–82`，`permissions.additionalDirectories` 应与 `--add-dir` 一同经过工作区目录校验后进入文件/搜索/Shell 可访问范围。Rust `SettingsPaths::additional_directories` 按用户、项目、本地及托管策略来源合并去重，并尊重 managed-only 限制；`src/main.rs` 的 Agent 初始化现将这些路径接入既有 `attach_configured_directories` 校验及真实工具访问链。**仅源码提交，未运行测试/CI**；原版交互授权确认、沙箱刷新、跨工作树/远程路径语义仍待验证，Issue #3 冻结项保持开放。

### /add-dir --remember 按原版保存至项目本地权限设置（源码已提交，待验证）

原版 `source/src/commands/add-dir/add-dir.tsx` 的 `handleAddDirectory` 与 `source/src/utils/permissions/PermissionUpdate.ts:244–266` 将用户确认的附加目录保存到 `localSettings.permissions.additionalDirectories`，并去重。Rust 原先错误写入通用 `config.additional_directories`。现在 `src/main.rs` 的真实交互 `/add-dir --remember` 通过 `SettingsPaths::add_directory(EditableSource::Local)` 原子更新 `.atlas/settings.local.json`，复用已有锁、非法 JSON 保护、托管策略禁改检查，原有会话即时授权保持；下次启动由 `additional_directories` 重新加载。**仅源码提交，未运行测试/CI**；完整沙箱即时刷新、权限 UI 和跨进程设置通知仍待验收。

### 撤销已授权附加目录并同步本地权限设置（源码已提交，待验证）

依据 `source/src/utils/permissions/PermissionUpdate.ts:297–315` 的 `removeDirectories`：先从当前权限上下文移除目录，再持久化到 `localSettings.permissions.additionalDirectories`。Rust `Agent::remove_workspace_directory` 现立即移除非主工作区的额外根目录，后续文件/搜索/命令工具重建授权根列表；交互命令 `/dirs remove PATH` 通过 `SettingsPaths::remove_directory` 原子更新本地权限数组，文件写入失败也保留当前会话撤销结果，拒绝撤销主工作区。**未运行测试/CI**；其他来源目录的跨源优先级、完整权限 UI 和沙箱刷新仍待验证。

### 权限模式修改与默认模式保存接通（源码已提交，待验证）

原版 `source/src/utils/permissions/PermissionUpdate.ts:317–325` 的 `setMode` 将权限模式写入对应设置来源，供下次启动恢复。Rust `SettingsPaths::set_default_mode` 现原子写入 `localSettings.permissions.defaultMode`，终端 `/mode set MODE [--remember]` 先经 Agent 实际授权上限校验和当前会话模式持久化，再按用户选择写入本地默认值；下次启动由已接通的 `SettingsPaths::default_mode` 读取。**仅源码提交、待验证**；原版 UI 设置界面、动态设置订阅与远程模式同步仍未全等价。

### 跨平台自动发现管理员托管权限策略（源码已提交，待验证）

原版 `source/src/utils/settings/managedPath.ts` 在 Linux/macOS/Windows 使用系统级托管目录，而不是要求每个用户单独配置环境变量。Rust `SettingsPaths::for_workspace` 现按平台自动寻找中立路径 `/etc/atlas-agent/managed-settings.json`、`/Library/Application Support/AtlasAgent/managed-settings.json` 或 `C:\\Program Files\\AtlasAgent\\managed-settings.json`，仍允许 `ATLAS_POLICY_SETTINGS` 显式覆盖；既有 `load_policy` 同时加载同目录的 `managed-settings.d`。无托管文件时不改变用户设置。**仅源码提交，未运行测试/CI**；完整系统 MDM、远程策略与用户级注册表优先级仍待实现。


## 2026-10-29：本地源码对照的快捷键运行时重写（完整 UI 链待验收）

对照原版 2.1.88 的 `keybindings/parser.ts`、`match.ts`、`resolver.ts`、`defaultBindings.ts`、`loadUserBindings.ts`、`template.ts`、`shortcutFormat.ts` 及 `commands/keybindings/keybindings.ts`，新增 `src/keybindings.rs` 原生组合键与别名解析、终端修饰键和 Escape 特例、上下文匹配、用户覆盖、`null` 解除绑定、键位配置、保留快捷键检查和安全创建模板。独立 `keybindings` 子命令与聊天 `/keybindings` 已接线，配置不依赖厂商账户且不会上传遥测。真实本地 Rust 4 项针对性测试和编译通过。原版 Ink UI 键事件消费、组合键超时和主动文件监听仍未接入；涉及源路径只登记部分实现、未升级完整验收。此条不代表剩余 816 项全部完成。


## 2026-10-29：快捷键状态化处理与 TUI 适配运行时（仍非 UI 全等价）

直接按 2.1.88 原版 `src/keybindings/KeybindingContext.tsx`、`KeybindingProviderSetup.tsx`、`useKeybinding.ts`、`useShortcutDisplay.ts` 的注册、注销、上下文、回调和 chord 生命周期补齐 `src/keybindings.rs::KeybindingRuntime`。终端输入交给一个状态持有者，支持按键组合的 1000 毫秒过期、首键拦截阻止编辑器吞键、动作回调返回 false 时后续输入仍可处理、注册令牌注销、嵌套界面上下文引用计数、配置更改后取消旧组合键并更新警告与快捷键显示。新增 3 个状态/限时/注销/磁盘重新加载的 Rust 回归测试，均在本地通过。

此运行时提供真实键事件消费接口，但现有 CLI 仍是按行读取，未有 Ink 同级完整事件循环、异步文件监听、全界面生命周期，故新增 4 原路径只标记部分实现，不计已验收；`src/keybindings/loadUserBindings.ts` 仍部分实现。远端 CI 必须按本提交 SHA 单独核对。


## 2026-10-29：后台 Shell 任务原版短 ID、旧记录恢复与输出工具别名（源码已提交、待验收）

逐项对照原版 `src/Task.ts:78–105`、`src/utils/task/diskOutput.ts:304–355`、`src/tools/TaskOutputTool/TaskOutputTool.tsx:29–35,144–151` 与 Rust `src/background.rs`、`src/tool_names.rs`、`src/tools.rs`。实际补齐三项关联用户行为：

1. 新建后台 Shell 任务返回原版 `b` + 8 位小写字母数字任务 ID，任务列表、停止、完整输出、增量输出和持久化文件使用同一 ID；此前 Rust 返回 UUID。继续识别既有 UUID 记录及其日志，避免升级后历史任务不可读取。源码提交 `dbfb1b2df6d4a3f563a962cd23b8e7273b4b7d9e`。
2. 后台任务的 stdout/stderr 文件被清理或尚未产生时，仍返回任务状态及可读取的其他日志；只对文件不存在返回空字符串，权限错误与不安全符号链接继续报错。原版 `getTaskOutput` 对 ENOENT 返回空日志。源码提交 `5830739854ade9906f6917cb86f362da70364fae`。
3. 旧 SDK/工具调用的 `AgentOutputTool`、`BashOutputTool` 与 `TaskOutput` 统一映射至现有 `task_output` 执行链，保留参数标准化、后台任务查询与返回格式；别名集中在 `tool_names::canonical_name`，避免各调用点重复适配。源码提交 `6356d785f0a97dd66790489af2700bdb7fb91540`、`898c72887b77915060d24478dee08739aea53e0d`。

**未实现/待核对**：原版其他任务类型（local_agent、remote_agent、teammate、workflow、monitor、dream）的 ID 与状态机、按会话隔离的输出目录、跨进程运行任务感知、取消/恢复/通知完整时序、输出驱逐及全部工具别名的行为验收。以上三项只计本轮新增源码补齐、待验证；不升级任何冻结 Issue #3 条目。遵循用户最新指令，本轮未运行 Cargo、测试或 CI。


## 2026-10-29：TaskList 隐藏任务标记语义补齐（源码已提交、待验收）

对照原版 `src/tools/TaskListTool/TaskListTool.ts:65–83` 的 `!t.metadata?._internal` 过滤，修复 Rust `src/task_graph.rs::TaskGraph::list` 只用 `as_bool()` 判断内部标记的问题。原版 JavaScript 将非空字符串、非零数字、数组、对象也视为真，Rust 此前会把这些带有内部标记的任务显示给用户。现统一按原版 JSON 值真值语义过滤，保留对 false、0、空字符串和 null 的公开行为。提交 `69f5d83abac70bde16a3b87ebeba014d332d2524`。仍需核对任务列表全部 UI/SDK、Hook、团队与并发状态链；本项已提交待验收，不勾选冻结项。


## 2026-10-29：自定义智能体定义与 Markdown 持久文件的实际移植（待完整等价验收）

对照原版 `src/components/agents/validateAgent.ts` 和 `src/components/agents/agentFileUtils.ts` 的校验、文件格式及保存/编辑/删除逻辑，新增 `src/agent_definition.rs`。Rust 原生验证名称长度与字符、说明、系统提示词、工具名称、用户与项目重复名称；以 `.atlas/agents/*.md` 存储 YAML frontmatter、转义说明、可选模型/权限字段、只创建不覆盖与显式更新、删除及同步文件数据。

独立 `atlas-agent agent-definition --file SPEC.json [--validate-only] [--user] [--replace]` 真实命令已接通。3 项单元测试通过；本地真实 CLI 进程完成配置验证、创建和重复创建拒绝，Rust 编译和格式检查通过。保留多模型与本地用户所有权，不带原厂账号/API/遥测依赖。Agent UI 编辑器、全设置源、生成器、子 Agent 全链路仍未等价，原路径继续标为部分实现，不增加最终验收数。


## 2026-10-29：Git 工作流与更新日志真实 Rust 执行链（仍待原版全链验收）

- `src/git_workflows.rs` 对照 `src/commands/review.ts`、`commit.ts`、`commit-push-pr.ts`：固定 Git 参数读取当前状态、差异和提交历史，按仓库实际配置检测默认分支；`review`、`commit`、`commit-push-pr` 均已从 CLI 及交互式斜杠命令调用真实 Agent、模型和工具权限链。明确要求中文 PR 标题及描述。3 项单元、2 项真实 CLI→模拟 HTTP 模型集成测试通过。
- `src/release_notes.rs` 对照 `src/utils/releaseNotes.ts`、`src/commands/release-notes/release-notes.ts`：版本解析/排序、最近 5 条过滤、HTTP(S) 500ms 下载、原子缓存写入和 CLI/交互展示。2 项测试包括真实 localhost TCP 响应。固定品牌服务器 URL 已移除。
- 本地 `cargo check --all-targets`、`cargo fmt --all -- --check`、`cargo test --all-targets` 均通过：12 个测试套件，共 552 通过、0 失败。此前基线中一处厂商品牌注释导致隐私测试失败，现已由并行提交单独修复，不覆盖其改动。
- 本轮 5 条未迁移原路径调整为**部分实现**，文件级已验收数量保持 29。GitHub 与工具权限原版全面对照、自动版本更新提示、全量 UI/SDK 路径等缺口继续开放。最新 HEAD 的远端 CI 结果须单独核实。

## 2026-10-29：工具反馈、交互问答、浏览器与会话主链源码补强（本轮 5/100，待行为验收）

本轮对照完整 2.1.88 原始 ZIP 的以下 TS/JS 函数，修改可从实际 CLI/Agent 进入的 Rust 生产代码，均已提交推送。依现行开发任务要求未运行 Cargo、测试或 CI；以下仅为**源码已提交、待行为验收**，不能计入原版完整等价。

1. **工具错误长文本反馈（1 项）**：`src/utils/toolErrors.ts::formatError` 使用 JS UTF-16 `length/slice`。Rust `src/tool_errors.rs::format_error_text` 原先按 Unicode 标量数截断，导致包含表情的错误反馈长度、截断位置和数量不正确。现按 UTF-16 单元处理；若切在代理项中间，Rust 用替代字符表示（仍需协议级边界验证）。真实入口 `ToolExecutor::execute`。提交 `5b8ceb2150cecc2b5364ff906991888b679d7939`。
2. **交互式问题选项校验（1 项）**：`src/tools/AskUserQuestionTool/AskUserQuestionTool.tsx::UNIQUENESS_REFINE` 按原始字符串精确去重，Zod 未额外裁剪文本或禁止字面量 Other。Rust `src/ask_user_question.rs::validate` 原先擅自 trim 并拒绝该字面量，现恢复原版 schema 语义；`Agent::ask` 的真实交互工具入口仍使用此校验。提交 `5b8ceb2150cecc2b5364ff906991888b679d7939`。剩余交互 UI、预览格式和取消时序待完整验收。
3. **浏览器打开原始 URL（1 项）**：`src/utils/browser.ts::openBrowser` 先用 `new URL` 验证协议，再将**原始字符串**交给系统浏览器。Rust `src/browser_launcher.rs::open_browser` 先前转用规范化的 `Url::as_str()`，可能更改用户输入的 URL。现保持校验后的原始地址，真实入口 `main.rs::OpenBrowser`。提交 `d3a10503cbe00d939d04b68c4b7fd1723e8677ae`。
4. **会话重命名后列表一致性（1 项）**：`src/commands/rename/rename.ts::call` 同时调用 `saveCustomTitle` 和 `saveAgentName`。Rust `src/session_control.rs::rename` 原先只保存 custom_title，而会话列表优先显示 agent_name，导致已有代理名的会话重命名后仍显示旧名。现同时保存两个字段，并在持久化失败时一起回滚；`/rename`、CLI Rename 均走此路径。提交 `43f7995133aa1979a433a78fd0aebca8e347dfde`。
5. **语义会话搜索自定义标题与完整摘要（1 项）**：`src/utils/agenticSessionSearch.ts::agenticSessionSearch` 在显示名不同于 customTitle 时仍将后者送给检索模型，摘要不作额外 2000 字符截断。Rust `src/semantic_session_search.rs::prompt` 现补入两个原版字段语义，真实入口 `main.rs::SemanticSessions`。提交 `f476503335c096417d5a92c9a878a7ee3a7e79e3`、`c6e3f6f52479c42b50b9af99f508832ce9783c7d`。

**本轮准确进度**：5 个互不重复用户业务行为有新 Rust 代码并推送，95 项未完成；尚未形成并逐项核实本轮完整 100 项问题清单，也未完成 100 项源码补齐。无行为测试、无完整等价验收，Issue #3 冻结 362 项继续全部开放。下一轮应复用本页未闭环差异，优先推进剩余真实功能，严禁以原始文件数、纯审计或重复功能凑数。


## 2026-10-29：无交互 TodoWrite 与队员完成任务提示（源码已提交，待验收）

对照完整 2.1.88 原版 `src/tools/TodoWriteTool/TodoWriteTool.ts`、`src/utils/todo/types.ts`、`src/utils/tasks.ts::isTodoV2Enabled`，补齐 Rust `src/task.rs`、`src/session.rs`、`src/agent.rs` 的无交互任务清单主链：在普通 SDK/单次运行提供原版 `TodoWrite` 工具，验证 `content/status/activeForm`，支持整表替换、最后一项完成自动清空，保存到会话并在恢复时读取；交互模式继续使用 Task V2，`ATLAS_ENABLE_TASKS` 允许无交互显式启用 V2。代码提交 `b4b03addbf2c94fe5c42632284d5db60dd74b8a8`。

对照原版 `src/tools/TaskUpdateTool/TaskUpdateTool.ts::mapToolResultToToolResultBlockParam`，补齐队员成功完成任务后向模型反馈下一项 TaskList 检索提示；仅对真实状态从非 completed 转 completed 且处于团队队员身份时发出，不重复提醒。代码提交 `090137302528e0550003bb2e66fa0e77bbe08998`。

本轮两条独立用户业务链源码已提交，均**待行为验收**；未运行 Cargo、测试或 CI，未提升任何原路径的完整等价验收状态，Issue #3 冻结 362 项继续开放。后续需要核对无交互/交互 SDK 工具模式、原版 Todo UI 状态和复杂团队任务时序。100 项本轮硬目标仍未完成，禁止将这两项当作 100 项闭环。


## 2026-10-29：任务列表显式共享与 Doctor 可执行探测（源码已提交，待验收）

对照原版 `src/utils/tasks.ts::getTaskListId` 的“显式列表 ID → 团队名 → 会话 ID”优先级，在 Rust `src/agent.rs` 中补齐厂商中立的 `ATLAS_TASK_LIST_ID`，跨独立会话及恢复/清空/分支会话沿用指定列表。创建团队时仍重置团队专属任务存储；删除团队只清理团队目录，避免误删显式共享列表。提交 `decdb5835ffd840b33dc1fde8c2f0901bee22a6a`。团队生命周期完整时序仍需验收。

对照原版 `src/utils/which.ts` 使用 `which` / `where.exe` / `Bun.which` 查找**可执行命令**的语义，修复 `src/diagnostics.rs` 只检查 `is_file()` 的偏差：POSIX 系统必须具有执行权限，Windows 按 PATHEXT 逐个检查；Doctor 的 Git 检测及 MCP stdio 启动预检查均接入同一函数。提交 `0a5791afe65b775bd0781748bfa42db79be67d5a`。未运行 Cargo、测试或 CI，仍待行为验收。

以上为两条独立业务功能链的源码补齐；与此前同轮 TodoWrite、队员任务反馈共计四条源码补齐、待验收。Issue #3 冻结 362 项均保持开放，100 项目标未达成。


## 2026-10-29：分支会话任务清单隔离（源码已提交，待验收）

对照 `src/commands/branch/branch.ts::createFork` 生成新会话 ID、`src/tools/TodoWriteTool/TodoWriteTool.ts` 以 `getSessionId()` 索引清单、`src/utils/tasks.ts::getTaskListId` 按新会话分配 V2 列表的行为，修复 Rust `src/agent.rs` 创建会话分支时克隆旧 `Session.todos` 和兼容 `Session.tasks` 导致清单串线的问题。保留完整历史消息与原版来源信息，新分支从独立空任务清单开始，团队 V2 任务图仍按团队身份共享。提交 `ad9edbdf291d73105dd5320aded0fb0c58a225ee`。源码已提交，待行为验收；未运行 Cargo/测试/CI。


## 2026-10-29：任务文件读取失败隔离（源码已提交，待验收）

对照原版 `src/utils/tasks.ts::getTask` 对任意单条任务读取异常返回 null、`listTasks` 继续展示其余任务的行为，修复 Rust `src/task_graph.rs::TaskGraph::load` 遇到权限拒绝、I/O 异常或超过本地安全上限的单条记录时抛错中断整个 TaskList/任务认领的问题。现单条异常记录视为缺席，其余合法任务仍可查询和处理；已有锁/符号链接安全边界保留。提交 `4924e7769191d30d9be81a4bb35ea5509b21044b`。源码已提交，待行为验收，未运行 Cargo/测试/CI。


## 2026-10-29：任务创建原版高水位写入时序（源码已提交，待验收）

对照 `src/utils/tasks.ts::createTask`、`deleteTask`、`resetTaskList`：原版创建任务只在持有任务列表锁时写入任务文件并返回 ID，高水位记录在删除或重置时更新。Rust `TaskGraph::create` 先前写入任务成功后额外写 `.highwatermark`，若第二次写入失败会向模型错误报告创建失败，导致重试出现重复任务。现移除创建后的多余计数写入，仍由现存文件名和删除/重置高水位共同分配唯一编号；同步修正源码测试的预期（未执行）。提交 `9c3c87dc0e535c6dc5f684f5dd59d2b618aba1e9`。源码已提交，待行为验收。

同轮继续完善已登记的任务文件读取隔离：单条目录项异常和任务目录被并行清理时保持其他可见任务或空列表回退，提交 `f087c0283e1c91274ac6792a76c353470bbad55e`、`56b865d0293cb0355e4d84381e569a5a499e7ea9`；Windows Doctor 命令探测补齐 where.exe 当前目录优先级，提交 `4effb30fae4d2f4fdcdec0714c980a009466fdc7`。以上为原有两条业务链深化，未重复计数。
