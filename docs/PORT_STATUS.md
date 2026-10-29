## 2026-10-29：本地技能用户调用与模型权限双通道（已提交、待验收）

- **原版依据**：`source/src/skills/loadSkillsDir.ts:216–258,316–373` 为本地 `SKILL.md` 同时提供用户可调用命令及模型调用元数据；`source/src/tools/SkillTool/SkillTool.ts:405–423` 独立检查模型调用禁用标志。
- **Rust 可达链**：交互 `src/main.rs::chat` 识别 `/本地技能名` 并检查实际 `.atlas/skills/<name>/SKILL.md` → `Agent::invoke_user_skill` → `ToolExecutor::invoke_user_skill/render_skill` → 文件授权、Read 规则、`user-invocable` → 技能参数/目录/会话变量展开 → `Agent::ask`；`/skills` 通过 `Agent::list_user_skills` 只列用户可调用技能。模型 `list_skills/read_skill` 独立按 `disable-model-invocation` 过滤，隐藏技能禁止回落成普通模型文本。
- **代码提交**：`61ddae6e18a1b61eb6aee29dd804e21bee1e3613`、`7854bf27aa93aefc7d8a2f079fdb8d6bd6cda6aa`、`be77ed494f993dfa095618b8343a63b8baaa88ca`、`28ecf4cf04e8510345f189e6526bfa963bed7d25`、`0c1f62671107fb904ae951362710ae4675391321`、`b3b63dc4d4d37114de9e9fd916e82dd0120feda8`。保守计 **2 项独立用户功能**：①交互 `/skills` 按用户权限展示技能，用户专用技能不会被模型白名单过滤误隐藏；②用户直接调用本地技能斜杠命令，含模型禁止调用但允许用户调用的技能。
- **累计**：本轮 **12/100，余 88**；先行 100 项逐项核实清单尚未完成，未运行 Cargo、测试、CI，所有提交均待行为验收，Issue #3 冻结 362 项保持开放。完整用户 UI/技能自动发现/技能元数据与跨平台行为仍缺。

## 2026-10-29：技能仅用户调用标志进入真实模型权限边界（已提交、待验收）

- **原版**：`source/src/skills/loadSkillsDir.ts:216–258` 解析 `disable-model-invocation`；`source/src/tools/SkillTool/SkillTool.ts:405–423` 在模型调用前明确拒绝该技能。
- **Rust**：`src/skill_arguments.rs::frontmatter_boolean` 统一解析顶层前置布尔字段；`src/tools.rs::ToolExecutor::list_skills` 排除禁止模型调用的本地/插件技能，`ToolExecutor::read_skill` 在读取实际内容后再次拒绝，避免模型绕过列表直接指定名称；用户侧插件斜杠技能仍按 `user-invocable` 单独控制。原版两个开关分别生效。
- **提交**：`3913fa608d9a4d0a9423343acd6cd4c0c7ba24d4`、`299599fdcf7cca8d62bb3fe5105fd438d55de29d`、`cb2e0ace11fd695768085b935922bd40be937808`。保守新增 **1 项独立用户业务权限功能**，本轮累计 **10/100，剩余 90**。未运行 Cargo、测试或 CI，已提交、待验收；Issue #3 冻结 362 项继续开放，先行 100 项完整问题清单仍未建立。

## 2026-10-29：插件技能斜杠调用与命令目录接线（已提交、待验收）

- **原版依据**：`source/src/utils/plugins/loadPluginCommands.ts:169–211,685–820` 把插件技能目录中的 `SKILL.md` 转成用户可调用命令，使用技能相对命名空间和 `user-invocable`；`source/src/skills/loadSkillsDir.ts:344–373` 提供技能目录和会话变量。
- **Rust 可达链**：`src/main.rs::chat` 解析 `/插件:组:技能` → `plugins::command_prompt_with_session` → `manifest.skills` → `skill_files` → 真实 `SKILL.md` → frontmatter 可调用校验、参数替换、技能目录前缀及会话变量 → `Agent::ask`；`plugins::list_commands` 同时列举可执行插件技能，并过滤 `user-invocable: false` 的模型专用技能。
- **真实提交**：`545e01caa794d670ff766ae831709bfe0acc41eb`、`e4043c4b5cf495f71a98cb9a2cefad2335513b57`。新增 **2 项用户可感知功能**：①用户直接运行插件技能斜杠命令；②`/commands` 显示可调用技能且隐藏模型专用技能。原版 SkillTool 更完整的权限和 SDK/无头路径仍缺，已提交、待验收。
- **本轮累计**：前述插件命令 3 项、插件技能/数据目录 4 项、本节 2 项，共 **9/100，余 91**。先行 100 项独立问题清单仍未完整确认；未运行 Cargo、测试或 CI；Issue #3 冻结 362 项不变。

## 2026-10-29：插件技能自定义路径真实接线及数据目录（已提交、待验收）

- **原版完整源码**：`source/src/utils/plugins/loadPluginCommands.ts:60–95,288–313,326–376,685–820`、`source/src/skills/loadSkillsDir.ts:344–373`、`source/src/tools/SkillTool/SkillTool.ts:1070–1086`、`source/src/utils/plugins/pluginDirectories.ts::pluginDataDirPath/getPluginDataDir`。原版从清单中的技能目录查找真实文件，支持技能路径、会话 ID、插件根目录及按需创建的持久数据目录变量。
- **Rust 真实调用链**：`ToolExecutor::list_skills/read_skill` → `plugins::resolve_skill_path` → `manifest.skills` 指定目录/嵌套 `SKILL.md` → `workspace_dirs::checked_path` 和 Read 权限规则 → 实际文件读取及技能本地路径/插件根路径/会话 ID 展开。插件命令 `src/main.rs::chat` → `plugins::command_prompt_with_session` → `plugin_data_directory`，只有真正执行且引用 `${CLAUDE_PLUGIN_DATA}` 时才创建工作区 `.atlas/plugin-data/<插件名>`，防止符号链接逃逸；列举命令时不产生数据目录副作用。
- **本次新增 Rust 提交**：`7af75f97ead51637caa167d32800d6ab104a7aa2`、`f3e7bdf8f67a4903d052ce468ac3352e36fe7f2a`、`a4c00e2dbc96aa37fd940dbbaf4a3b7032f1fe49`、`7aeb692c8266ca31e1ab0be4b56f6a47bbb08bc4`。新增 **4 项独立业务功能**：①插件自定义/嵌套技能在模型工具目录中实际可见；②模型可从清单真实位置读取该技能并执行权限检查；③技能内容展开原版技能路径/插件根路径/会话变量；④插件命令按需建立可持久保存的插件数据目录。与下方插件命令的 3 项不重复计数，**截至该小节累计 7/100；加上上方插件技能斜杠调用 2 项，本轮累计 9/100，余 91**。
- **差异和限制**：SDK/无头插件命令调用、插件 user-config 敏感值遮蔽、内嵌 Shell/权限、插件热更新与异步取消、完整技能模型/effort 元数据、跨平台权限和交互 UI 尚缺。已静态核对真实源码入口和提交文件，**未运行 Cargo/测试/CI**，均为“已提交、待验收”，Issue #3 冻结 362 项保持开放。先行 100 项经逐项确认的缺口清单仍未完成，流程 A/B 与本轮数量硬目标未达标。

## 2026-10-29：插件命令用户可调用边界与运行时变量（源码已提交、待验收）

- **原版逐函数依据**：完整用户 ZIP 的 `source/src/utils/plugins/loadPluginCommands.ts:288–313,326–376`：`user-invocable` 由 frontmatter 决定用户能否直接调用；命令执行时展开 `${CLAUDE_PLUGIN_ROOT}` 和 `${CLAUDE_SESSION_ID}`；`source/src/utils/plugins/pluginOptionsStorage.ts::substitutePluginVariables` 负责插件根路径替换。
- **Rust 真实调用链**：交互 CLI `src/main.rs::chat` 解析 `/plugin:command` → `plugins::command_prompt_with_session` → `src/plugins.rs::plugin_command_body` → frontmatter 用户可调用检查、参数展开、插件规范化根路径与当前 Agent 会话 ID 展开 → `Agent::ask`。不可由用户调用的命令被显式拒绝，不会回退成普通模型提问；`/commands` 的发现仍过滤隐藏命令。
- **本轮实际 Rust 提交**：`2bd844132dc791f7e01ef32918d616368a1d209f`、`86909a567d1f08d32b1af725805c791a4182986a`、`b1e64546dd35eeadf620f0ee22433a884f18a947`、`e0e67d017bea4fb33118c203b8039f359e26c026`、`5a565f304ce5416710c956aefa9c4a9369b6c889`。保守计 **3 项独立用户可感知功能**：①隐藏插件命令禁止用户调用；②插件根目录变量在实际命令提示中展开；③当前会话 ID 在交互调用时展开。多个修正提交不重复计数。
- **静态核查与限制**：已核对源文件、Rust 插件解析与交互 Agent 入口，未运行 Cargo、单元/集成/端到端测试及 CI；均为**已提交、待验收**，Issue #3 冻结 362 项不勾选。仍缺敏感用户配置占位符、Shell 内嵌执行及其权限、插件命令模型/effort 元数据、SDK/无头命令调用、动态刷新与取消传播。
- **本轮目标状态**：100 项先行逐项确认的缺口清单尚未形成；截至该小节的代码补齐 **3/100**；结合上方技能及数据目录新增 4 项，**本轮累计 7/100，余 93**。流程 A/B 与本轮 100 项硬目标未完成，不能把静态源码审阅当成行为等价验收。

## 2026-10-29：SDK 会话标题生成、独立保存及历史显示（已提交、待验收）

- **原版核对**：`source/src/cli/print.ts:3783–3814` 的 `generate_session_title` 控制请求；`source/src/utils/sessionTitle.ts:56–129` 的句式标题 JSON 生成及空描述/失败返回；`source/src/utils/sessionStorage.ts:2657–2673` 的 `ai-title` 与显式重命名隔离。
- **Rust 可达业务链**：SDK NDJSON `control_request` → `src/main.rs::sdk_nested_control(generate_session_title)` → `src/agent.rs::generate_session_title` → 当前配置的通用模型 `ModelProvider::complete` → JSON `title` 校验、用量记账 → `src/session.rs::ai_title` 独立持久化（仅 `persist=true`）→ `src/session_control.rs::list` 在无显式用户标题时展示生成标题。空描述返回 `null`，无效模型 JSON 不写标题，存储失败回滚内存状态并返回错误；不向主对话追加伪造用户消息。
- **代码提交**：`f2b7d484db465505b82ed7875416a8e74021911a`，4 个真实 Rust 运行模块，保守计 **1 项独立用户可感知业务功能**。静态核对入口、模型请求、持久化字段、历史读取及提交文件；未运行 Cargo/CI/行为测试，**已提交、待验收**。
- **准确剩余差异**：原版该请求异步执行且不阻塞 stdin/interrupt；当前 Rust SDK 仍串行等待模型标题生成。原版取消信号传播、远程端标题回写、完整标题元数据索引、跨进程并发及标题竞态还需补齐。冻结 Issue #3 的 362 项均继续开放。
- **本轮严格目标**：100 项先行独立业务缺口清单尚未逐项核验；本次实际代码补齐 **1/100，余 99**，未达标，不将台账文字计为业务功能。

## 2026-10-29：Glob 绝对路径通配符跨授权目录搜索（源码已提交、待验收）

- **原版源码**：`source/src/utils/glob.ts:12–84::extractGlobBaseDirectory/glob`。原版绝对通配符从首个特殊字符前的静态目录开始遍历，支持不显式传入 `path` 的绝对文件名/路径模式。
- **Rust 真实入口**：Agent `glob_files(pattern="/authorized/other/**/*.rs")` → `ToolExecutor::dispatch` → `search::absolute_glob_base` 取得真实静态根 → `checked_path` 核对主工作区或显式授权附加目录 → `require_read_rules` 审批实际搜索根 → `glob_files_filtered` 逐目录/文件执行 Read deny 排除 → 返回文件列表。避免原实现将所有绝对模式强行限定在默认 `path` 下而误拒绝已授权目录。
- **代码提交**：`814bb0d32978eb6b3eb0ce6cd8008e752246838c`、`33dc985b8dbb029f4d4f39f52b2c565da44cc356`。保守新增 **1 条独立用户可感知功能**；本次累计 **3/100，剩余 97**。未运行 Cargo、测试或 CI；已提交、待行为验收，完整等价验收 0，Issue #3 冻结 362 项继续开放。
- **剩余差异**：Windows 驱动器/UNC 特殊模式、通配符内非规范路径、插件缓存排除、动态结果上限、取消与超时、Glob UI 展示仍待源码与行为验收；100 条前置完整缺口清单尚未建立。

## 2026-10-29：Glob/Grep 搜索逐文件读取权限与错误传播（源码已提交、待验收）

- **原版完整源码**：`source/src/tools/GrepTool/GrepTool.ts:411–435` 在 ripgrep 搜索前加入 `getFileReadIgnorePatterns` 生成的排除规则；`source/src/utils/glob.ts:86–117` 对文件名搜索应用相同 Read 排除；`source/src/utils/permissions/filesystem.ts:800–851` 汇总并按搜索目录转换 Read deny 规则；`source/src/utils/ripgrep.ts:382–388` 对 EACCES/EPERM 关键读取错误显式失败。
- **真实 Rust 业务链**：Agent 模型 `glob_files` / `grep_files` → `src/tools.rs::ToolExecutor::dispatch` 根目录检查 → `src/search.rs::glob_files_filtered/grep_files_filtered` → 每个候选文件 `ToolExecutor::search_read_allowed` → `read_rule_decision` / `permission_rules::policy_for` → 搜索结果或错误。根目录可读不再意味着其内部所有文件都可被模型搜索；路径拒绝规则及解析后的目标在逐文件阶段生效；Read ask 规则仍在搜索根目录审批阶段处理，避免错误隐藏已获准的搜索结果。Grep 的内容、计数、文件名三种模式都在打开文件前过滤；被 Read deny 命中的目录在遍历前被裁剪，避免进入被禁止的子树；文件消失继续处理，其他读取失败返回可见错误。
- **实际 Rust 提交**：`de28f3ed702e458e4c1d40b41bdac3dd257574f3`（遍历前排除被拒绝目录）、`8c6e47ed35bb0816e0b07425c1b9e9b66d58723a`（按原版修正 ask 规则边界）、`93fa1d9bdde6377a391c7bcf7775dd41a149dd6e`（搜索过滤入口）、`7c5a8dde22ae4142e97fd8625e8a64c6904438fd`（Agent 工具实际接线）、`e039a06f53306e09dc321f0f3cef350337a07675`（读取错误传播）。保守计 **2 项不同用户可感知业务功能**：① Glob 文件名发现遵守 Read 拒绝规则；② Grep 内容/路径/计数检索遵守 Read 拒绝规则并正确上报读取错误。多处内部修复不额外计数。源码已提交、待行为验收，新增完整等价验收 **0**。
- **静态核对**：从最终远端 HEAD 读取 `src/search.rs`、`src/tools.rs`，确认两个过滤函数、候选文件检查和 Agent 实际调用均存在；遵照要求未运行 Cargo、测试或 CI。PR #2 保持 Draft，Issue #3 冻结 362 项保持开放。
- **未实现差异**：原版 Read 排除模式的全部根目录转换、完整 ripgrep 的 `.ignore` / `.rgignore` / 嵌套忽略优先级、插件缓存排除、取消/超时、并发文件变化、跨平台路径及部分输出语义尚未等价；100 条独立功能缺口前置清单未完成逐函数确认，本次 **2/100，剩余 98**，不宣称达标。

## 2026-10-29：插件 MCP 配置兼容与独立加载补强（源码已提交、待验收）

- 原版对照：`source/src/utils/plugins/mcpPluginIntegration.ts::loadMcpServersFromFile/loadPluginMcpServers/resolvePluginMcpEnvironment/extractMcpServersFromPlugins`（默认 `.mcp.json` 可选、逐服务 schema 验证、插件根/数据目录变量、环境变量展开及独立失败）；`source/src/utils/plugins/pluginOptionsStorage.ts::substitutePluginVariables`、`source/src/services/mcp/envExpansion.ts::expandEnvVarsInString`。
- Rust 真实入口：`/plugin install` / `/plugin enable` → `src/plugin_manager.rs::install_local/execute` → `src/plugins.rs::manifest_servers/PluginSet::load/resolve_plugin_mcp_server` → `src/agent.rs::attach_mcp` → `src/mcp.rs` / `src/mcp_http.rs`。
- ① 插件 MCP stdio 命令、参数、环境及 HTTP URL/Headers 现在解析插件专属 ROOT/DATA 占位符与普通环境变量；stdio 子进程获得独立插件目录与数据目录环境变量。② 一条无效 MCP 服务 JSON 不再阻断同一插件的其他有效服务，并按原版配置 Schema 拒绝空 stdio 命令及缺失远程 URL。③ 清单引用的可选 MCP JSON 文件缺失或 JSON 损坏时跳过该文件，其余来源继续合并；已有文件仍需通过插件根目录边界检查。
- ④ 清单数组中某个 MCP 配置来源失败时，以原版逐来源容错语义保留其他有效来源，并按原声明顺序覆盖；失败来源不会留下部分合并状态。⑤ 移除原版没有的插件 MCP 服务名 64 字节上限，避免长名称导致整插件无法启用。
- 本轮代码提交：`27f43a8827be58f4155475f1788662759024d4bd`、`b4f50406e6cb4d1dc0543350e5923dd965db4a68`、`128bf3fa05b3b079e087d1a5670178376236c428`、`4fd1ceb4a80f7834b127b40e475b5496ba21f839`、`4bfd7a5421af9cd4cc41227de938bb7835c9a4e8`、`5f686a25782098f3799f2a00df3cd729d6271ee6`。**按不同用户可感知业务行为保守登记 5 项源码补强，全部待行为验收；完整等价验收新增 0。** 依用户要求未运行 Cargo、测试或 CI，Issue #3 冻结 362 项保持开放。
- 尚存差异：`user_config` 持久化及敏感值读取、MCPB 安装、服务能力 schema 完整验证、服务加载失败在交互 UI 的逐项错误展示、跨插件/市场作用域、数据目录卸载策略、异步重连及取消等。100 项完整前置缺口清单尚未建立，当前本轮仅可确认上述 5 项实际源码修改，**5/100，剩余 95 项**。

## 2026-10-29：插件原版标准清单、无清单安装、Hooks/MCP 真实加载与本地安装安全（源码已提交、待验收）

- **原版源码**：完整用户 ZIP `source/src/utils/plugins/pluginLoader.ts::loadPluginManifest`（1147–1213 行）、`createPluginFromPath`（1310–1386、1613–1759 行）、`installFromLocal`（856–868 行），以及 `pluginInstallationHelpers.ts::installResolvedPlugin`。
- **Rust 真实入口**：`/plugin install` / `/plugin validate` / `/plugin manage` → `src/plugin_manager.rs::manifest_name/install_local/installed` → `src/plugins.rs::validate_local/read_manifest/PluginSet::load`；Agent 的插件初始化继续通过 `PluginSet::load` 将 Hook 与 MCP 服务接入会话。
- **实际补齐**：①读取标准 `.claude-plugin/plugin.json`，兼容已有根目录 `plugin.json`，允许仅有 commands/agents/skills 的无清单插件以目录名安装、验证、启用与列举；②按原版读取 `hooks/hooks.json`、清单内追加 Hook 文件和内联命令 Hook，保留事件、匹配器、超时、插件根目录与重复文件去重，越界文件拒绝；③原版 `mcpServers` 名称映射转换成真实 `McpServerConfig` 并继续进入 Agent 的 MCP 初始化；④本地插件复制时排除源仓库顶层 `.git`，避免把仓库历史、远端地址与本地 Git Hooks 放入安装目录。
- **同功能接线复查**：`88639a1`、`b0cd032` 修正 Hook 零超时按原版使用默认值；`5971d19` 将条件和 Shell 类型接入实际 HookRule，未支持的 async/once 明确拒绝，防止悄悄改变生命周期语义。这些均归入上述 Hook 功能，不重复计数。
- **提交 SHA**：`d8d673a11cec0430919b837bc45003d46500f06f`、`c554ea70633e99d3f2456df64722dd18ca8ec685`、`6c17c1c5784846f613afe82021d44697ff92640a`、`84f2a6e2996e19feba30b56995951585e007481a`。
- **保守计数**：4 条独立用户可感知插件业务链获得真实代码补齐，全部**已提交、待验收**；本轮完整 100 条前置缺口清单尚未建立，初始四项 **4/100**，同轮续补后累计 **7/100，剩余 93**。没有运行 Cargo、测试或 CI；没有勾选 Issue #3 的任何冻结项。
- **未实现差异**：原版插件命令/代理/技能自定义路径、内联命令、HTTP/异步/提示词 Hook、完整 MCP 配置文件引用及 SDK 传输、插件设置、市场多范围安装、插件生命周期与并发回滚、所有 UI/SDK 交互仍待补齐；无清单插件的命令和代理还需要逐入口核对实际可达性。

### 同轮续补：插件 MCP 标准文件及清单路径引用

原版 `source/src/utils/plugins/mcpPluginIntegration.ts::loadPluginMcpServers`（131–211 行）先加载插件目录 `.mcp.json`，再按清单 `mcpServers` 的文件路径、路径数组、内联对象覆盖同名服务。Rust `src/plugins.rs::manifest_servers/merge_server_specs/mcp_file` 已接通 `PluginSet::load` 的真实会话初始化；新增工作区内文件规范化与循环引用拒绝，避免非法插件配置递归故障。提交 `df08c7ac2f41a5e84179dc29013cc3251c17565f`、`03e4587d723343e7863b47a70e9c476330610dc6`、`328cc963d7de75ce698a0466a492369224fa77c4`。此为额外 **1 项独立用户可感知业务功能**，本轮当时累计 **5/100**，继续补齐技能路径后为 **6/100，剩余 94**，源码已提交、待验收。MCPB 安装、完整环境变量展开、无效单服务的原版局部容错、异步恢复与跨插件冲突处理仍未等价；Issue #3 冻结 362 项保持开放。未运行 Cargo、测试或 CI。

### 同轮续补：插件自定义技能目录与读取

原版 `source/src/utils/plugins/pluginLoader.ts::createPluginFromPath`（1557–1583 行）在清单声明 `skills` 时读取自定义路径，未声明时扫描默认 `skills/`。Rust `src/plugins.rs::skill_roots/skill_files/list_skills/read_skill` 现支持相对目录或数组、直接技能目录与目录下多个技能，列表和读取共用同一解析与插件根目录安全边界，安装前 `validate_local` 也检查路径。实际可达入口为 Agent 的技能列表及读取工具。提交 `687d8850684cd2bb1ada1b1028a289344541a530`、`0b146e44253d56286f0f8c8ef9337ef0adbc4436`。 续补 `2075fb59c84abfbfeb9f2bc692e08b22caf52351`：按原版 `loadPluginCommands.ts::getCommandNameFromFile` 的层级规则递归发现嵌套 `SKILL.md`，生成冒号分隔命名空间并在 `read_skill` 复用，属于同一技能路径功能完善，**不重复计数**。**另计 1 条用户可感知业务功能，累计 6/100，剩余 94**；源码已提交、待验收。嵌套技能命名空间、全部技能元数据与 SDK/插件动态刷新仍缺；冻结 362 项不勾选。未运行 Cargo、测试或 CI。

### 同轮续补：插件 Markdown 斜杠命令真正进入模型执行链

原版 `source/src/utils/plugins/loadPluginCommands.ts::collectMarkdownFiles/getCommandNameFromFile/createPluginCommand`（50–213 行）与 `source/src/utils/argumentSubstitution.ts::substituteArguments`（94–144 行）。Rust `src/plugins.rs::command_prompt` 从默认 `commands/` 或清单自定义路径、内联命令/文件元数据中读取 Markdown，解析命名空间、去除 YAML 头、使用现有 `skill_arguments` 的命名/索引/`$ARGUMENTS` 参数替换；`src/main.rs` 的交互式 `/plugin:command` 或 `/plugin:namespace:command` 入口实际调用 `Agent::ask`。提交 `7e2c295fa1ec19b27d5eeb09d5d15134143285be`、`a6e148a31ef70e33f61766e6e9affbe7e06866fe`。 继续提交 `848b90da`、`dfc0dca9`、`3f3f10ec`：`plugins::list_commands` 与交互式 `/commands` 列表接线，逐条复用 `command_prompt` 确认可实际执行，目录错误显式反馈；属于同一插件命令功能链的发现与展示补全，**不重复计数**。**另计 1 项用户可感知业务功能，累计 7/100，剩余 93**；源码已提交、待验收。原版完整 frontmatter 元数据、Shell 内嵌命令执行、动态命令发现、SDK 命令列表、插件变量与命令禁用策略仍未等价；Issue #3 冻结 362 项不勾选。未运行 Cargo、测试或 CI。

## 2026-10-29：SDK 动态 MCP 服务配置更新进入真实执行链（源码已提交、待验收）

- **原版完整源码**：`source/src/cli/print.ts::applyMcpServerChanges` 与 `mcp_set_servers` 控制分支；`source/src/cli/print.ts::handleMcpSetServers/reconcileMcpServers`；`source/src/services/mcp/config.ts::filterMcpServersByPolicy`。原版支持按 SDK 期望配置增删、更换进程/HTTP 服务，清除旧工具路由并保留无变化服务，按策略拒绝非法注入，逐服务返回错误。
- **Rust 实际代码**：`src/main.rs::sdk_nested_control("mcp_set_servers")` → `src/agent.rs::set_dynamic_mcp_servers` → `src/mcp_policy.rs::allows` / `Config::validate` → `src/mcp.rs::McpManager::reconcile_dynamic` → 真实连接/断开、工具定义更新、MCP 状态与错误回传；Agent 子代理和团队成员继承当前动态服务配置。重配仅触及有差异的动态服务，保留静态服务及未改变的连接；单服务失败写入可见状态，配置和策略错误逐项返回。新进程服务只在当前会话生效。
- **提交**：`91bb6189`、`3524b306`、`9a402ee4`、`046bbd4e`、`cb8355a3`、`b6351fa9`、`83e6f124`、`56883624`、`5cbcd324`、`a0716f3e`、`63a53430`。已静态审查 SDK 入口→配置校验→策略→增删/重连→工具路由→子代理继承，未运行 Cargo、CI 或行为测试。保守计 **1 项独立用户可感知业务功能：SDK 会话内动态配置 MCP 服务**，已提交、待验收，新增完整等价验收 0。
- **尚存差异**：原版 SDK 内进程 `type:sdk` 传输回调、SSE 传输、OAuth 交互、原版完整策略来源/匹配边界、动态服务与插件后台安装的并发合并、所有工具和资源订阅恢复及异常时序尚未全等；对不支持的 SDK 传输明确返回错误。此段与同轮文件编码读写补强合计 **2/100，剩余 98 项**；100 条前置缺口清单仍未完整核验，不宣称本轮达标。Issue #3 冻结 362 项继续开放。

## 2026-10-29：文件读取、编辑及 SDK 读取状态的原版编码与换行处理（源码已提交、待验收）

- **原版完整源码依据**：`source/src/utils/fileRead.ts::readFileSyncWithMetadata/detectLineEndingsForString`；`source/src/tools/FileEditTool/FileEditTool.ts::readFileForEdit`；`source/src/tools/FileWriteTool/FileWriteTool.ts::call`；`source/src/cli/print.ts` 的 `seed_read_state`。原版按 UTF-16LE BOM 识别编码、读取时规范化 CRLF、编辑时恢复原有换行风格；完整覆盖写入遵从模型明确指定的换行内容并保留原文件编码。
- **已落地 Rust 真实执行链**：`src/tools.rs::read_editable_text/write_editable_text` 接入 `ToolExecutor::read_file_with_mode/seed_read_state/write_file/edit_file`，贯通 Agent 的 Read/Edit/Write 和 SDK `seed_read_state`。补齐 UTF-16LE、UTF-8 BOM 识别，读取缓存与磁盘编辑内容统一规范化，编辑保留原有编码及 CRLF/LF 风格，完整覆盖写入保留用户指定的换行；无效 UTF-16 字节序列显式报错。
- **提交**：`3e8d982c9eab6dc13b0c1ef9e2b6523adf64e93b`、`2931b1bc527a88aa01df35460d063a104becd362`。已静态检查读取→缓存→编辑比较→落盘的源码接线与修订点；遵照本任务要求未运行 Cargo、测试或 CI。保守计 **1 条独立用户可感知业务链：多编码文件的安全读取与编辑**，已提交、待行为验收，完整等价验收新增 0。
- **仍有差异**：原版所有编码探测分支、UTF-16BE、写入原子性与并发文件变更竞态、LSP 编辑通知、原版精确文件大小/编码限制和跨平台换行边界仍待核对。前置 100 条完整独立缺口清单尚未完成核验，本轮 **1/100，尚差 99 项**；不得把本次多入口修改重复计数。Issue #3 冻结 362 项继续开放。

## 2026-10-29：插件技能发现与实际读取的路径一致性（源码已提交、待验收）

原版参考 `source/src/utils/plugins/pluginLoader.ts` 与 `walkPluginMarkdown.ts` 的插件技能发现流程。Rust `src/plugins.rs::list_skills` 原先只检查技能真实路径是否仍在整个工作区，允许列出指向另一个插件的符号链接技能，而 `read_skill` 会拒绝读取。现发现阶段验证 `skills` 目录及每个 `SKILL.md` 均在当前插件根目录，和读取入口保持同一授权边界。提交 `e45e4187725d7b2de90f365a96bf924615079487`、`5b52f8eaffc729008b1354e8205944da860ad9d8`。

本项计 **1 条用户可感知业务链：插件技能列表与读取的安全一致性**；本轮累计 **10/100**。未运行 Cargo、测试或 CI；源码已提交、待行为验收，新增完整等价验收 0。插件全量目录、动态启停、技能参数、权限与跨插件引用仍有差异，Issue #3 冻结 362 项继续开放。

## 2026-10-29：SDK MCP 服务启停与初始化真实能力发现（已提交、待验收）

- 追加贯通交互式 `/mcp enable SERVER_NAME`、`/mcp disable SERVER_NAME`，与 SDK 共用同一持久化与实时路由清理逻辑；再次启用时检查 stdio 子进程是否已退出并独立重连，提交 `b32c18a7d87263f28a662bffb0f1cb9518ca3dd8`。该接线归入前述 MCP 启停功能，不重复计数。


- 原版依据：完整 2.1.88 `source/src/cli/print.ts:3206–3296` 的 `mcp_toggle` 持久禁用、断开并清理工具/提示词/资源；`source/src/cli/print.ts:4336–4495` 的 `handleInitializeRequest` 返回真实可调用命令与自定义代理目录。
- 已补齐的三个独立用户业务行为：① SDK 通过 `mcp_toggle` 按服务名启停 stdio/HTTP MCP，禁用状态跨会话保存，实时清理路由与资源订阅，禁止目录查询和自动重连偷偷重启禁用服务，重新启用时独立重连并恢复工具；② SDK 初始化返回已接入交互 CLI 的真实命令及说明，不暴露尚未迁移的静态注册命令；③ SDK 初始化读取项目/用户磁盘自定义代理定义的名称、描述和模型，项目同名优先，跳过无效元数据，不返回代理系统提示词。
- Rust 实际接线：`src/main.rs::sdk_nested_control` → `src/agent.rs::toggle_named_mcp/sdk_agent_descriptors` → `src/mcp.rs::McpManager::toggle_named/connect_with_root_filtered`、`src/agent_definition.rs::existing_agent_types`、`src/command_manifest.rs::visible`。代码提交 `6dbe7e97b7ed7fd0ff4aed9c2e02d183c1cd9171`、`a24b095ea653815f4332c93a5bb950c24ba0e47f`、`eb248f8674555d2a524dd17d8bdc2cf883fa7575`。
- 源码级检查：SDK 请求名/响应封装、磁盘配置保存与加载、禁用后工具路由清理、其他服务索引更新、失败后重连、项目与用户代理优先级、命令对应真实交互入口均已核对。按本任务要求没有运行 Cargo、测试或 CI；**3 项已提交、待行为验收，最终等价验收 0**。
- 未实现差异：SDK 动态 `mcp_set_servers`、`reload_plugins`、`rewind_files`、`seed_read_state`、`side_question`、`generate_session_title` 等控制链；完整 YAML 代理元数据、多来源代理动态加载、MCP 跨进程开关一致性、HTTP 会话显式终止、SDK 并发中断。Issue #3 冻结 362 项全部保持开放。
- 本轮 100 项前置独立缺口清单尚未逐项核实；仅按上述 3 项已提交的真实业务功能计数，**3/100，剩余 97 项**，不将文件数、静态注册或辅助修复另计。

## 2026-10-29：插件市场注册表与锁文件路径安全（源码已提交、待验收）

对照原版 `source/src/utils/plugins/pluginInstallationHelpers.ts::checkPathTraversal` 及 `source/src/utils/plugins/marketplaceManager.ts` 的市场存储/注册行为，复核 Rust `src/marketplace_registry.rs::exclusive_lock/load/save/execute/resolve_local_plugin`。原实现直接打开 `.atlas/marketplaces.lock` 和 `.atlas/marketplaces.json`，当工作区存储目录或文件被符号链接重定向时可能读取或写入外部位置。现拒绝 `.atlas`、锁文件和注册表的符号链接，确认规范化存储目录位于工作区内，锁文件保持非截断打开，所有现有市场操作继续共用同一安全门禁。代码 `b9027b6d25c95fe5cddde0f0925334725fd9905c`、`80e3be9439ba001a619df08c7cb0e6ee4c99bd1d`。

本项保守计 **1 条用户可感知业务链：本地市场注册与更新的存储安全**；本轮累计 **9/100**。该源路径只标记部分实现，远程下载、版本与依赖处理、跨进程一致性和完整市场管理仍未等价。按要求未运行 Cargo、测试或 CI，源码已提交、待行为验收；Issue #3 冻结 362 项不勾选，新增完整等价验收 0。

## 2026-10-29：本地插件安装存储边界修复（源码已提交、待验收）

原版依据：`source/src/utils/plugins/pluginInstallationHelpers.ts:79–106` 的安装路径越界检查。Rust `src/plugin_manager.rs::install_local/installed/execute(Uninstall)` 原先可能在插件加载校验前跟随 `.atlas` 或 `.atlas/plugins` 符号链接写出工作区。新增 `checked_installed_root`，安装、卸载、列举前拒绝重定向存储根并检查规范化路径；插件列表保持只读，不额外创建目录。代码 `2dd6c0b0a7ba279b125610926f6b5118b02133bd`、`14b097fe988661ac442bb390028e51c26ca32563`。

本项计 **1 条用户可感知业务链：插件安装与卸载的存储边界保护**；本轮累计 **8/100**。未运行 Cargo、测试或 CI，源码已提交、待行为验收，新增完整等价验收 0。插件市场、全量验证、版本与多范围安装仍缺，Issue #3 冻结 362 项继续开放。

## 2026-10-29：Shell 毫秒超时、任务停止与输出的真实执行门禁（源码已提交、待验收）

- 原版源码：`source/src/tools/BashTool/BashTool.tsx:227–247,857–882` 的毫秒 timeout 与 semanticNumber；`TaskStopTool/TaskStopTool.ts:43–105` 的状态预校验；`TaskOutputTool/TaskOutputTool.tsx` 的严格参数与 semanticBoolean。
- **后台任务停止**：`src/background.rs::validate_stop` 先读取实时任务状态；`src/tools.rs::validate_task_stop` 统一 task_id/shell_id、未知字段与任务状态；`src/agent.rs` 在 PreToolUse 前拒绝非法停止请求，授权后 `stop` 再检查竞态。代码 `9008bf39b12b2f1908d1fb444dc10a16fe4acc94`、`9b0a7fef2a9e1846e00a2188db758f69d9ab37f5`。
- **后台任务输出**：`ToolExecutor::validate_task_output` 同时用于模型工具调用前置校验与真实执行分发，保留 `block` 语义布尔转换、默认 30 秒和 0–600000 毫秒范围，非法字段不触发 Hook。代码 `0bb6043f8829e2f728b63785ae157fc2df350c61`。
- **Shell 毫秒超时**：`src/tools.rs::run_command_with_shell` 支持原版 Bash/PowerShell 的毫秒 timeout 与通用 timeout_seconds，执行超时改用毫秒精度；`src/semantic_inputs.rs` 处理原版 semanticNumber 数字字符串。代码 `6de19f6a211eab3b11a83cf5ea5656cf7daf3624`。
- **Shell 输入安全门禁**：`ToolExecutor::validate_shell_input` 对命令、描述、后台标志和超时字段执行无副作用检查，接入 Agent 的 PreToolUse 前置入口与执行器，非法模型参数不触发 Hook 或 Shell 进程。代码 `105373ea261e99c4b4a8891121f0b715cf12c9e5`。
- 本段保守计 **4 项独立用户可感知业务行为补强**，与前段任务图 3 项合计 **7 项**。未运行 Cargo、单元/集成测试或 CI；均为**已提交、待行为验收**，没有新增等价验收路径。Issue #3 冻结 362 项保持开放。仍缺多类型任务统一停止、跨进程状态、完整 Bash 自动后台化/沙箱与 Hook 全协议；本轮 100 项硬目标尚未达到。

## 2026-10-29：任务依赖持久化与任务 Hook 参数安全门禁（源码已提交、待验收）

- 原版依据：`source/src/utils/tasks.ts::blockTask`（458–486 行）双向维护 `blocks` / `blockedBy`；`source/src/tools/TaskCreateTool/TaskCreateTool.ts`、`TaskGetTool.ts`、`TaskListTool.ts`、`TaskUpdateTool.ts` 的 Zod strictObject 输入校验发生在工具调用和生命周期 Hook 之前。
- `src/task_graph.rs::link` 修复任务自依赖时两份旧快照相继写入导致 `blocks` 丢失，确保同一条任务记录双向关系完整落盘，TaskGet/TaskList/认领阻塞可继续读取；提交 `4f4cf7b1dab280ddea60853c0f91137f20162f0b`。
- `src/task_graph.rs::validate_update_input` 和 `src/agent.rs::run_task_graph` 在 TaskCompleted Hook 前完整校验字段；模型调用进入 PreToolUse 前同样校验，防止非法状态、元数据或依赖参数先触发 Hook 副作用再报错；提交 `809afbda1ef8c7a913db84819c3a0290a2c539d7`。
- 将同一严格输入校验扩展至 TaskCreate、TaskGet、TaskList，非法创建/读取/列表请求不进入 PreToolUse Hook，合法请求仍走原业务链；提交 `06c65cc05913a63a94619b6d6103b225651d77d2`。
- 本轮保守计为 **3 条独立用户可感知业务行为补强**：任务自依赖关系持久化、任务更新前的 Hook 安全校验、任务创建/读取/列表的统一参数安全校验。按用户要求仅做源码接线审阅，不运行 Cargo、测试或 CI；均标为**已提交、待行为验收**，完整等价验收新增 0。Issue #3 冻结 362 项全部继续开放。
- 尚缺任务系统跨进程通知、原版全状态迁移与并发细节、TaskCompleted/TaskCreated Hook 全协议和 UI/SDK 结构化输出；本轮 100 项预定清单尚未全部完成，不能按功能数宣称达标。

## 2026-10-29：无工具 MCP 服务与资源/提示词完整目录（源码已提交，待验收）

原版依据：`source/src/services/mcp/client.ts::fetchToolsForClient`（1743–1757 行）在服务未声明 `capabilities.tools` 时返回空工具列表，仍保留连接；`fetchResourcesForClient`（2000–2027 行）和 `fetchCommandsForClient`（2033–2046 行）分别按资源/提示词能力读取。Rust `src/mcp.rs::McpManager::connect_with_root` 以前无条件请求 `tools/list`，使仅提供资源或提示词的合法服务被误判连接失败。提交 `ded394f83f661c8c9b6ec61da1ce2745655cfc2b` 让此类服务正常连接，并继续使用现有目录和读取入口。

另在 `McpManager::catalog` 补齐资源/提示词能力判断：不支持该能力的服务器返回空目录，避免把整个服务误判故障；移除 20 页、1,000 条目录的额外硬限制，继续遵守服务端游标及重复游标检测，支持大量 MCP 资源和提示词。提交 `d33aa9b8124dab4d8cca1a4182cb854e45a74d2a`。

静态复核发现现有测试及旧 MCP 服务器允许初始化响应省略 `capabilities`，因此 `bc5d26276b534a28d063576d420218ac1d4212f8` 保留未声明能力时的旧式工具/目录发现；显式声明仅支持资源/提示词的服务器仍跳过不支持的工具方法。另有一项探索性工具发现失败处理改动已通过 `217fe5cbcc9955d40fa3d88aa4f49d75179f61bc` 撤回，未计入有效补齐。

本段按**两个独立用户可感知功能**计数：资源/提示词专用 MCP 服务连接、完整资源/提示词目录读取。源码已提交、待行为验收；未运行 Cargo、测试或 CI，完整等价验收新增 0。仍缺动态资源/提示词列表变化后的主动刷新、断线重连时的目录缓存失效、跨厂商多模态等价；Issue #3 冻结 362 条不勾选。

## 2026-10-29：MCP 资源更新通知不再静默丢失（源码已提交，待验收）

原版依据：`source/src/services/mcp/useManageMCPConnections.ts:705–730` 的资源列表变化通知回调会逐次处理事件，不设置 1,024 条静默丢弃门槛。Rust 原有 `src/mcp.rs::Client::request`、`src/mcp_http.rs::HttpClient::post/poll_notifications` 在资源通知积压达到 1,024 条后直接丢弃后续事件，SDK 无法获知全部更新。现移除 stdio、HTTP 请求附带 SSE、HTTP 独立 GET 三处静默上限，保留 `McpManager::drain_resource_updates` 的原有消费入口。源码提交 `1ea7832626d95e7bd641277cd418b6e45f492e4c`、`82c1eb66c4e9d3dd260f8eded24c65e5e823e960`。

按**一个独立用户可感知功能：持续接收 MCP 资源变更通知**计数。源码已提交、待行为验收，未运行 Cargo/测试/CI；仍缺原版的资源自动刷新与技能/提示词缓存联动、通知背压与断线恢复，完整等价验收新增 0，Issue #3 冻结清单不勾选。

## 2026-10-29：MCP 工具调用超时、完整输出与分页发现（源码已提交，待验收）

对照完整原版源码 `source/src/services/mcp/client.ts:211–228,1743–1757,3067–3099,2662–2799` 和 `source/src/utils/mcpValidation.ts:14–46,151–208`，核查 Rust 真正入口 `src/mcp.rs::McpManager::connect_with_root/refresh_changed_tools/call`，已提交 `c802ab7c014ff60ff0d9bb3c8012c5cf10f9f41f`：

1. **长时间运行的 MCP 工具**：此前所有 RPC 共用 30 秒超时，合法的长任务被提前中止。现在仅 `tools/call` 按原版使用默认 100,000,000 毫秒及 `MCP_TOOL_TIMEOUT` 环境变量；发现、读取等控制调用保留短超时。
2. **MCP 工具完整长输出**：此前结果无条件截断到 16,000 字符且无提示。现在普通结果保留完整文本，超过默认 25,000 token 对应的近似字符预算时将完整非图片结果持久化到工作区 `.atlas/tool-results/`，返回可用 Read/Shell 读取的真实文件路径；防止符号链接越界并检查写入错误。图片长结果明确提示截断。此为保守字符估算，尚未对齐原版精确模型 token 计算、图片压缩及结构化多模态输出。
3. **大量 MCP 工具分页发现**：首次连接和工具清单动态刷新都移除 20 页的额外硬拒绝，继续使用服务端 `nextCursor` 和重复游标检测，保留现有工具冲突检查。

补充跨模块接线：`7f28d867fb0aaa521aacacc8a9089e949ff5ea55` 同步修复 `src/mcp_http.rs` 的 HTTP 客户端全局 60 秒超时（原先会抵消 `tools/call` 长超时）以及工具结果 2 MiB 硬拒绝；HTTP 初始化和通知单独保持 30 秒控制期限。此提交归入上面已有的“长时间运行的 MCP 工具”和“MCP 工具完整长输出”两项，不额外增加功能计数。仍未运行行为测试。

本轮按**三个独立用户可感知功能**计数；同一功能的多个调用位置不重复计数。以上为源码已提交、待行为验收，本任务未运行 Cargo、测试或 CI，完整等价验收新增 0。剩余：MCP 取消与进度事件、资源更新排队、会话重连、长结果精确 token 判断与图片压缩、远程多模态/SDK 全链；Issue #3 固定 362 项全部继续开放。

## 2026-10-29：项目指令排除、来源控制与外部引用授权（源码已提交，待验收）

原版依据：用户完整 2.1.88 源码 `source/src/utils/claudemd.ts::isClaudeMdExcluded/resolveExcludePatterns/processMemoryFile/getMemoryFiles`（547–684、790–925 行），以及 `source/src/context.ts::getUserContext`。实际 Rust 入口：`Agent::system_prompt` → `conversation_context::project_instructions_with_dirs` → 指令/规则/@include 读取 → 模型系统提示词。

- **指令文件排除**：`src/config.rs` 新增持久化 `instructionExcludes` glob 配置；`src/conversation_context.rs` 对用户、项目、本地指令、递归规则和引用文件在读取前过滤，绝对路径模式增加符号链接真实路径展开，避免排除文件仍进入模型。代码 `7de3b735`、`03b5697f`、`0b774e44`（源码复核修正通配符引用匹配）。
- **指令来源开关**：`instructionSources` 可分别设置 `user/project/local` 为 `false`，从实际模型上下文加载入口禁用对应来源；项目开关同步控制附加授权目录。默认全部开启，兼容旧配置。代码 `95f8dde9`、`f3c2c0f6`。
- **外部 @引用显式授权**：用户级指令按原版允许引用外部文件；项目、本地和附加目录默认继续限制在批准的范围，用户在 `approvedExternalInstructionIncludes` 中按项目 Git 根目录设置 `{"/绝对/项目路径": true}` 后才允许该项目递归加载工作区外文件，子目录启动继承同一授权，其他项目不受影响；保留深度、去重及排除过滤。代码 `93493750`、`123d67f0`、`5f5a9940`、`9c8d6609`、`63695104`。
- **全路径规则生效**：原版 `claudemd.ts::parseFrontmatterPaths` 对 `paths: **`、空列表和仅包含 `**` 的数组按无条件规则加载；Rust 原先遇到任意 `paths:` 就整条跳过。现修复这些规则的全局加载，同时继续隔离有实际路径限制的条件规则。代码 `63133ee2`。
- **组织托管指令**：原版 `getMemoryFiles` 先加载 Managed 指令及规则，且不受个人/项目来源开关和普通排除配置影响。新增 `managedInstructionFile`、`managedRulesDirectory` 显式管理路径，实际加载顺序为托管→个人→项目→本地；托管规则递归与引用同样受路径授权和去重约束。代码 `fa0895c8`、`be2417eb`。
- **完整读取长指令与非标准 UTF-8 文本**：原版 `safelyReadMemoryFileAsync` 直接按 UTF-8 读取文件，没有 50,000 字节或 200 文件的硬拒绝；Node 对非法 UTF-8 字节使用替代字符。Rust 移除两个额外硬上限，并使用有损 UTF-8 解码保留可读内容，继续保持原有目录作用域、递归深度和去重检查。代码 `764d05f8`。

按**六个独立用户可感知业务功能**计数：排除指令、选择指令来源、批准外部引用、全路径规则正确加载、组织托管指令、完整读取长指令；不按函数或提交数重复计数。均为**已提交、待行为验收**。按本任务要求未运行 Cargo、测试或 CI；完整等价验收新增 0。原版外部引用交互式审批、按访问文件动态加载条件规则、完整 Markdown 词法、托管策略的系统级只读强制、自动路径发现和 SDK 配置来源传播仍有差异；Issue #3 冻结 362 项继续开放。

## 2026-10-29：MCP 提示词完整多消息与长内容传递（源码已提交，待验收）

原版 `source/src/services/mcp/client.ts:2073–2087` 的 `getPromptForCommand` 对 `result.messages` 逐条转换并展平，没有 64 条消息或 128 KiB 内容的额外限制。Rust `src/mcp_prompt.rs::render_content_in_workspace` 原先在多消息提示词或长文本超出该固定数量时整条拒绝，造成用户调用合法 MCP slash prompt 失败。现移除两项额外硬上限，保留每条消息的角色/内容结构检查、图片格式验证、二进制附件保存及现有真实 Agent 多模态输入路径；不丢弃已成功读取的内容。

按**一个独立用户可感知功能：完整执行 MCP 多消息提示词**计数；数量与长度属于同一功能，不重复计数。源码已提交、待行为验收，未运行 Cargo/测试/CI。仍缺原版图片缩放、MCP 会话取消/重连、二进制文件生命周期及全协议等价；Issue #3 固定 362 条继续开放。

## 2026-10-29：MCP 多服务器连接不再整批拒绝（源码已提交，待验收）

原版 `source/src/services/mcp/client.ts::fetchToolsForClient`、`source/src/services/mcp/config.ts` 与 `MCPConnectionManager.tsx` 对 MCP 服务器连接/发现没有全局 16 个服务的硬上限。Rust `src/mcp.rs::McpManager::connect_with_root` 原先在用户配置第 17 个服务器时立即报错，导致包括前 16 个正常服务器在内的所有工具不可用。现删除此额外数量闸门，继续使用原有逐服务启动失败隔离、名称冲突检查及各自工具发现逻辑，避免因数量超过 16 而整批失败。

按**一个独立用户可感知功能：多个 MCP 服务同时可用**计数。源码已提交、待行为验收，未运行 Cargo、测试或 CI。仍缺高并发连接资源调度、取消、重连及全部异常路径等价；Issue #3 冻结 362 条不勾选。

## 2026-10-29：工具搜索大量结果与项目引导识别已有指令（源码已提交，待验收）

1. **ToolSearch 大量工具结果**：原版 `source/src/tools/ToolSearchTool/ToolSearchTool.ts:21–34,297–301,328–329` 的 `max_results` 是可选数字，关键词搜索直接按请求数量截取，没有 100 的固定上限。Rust `src/tool_search.rs::definition/execute` 原先拒绝 `max_results > 100`，模型在拥有大量 MCP 工具时无法一次获取所需候选。现去掉人为 100 上限，仍拒绝零、负数、非整数；以可用整数范围限制内存索引，保留原版关键词排名与选择逻辑。
2. **项目引导识别现有指令**：原版 `source/src/projectOnboardingState.ts:19–47` 在工程已有指令文件时完成引导；Rust `src/conversation_context.rs` 已支持 `AGENTS.local.md`、`.atlas/AGENTS.md` 等，但 `src/project_onboarding.rs::steps` 只检查两种路径，造成已配置指令的项目仍要求运行 /init。现将引导检查与真实 Agent 可加载的主要指令文件路径对齐，避免重复生成指令文件。

两项均为用户可感知的独立业务功能，**源码已提交、待行为验收**。未运行 Cargo/测试/CI。ToolSearch 原版浮点/负数特殊行为、全部延迟加载条件和项目引导完整 UI/并发配置写入仍待核对；Issue #3 冻结 362 条继续开放，完整验收新增 0。

## 2026-10-29：指令继承与附加目录补齐（源码已提交、待验收）

原版依据：`source/src/utils/claudemd.ts::getMemoryFiles` 第859–975行，`source/src/context.ts::getUserContext`。Rust 真实调用链：`Agent::run/context_usage_snapshot` → `system_prompt_with_dirs` → `project_instructions_with_dirs`。

- 嵌套 Git 工作树：识别主仓库与子工作树，跳过主仓库已签入指令/规则，仍加载本地私有指令，避免双份冲突。代码 `fb4210e7`。
- 已授权附加目录：显式 `ATLAS_ADDITIONAL_DIRECTORIES_INSTRUCTIONS=1` 后，读取 `--add-dir` 或配置授权目录中的指令和规则，接入 Agent 请求与 /context 统计，保留大小、数量、边界和去重检查。代码 `80336875`、`22b50ecd`。
- 两条独立业务功能**源码已提交、待行为验收**。未运行 Cargo、测试或 CI；完整验收新增 0，Issue #3 冻结362条保持开放。外部引用审批、条件规则动态加载、记忆排除、来源开关、SDK 全路径及跨平台行为仍缺。本轮100项硬目标仍差98，100项问题清单和方案尚未完成。

## 2026-10-29：MCP 长说明与服务器指令按原版安全截断（源码已提交，待验收）

完整原版 `source/src/services/mcp/client.ts:218,1157–1170,1786–1794` 对 MCP 服务说明和工具说明分别应用 2,048 个 JavaScript UTF-16 单元上限并附加可见截断提示。Rust `src/mcp.rs` 原先对模型工具说明无上限，服务器说明允许 16,384 个 Unicode 字符，导致大量服务器文本进入模型上下文。现新增 Unicode 边界安全的 2,048 UTF-16 单元截断函数，分别接入初次发现/动态刷新工具说明和 Agent 实际使用的 MCP 服务器指令，保留 `… [truncated]` 提示，避免长文本挤占模型上下文。

本轮按**两个独立用户可感知能力**计数：① MCP 工具说明的上下文控制；② MCP 服务器指令的上下文控制。代码已提交、待行为验收，未运行 Cargo/测试/CI。原版截断边界若落在 UTF-16 代理项中，Rust 按完整 Unicode 字符截断；仍需实测、核对 MCP 各传输与多模型请求。Issue #3 冻结 362 条继续开放，完整验收新增 0。

## 2026-10-29：远程会话上传附件的完整批量接收与工作区边界（源码已提交，待验收）

原版 `source/src/bridge/inboundAttachments.ts:31–48,64–116,123–174` 使用数组 schema、逐个下载、独立失败隔离并将成功文件的引用附加到用户消息；原版未设置 8 个附件或 16 MiB 单文件的固定上限。Rust `src/bridge_inbound_attachments.rs::extract/download_one/resolve_and_prepend` 原先对超过 8 个附件整批丢弃，对超过 16 MiB 的合法附件丢弃；现移除这两个额外限制及字符串任意长度限制，保留原版逐附件失败隔离与真实文件保存。上传目录按工作区逐级规范化验证，拒绝 `.atlas` / `uploads` / 会话目录的符号链接越界，避免网络文件写出当前工作区。原始来源的厂商账号/API 绑定仍由中立可配置桥接替代。

本次按**一个独立用户可感知功能：远程消息附件接收**计数，涉及同一链路的数量、大小和目录安全不拆项。已提交、待行为验收；按本开发任务要求未运行 Cargo、测试或 CI。仍需对照原版网络失败、流式内存占用、下载取消、重启清理和多平台路径处理；Issue #3 冻结 362 条保持开放，完整验收新增 0。

## 2026-10-29：MCP 工具直载与搜索提示元数据贯通（源码已提交，待验收）

依据完整原始 2.1.88 源码 `source/src/services/mcp/client.ts:1765–1785` 与 `source/src/tools/ToolSearchTool/prompt.ts:53–108`、`ToolSearchTool.ts:235–290`，对照 Rust `src/mcp.rs::McpManager::connect_with_root/refresh_changed_tools`、`src/tool_search.rs::visible_catalog/execute`：

- **MCP 工具无需搜索直接可用**：原版 MCP 发现结果的 `_meta` 直载开关此前在 Rust `tools/list` 转换时丢失，模型第一轮无法直接调用声明直载的工具。现将原版扩展及中立 `alwaysLoad` 映射为内部布尔值，并贯通初次连接与动态工具列表刷新；直载工具立即进入模型工具清单。
- **MCP 工具关键词检索提示**：原版 `searchHint` 此前被丢弃，导致模型按服务声明的功能关键词找不到相关工具。现保留并规范化空白字符，供 ToolSearch 原有关键词排名使用；动态刷新同步更新提示。
- **模型协议边界**：模型实际接收的 `tools` schema 移除内部 `alwaysLoad/searchHint/shouldDefer/_meta` 字段，避免通用模型 API 拒绝非标准函数定义。MCP 原始扩展只在发现阶段解析。
- **状态**：2 个不同用户可感知业务功能的 Rust 源码已补齐并提交，尚未运行 Cargo、行为测试或 CI；仅做源码静态审阅，均为**已提交、待验收**。仍缺 MCP 待连接服务器提示、所有动态权限/重连/取消及完整工具发现生命周期；Issue #3 固定 362 条全部保持开放，完整等价验收新增 0。

# Rust 全量迁移实况（基于逐文件台账）

## 2026-10-29：MCP 提示词二进制资源实际落盘（源码已提交、待行为验收）

对照完整原始 2.1.88 ZIP 的 `source/src/services/mcp/client.ts::transformResultContent/persistBlobToTextBlock`（音频和非图片资源分支）及 `source/src/utils/mcpOutputStorage.ts::persistBinaryContent/extensionForMimeType`。Rust 原 `src/mcp_prompt.rs::render_content` 对音频直接报不支持、对非图片资源 blob 明确报缺少落盘路径；实际入口为 `src/agent.rs` 的 MCP slash prompt → `McpManager::get_prompt` → `mcp_prompt::render_content` → 模型输入。

- **功能 1：MCP 提示词音频附件**。增加音频 base64 解码、按 MIME 扩展名保存原始字节、向模型传入保存路径与失败原因，供后续 Read/Shell 使用。
- **功能 2：MCP 提示词非图片资源附件**。将原先拒绝的 `resource.blob`（PDF/ZIP/Office/音视频等）接入同一真实文件保存路径，保留资源 URI 文本前缀和原有图像多模态处理；失败显式反馈。
- **实际接线**：`src/agent.rs` 传入当前 Agent 工作区 `self.root`；`src/mcp_prompt.rs` 在 `.atlas/tool-results/` 使用随机独占文件名、路径规范化/符号链接边界检查、同步落盘、失败时清理不完整文件，并保持提示词内容长度约束。涉及代码提交 `e5413535`、`cbbd0156`、`6798f28c`、`fb3150b2`、`57c02132`。
- **仍缺**：原版图像压缩缩放、MCP 二进制附件跨会话恢复/清理、全部 MIME 和特殊 base64 容错、磁盘写入故障实测、并发/取消/重启、完整 MCP 资源协议和最终等价验收。
- **本轮计数**：2 个不同用户可感知业务功能发生源码补齐并已提交，均为**待行为验收**；新增完整等价验收 0。按本开发任务现行要求未运行 Cargo、测试或 CI。100 项数量目标仍差 98，冻结 Issue #3 的 362 条保持开放。


## 2026-10-29：队友状态树与单行渲染真实接线（部分实现）

- `src/components/Spinner/TeammateSpinnerTree.tsx` 对应 `src/generated_ports/p_024038719ca35.rs`：现已将真实任务筛选、排序、选中状态、终端列宽和消息预览传给 `p_09647f4245ece.rs::render_source_view`，替代原先无子节点的静态 `TeammateSpinnerLine`。
- `src/generated_ports/registry_03.rs` 把两个组件的 `view` 出口接通 Rust 渲染器，更新 `catalog` 并增加注册表到组合视图的数据与空值回归测试。核心提交 `a1b42bfcd311b27aa9c293f6794b6b7f44dba5dc`。
- 源码调试视图与嵌套数据传递已接通；真实终端的刷新、键盘选择与完整交互仍待验收。两条路径继续保留部分实现，不计入已验收。




## 2026-10-29：原版指令发现链路补齐（源码已提交、待行为验收）

对照用户原始 2.1.88 源码包 source/src/context.ts::getUserContext、source/src/utils/claudemd.ts::getMemoryFiles/getClaudeMds/processMdRules，核对 Rust src/agent.rs::system_prompt 与 src/conversation_context.rs，发现原先仅加载当前目录的 AGENTS.md 和 .atlas/instructions.md，父目录/个人级指令、递归规则均未进入真实模型请求。

- Rust 实际代码：src/conversation_context.rs 新增 project_instructions，按用户→父目录→当前目录顺序发现厂商中立的 AGENTS.md、.atlas/AGENTS.md、.atlas/instructions.md、AGENTS.local.md、.atlas/instructions.local.md、.atlas/rules/*.md 和个人配置目录规则；规则递归、规范路径去重、文件数/大小边界、符号链接作用域检查、ATLAS_DISABLE_PROJECT_INSTRUCTIONS 显式关闭。
- 原版 frontmatter 的 paths 条件规则只允许在命中文件后加载；当前没有目标文件时，Rust 不把这些规则误当作全局指令。原版还包含外部 @include 审批、工作树去重、按访问文件动态条件规则、managed/user/SDK 来源过滤等，本次均未宣称等价。
- 实际 Agent 接线：src/agent.rs::system_prompt 在模型请求前追加该函数输出；保留现有输出风格处理。补正配置目录读取错误传播，避免用户规则静默缺失；Git 状态快照的 2,000 字符限制改为原版 JavaScript UTF-16 计数，修复中文和表情文件名的截断偏差。代码提交 42b729ca、549f12b2、5db0100c、24e27e1c；台账提交 052034cf。
- 本轮保守计数：**1 个用户可感知业务功能链（项目指令继承与规则发现）源码已提交待验收**，完整等价验收 0；本轮目标 100 项尚差 99。按用户要求未运行 cargo、测试或 CI，只静态复查远端源代码及调用入口。Issue #3 冻结 362 条继续全部开放，PR #2 保持 Draft。


## 2026-10-29：搜索与交互工具源码补强（本轮四项，待行为验收）

基准：用户资料库完整 `claude-code-2.1.88-main(1).zip` 的 `source/src/`，逐项读取对应原始 TS/TSX 与 Rust 真实 Agent/工具调用入口。以下按**四个独立用户可感知功能**计数；同一功能内部的多个修正合并计一次。全部只提交至 `feat/full-rust-parity` / Draft PR #2。

| 功能 | 原版代码及行为 | Rust 真实入口、补齐内容 | 提交 | 剩余差异 |
|---|---|---|---|---|
| WebSearch 网页搜索 | `src/tools/WebSearchTool/WebSearchTool.ts::validateInput/call`：允许域名与屏蔽域名不能同时指定，输出包含搜索耗时 | `src/websearch.rs::execute`：在任何网络请求前拒绝双域名列表，响应加入 `durationSeconds` | `74dd928c` | 原版服务器侧搜索/引用分组、进度事件、模型摘要及厂商中立适配仍待完整核对 |
| ToolSearch 工具发现 | `src/tools/ToolSearchTool/prompt.ts::isDeferredTool`、`ToolSearchTool.ts::searchToolsWithKeywords`：MCP 及交互/任务/网络等工具延迟加载，关键词只检索延迟工具，精确名称可选已加载工具 | `src/tool_search.rs::is_deferred/visible_catalog/execute`：延迟目录接入真实 Agent 的 `visible_catalog` 与下一轮模型工具清单，按原版过滤关键词候选 | `c478582f` | MCP `alwaysLoad` 元数据贯通、待连接服务器提示、原版所有条件性延迟策略 |
| Grep 文件内容搜索 | `src/tools/GrepTool/GrepTool.ts::call`：`rg -l` 按行匹配锚点、`--type` 使用文件类型别名，默认排除隐藏文件 | `src/search.rs::grep_files/type_matches`：修复 `^/ 在非首行的文件匹配，过滤隐藏后代，扩充常用 ripgrep 文件类型别名 | `233f1361` | 完整 ripgrep 类型数据库、所有 .ignore/gitignore 规则、复杂正则及文件竞态 |
| AskUserQuestion 交互问答 | `src/components/permissions/AskUserQuestionPermissionRequest/AskUserQuestionPermissionRequest.tsx`：用户自填 Other 文本同时作为回答及 `annotations.notes` 传递 | `src/ask_user_question.rs::execute`：终端真实用户选择的自定义内容保留为答案和备注，进入下一轮模型可见结果 | `9ec6f725` | 原版终端选项 UI、SDK 问答确认、图片附件、完整取消/重启状态 |

**本轮源码修改并成功推送：4/100；本轮达到原版完整等价验收：0；剩余目标：96。** 按现行开发流程未运行 cargo、单元/集成/端到端测试或 CI；四项均为“源码已提交、待行为验收”，不得升级为“已验收”。未先完成 100 条逐项证实且去重的问题清单，故本轮数量目标明确未完成。Issue #3 原冻结 362 条保持全部开放，动态迁移矩阵不得替代固定范围。下次先对比最新 HEAD 与上述四次提交，只续接新差异，优先补原版 ToolSearch 待连接提示、Grep 忽略规则、AskUserQuestion 取消/SDK 交互、WebSearch 引用/进度；其他主链按最新矩阵继续查缺。


## 2026-10-29：复制回复与上下文窗口统计的原版业务补齐（源码已提交，待验收）

原始源码依据：完整 2.1.88 源码包的 `source/src/commands/copy/copy.tsx::collectRecentAssistantTexts/extractCodeBlocks/copyOrWriteToFile`、`source/src/utils/analyzeContext.ts::processAssistantMessage/approximateMessageTokens`；Rust 可达入口 `src/copy_response.rs::copy/recent_assistant_texts`、`src/context_usage.rs::Snapshot::from_request/count_messages`。

1. **/copy 复制助手回复**：结构化回复的多个文本块按原版以两个换行符连接，纯工具调用不被误当成可复制回复；模型输出中未闭合的代码围栏仍可选中复制；反馈中的字符数按 JavaScript UTF-16、行数按换行符加一；终端 OSC52 复制先于文件写入，文件写入失败不会抹掉已经发送的剪贴板内容。修改提交 `ad684791`、`9270d42f`。
2. **/context 上下文用量**：空 `content_blocks` 时继续统计普通文本，避免误显示 0；SDK 同时提供 `tool_use` 块和同 ID 的 `tool_calls` 时只计一次工具调用，避免重复展示。修改提交 `7d7b99fa`。

这两项属于**两个独立用户可感知业务功能**，内部多个修正不拆分凑数；仅静态源码复核，未执行 Cargo/测试/CI，状态均为**源码已提交、待行为验收**。原版完整 Markdown lexer、复制交互选择器、上下文系统/记忆/工具精确令牌估算等仍待核对。Issue #3 冻结 362 条全部保留开放，新增完整等价验收 0 项。本轮预定 100 项中的其余 98 项尚未完成逐项源码确认与提交，未达标。


## 2026-10-29：真实 Hook 业务链的原版语义补齐（源码已提交，待验收）

**原版依据**：用户原始 `claude-code-2.1.88-main(1).zip` 的 `source/src/utils/hooks.ts::createBaseHookInput/getMatchingHooks/executeTaskCreatedHooks/executeTaskCompletedHooks/executeSubagentStartHooks/executeStopHooks/executeTeammateIdleHooks/executeSessionStartHooks/executeSessionEndHooks/executePreCompactHooks/executePostCompactHooks/executeUserPromptSubmitHooks`，以及 `source/src/query.ts` 的 Stop 阻断后继续状态、`source/src/query/stopHooks.ts` 的队友收尾链和 `source/src/entrypoints/sdk/coreSchemas.ts` 的 Hook 输入约束。逐项对照 `src/hooks.rs`、`src/agent.rs` 的可达调用。

**已真实写入 Rust 的业务行为，保守按 16 项不同用户功能计入“已提交、待验收”**（同一事件在独立/团队两条调用链只计一次；同一工具前后 Hook 上下文接线合并计一次）：

1. TaskCreated：创建任务时向脚本传递实际会话、工作目录、记录路径及可选团队身份。
2. TaskCompleted：完成任务时传递相同的任务/会话/团队身份。
3. Task Hook：多个独立脚本同时阻断时保留全部反馈，任务完成的拒绝仍以普通工具结果返回。
4. SubagentStart：独立委派和团队成员启动时传递会话与代理类型。
5. SubagentStop：独立委派和团队成员停止时传递会话、代理记录路径、最终回复、`stop_hook_active` 与权限模式。
6. TeammateIdle：队友空闲脚本获得会话、团队名称、成员名称与权限模式，移除原版未声明的额外代理 ID。
7. Hook 选择：简单名称与 `A|B` 精确匹配、`*` 通配、正则、旧工具名别名，以及按事件选用触发来源/代理类型/工具名；无匹配查询的事件不过滤。
8. SessionStart：区分新会话 `startup`、已有会话 `resume`、清空后的 `clear`，传递真实会话字段和模型。
9. SessionEnd：提供合法的 `other` / `clear` 退出原因及会话字段。
10. Stop：模型在阻断反馈后再次结束时，继续运行 Hook 并传递 `stop_hook_active=true`，不绕过第二次检查。
11. Stop：保留 JSON `stopReason`，将 `continue:false` 的主动停止原因送入会话事件。
12. UserPromptSubmit：真实用户提示触发 Hook 时传递当前权限模式及会话字段。
13. PreCompact：手动/自动压缩前传递原版 `trigger/custom_instructions` 和会话字段。
14. PostCompact：压缩后传递 `trigger/compact_summary` 和会话字段。
15. PreToolUse/PostToolUse/PostToolUseFailure：工具执行前后从活跃 Agent 注入当前权限模式、会话、工作目录及子代理身份，保留实际工具输入/结果/错误。
16. Hook 去重：同一事件多个匹配配置注册相同命令时只执行一次，保留配置顺序及最后匹配定义。

**代码提交**：`a9fe5f0`、`85abed0`、`55bac27`、`a3bae8e`、`58535d3`、`21d7fb0`、`1699678`、`19a0854`、`5dc39a5`、`73e1dca`、`bc39e3c`、`dd9a015`、`33c487c`、`7fe8348`、`1a06c78`、`289222c`、`ce3343f`、`0daa3b0`。均为原草稿 PR #2 分支上的真实代码修改，未新建 PR、未合并 main。按当前任务约束未运行 Cargo、测试或 CI。

**明确未闭环**：本轮 100 个独立业务功能的硬目标仍差 **84 项**，不把现有 16 项扩写成 100 项。原版异步 Hook 注册、prompt/agent/http/callback Hook、`if` 条件、全部 SDK 回调与取消传播、PermissionRequest/PermissionDenied 触发、TeammateIdle 阻断后的继续工作、队友空闲时在办任务的 TaskCompleted Hook、StopFailure 和全事件 schema 精确验证仍需逐项实现；任务/远程等其他主链差异也未消失。下一续接点：`source/src/query/stopHooks.ts:330–460` 对照 `src/agent.rs::run_team`，以及 `source/src/utils/hooks.ts:1615–1867,2909–3021` 对照 `src/hooks.rs`。Issue #3 冻结 362 条仍全部开放，完整等价验收仍 **0/362**。

## 2026-10-29：/compact 自定义指令对齐原版并接入 CLI、SDK（源码已提交、待验收）

完整原版依据：`source/src/commands/compact/compact.ts::call` 第 40–53、96–108 行保留 /compact 参数作为自定义摘要指令；`source/src/services/compact/compact.ts::compactConversation/mergeHookInstructions` 保证用户指令在前，PreCompact Hook 附加指令在后。

- `src/agent.rs::compact_history_custom` 接收并保留自定义摘要要求，写入真实模型总结提示词，PreCompact Hook 获取同一用户指令；自动压缩仍不添加用户指令。提交 `3272f2c3`。
- `src/main.rs` 普通终端的 `/compact [instructions]` 和 SDK `control.action=compact` 的 `instructions`/`customInstructions` 参数通过同一 Agent 入口执行，裸 `/compact` 既有行为保留。提交 `31bc02ae`。
- 当前独立业务能力计数为 1 条：用户按需指定会话压缩保留范围。仍缺完整取消状态传播、压缩摘要界面交互和多模型行为验收，冻结 Issue #3 的 362 项不勾选；本轮未运行 Cargo、测试或 CI。


## 2026-10-29：MCP 资源请求过期重连与错误凭据脱敏（源码已提交、待验收）

原版依据：`source/src/services/mcp/client.ts` 中 401/过期会话触发连接缓存清理并由 `ensureConnectedClient` 获取新连接、更新 resources/prompts 缓存；`source/src/services/mcp/headersHelper.ts` 中动态请求头重新获取。Rust 已有工具调用恢复，本轮扩展到此前遗漏的资源及提示词真实用户入口，并保证未成功确认的请求不会自动重放。

- MCP 用户读取 resources/read、prompts/get、resources/subscribe/unsubscribe 现在通过 `src/mcp.rs::connected_client` 按需恢复失败连接，保留资源订阅恢复和其他 MCP 服务器状态；提交 `03693b72`。
- resources/list、prompts/list 和 resourceTemplates/list 目录查询会独立恢复失败服务器，不丢失正常服务器返回；提交 `c03937b7`。
- 当资源/提示词/订阅及目录请求实际返回会话失效或 401 时，同步记录该客户端失效状态，下次调用重新初始化并刷新动态凭据。失败的原请求不自动重放。提交 `49d6cdb3`。
- MCP 工具、资源、订阅、提示词和通知轮询错误路径统一复用此前连接状态使用的 URL/配置请求头脱敏，避免显露原始配置和环境变量展开后的查询令牌。提交 `f7924b3e`、`a7177984`。

本小节按 **2 条不同用户业务链**（MCP 资源/提示词恢复、MCP 错误隐私保护）计入源码实际补强。完整 OAuth PKCE 交互、scope 提升后重新授权、刷新令牌轮换及注销仍未实现；Issue #3 的 362 条冻结清单不勾选。本轮没有运行 Cargo、测试或 CI，均保留源码已提交、待验收。


## 2026-10-29：压缩生命周期 Hook 原版行为与多模态 Token 计算续接（源码已提交、待验收）

本次从完整 Claude Code 2.1.88 原始 ZIP 对照 `source/src/utils/hooks.ts::executePreCompactHooks/executePostCompactHooks/executeHooksOutsideREPL`、`source/src/services/compact/compact.ts::compactConversation`、`source/src/services/tokenEstimation.ts::roughTokenCountEstimationForMessages`。源码审阅范围为真实 Agent 压缩、Hook 命令执行和 SDK 结构化消息预算，无静态生成文件计数。

- **压缩 Hook 实际执行链**：`src/hooks.rs::plain_hook_context` 对 PreCompact 成功命令输出保留文本，`src/agent.rs::compact_history_with_trigger` 传入 auto/manual 作为 Hook 匹配参数、将回传上下文加入摘要 prompt，并使 PostCompact 按同一触发来源匹配。提交 `2d0584e4`、`dd1a319f`、`eec9a2a3`。
- **失败与并发次序**：原版 PreCompact/PostCompact 外部命令失败仅显示状态，不阻断有效压缩摘要；Rust 现在给出诊断继续处理。`src/hooks.rs::run` 继续并发启动所有 Hook，改为按配置注册顺序合并结果，符合原版 `executeHooksOutsideREPL` 中 `Promise.all(hookPromises)` 的有序结果。提交 `b241e11b`、`4b6a4a0c`。
- **多模态工具请求 Token 预算**：`src/agent.rs::compact_round_token_estimate` 对同时提供标准化 `tool_calls` 与 `content_blocks` 内 `tool_use` 的 SDK 消息只计一次调用内容，避免误判上下文耗尽和过早自动压缩。提交 `0bef4b9e`。

以上有效变化按 **2 条用户可感知业务链**（压缩 Hook 及自动压缩预算）计入本轮源码补强；完整等价验收 0 项。尚未闭环：压缩及记忆选择取消信号传递、缺少 Assistant 消息 ID 时的流式 API 轮次精确分组、PreCompact 用户自定义指令 UI、OAuth 交互授权和 MCP 权限完整生命周期。按现行用户约束未运行 cargo、测试或 CI，不勾选冻结 Issue #3 362 条。


## 2026-10-29：后台任务跨会话隔离、启动失败回收与落盘安全（源码已提交、待验收）

原版对照：完整 2.1.88 源码 `source/src/utils/task/diskOutput.ts::getTaskOutputDir` 以首次会话 ID 固定任务输出目录，避免同一工作区的并发会话相互覆盖；`source/src/Task.ts` 的任务生命周期；`source/src/utils/task/TaskOutput.ts` 的输出文件管理。Rust 真实执行入口为 `src/agent.rs::Agent::new` → `src/background.rs::BackgroundTasks::start_with_shell/output/stop/list`。

1. **不同会话的后台任务互不串线**：原 Rust 全工作区共用 `.atlas/background`，新会话可看到旧会话任务。新增 `BackgroundTasks::for_session`，由 `Agent::new` 传入当前会话 UUID，将日志、元数据、任务列表及停止/读取操作限定在各自会话目录；同一 Agent 期间保持原输出目录，Shell 仍在真实项目根目录执行。提交 `517dd1e`、`803cc7f`、`6c40487`。
2. **后台命令注册失败不留下孤儿进程**：Shell 启动失败时删除已预留的输出文件；子进程启动后若任务元数据写入失败，终止整个 Unix 进程组、等待进程退出并清理输出文件，避免用户收到错误但命令继续执行。提交 `517dd1e`。
3. **输出写入失败如实标记任务失败**：stdout/stderr 写入任务报错或异常退出时记录可见诊断，避免 Shell 返回 0 就把丢失输出的任务标为成功。提交 `517dd1e`。
4. **后台任务元数据文件安全**：元数据读写拒绝 Unix 符号链接、校验常规文件及大小上限，采用唯一临时文件、写入同步和原子替换，防止同目录恶意链接导致的越界读取/覆盖及残留临时文件阻断恢复。提交 `91b5f79`、`34a56f6`。

5. **退出时批量停止任务不再逐个等待**：`BackgroundTasks::stop_all` 先对所有仍运行的任务发出取消，再以共享 3 秒期限等待，避免同时存在几十个 Shell 时退出被逐个等待拖延数分钟。提交 `0b46118`。
6. **停止任务的结果反映真实状态**：`BackgroundTasks::stop` 在完成通知后核对最终状态；若命令恰好自行完成，不再虚报“成功停止”。对照 `source/src/tasks/stopTask.ts` 与 `source/src/tools/TaskStopTool/TaskStopTool.ts`。提交 `52f538e`。

本轮为 **6 条独立业务/安全行为的源码补强**，均已推送 `feat/full-rust-parity`，尚未运行 Cargo、测试或 CI，**仅标记已提交待验收，完整等价验收新增 0 项**。已知剩余：多任务类型统一生命周期、任务完成通知与状态驱逐、5 GiB 输出上限的主动进程终止、跨进程恢复、用户 UI 与 SDK 事件等。冻结 Issue #3 的 362 项继续开放，100 项目标尚差 94 项。



## 2026-10-29：会话压缩 Token 差额与远程 MCP 失效凭据续接（源码已提交、待验收）
- **MCP 失效连接同轮按需重连**：新工具调用通过 `src/mcp.rs::call` 对失败的服务器按需重建连接，调用动态凭据助手，保留其他服务器及工具路由，不重放失败调用。提交 `b30acce3`。
- **MCP 权限范围提升提示**：`src/mcp_http.rs::post` 识别 `WWW-Authenticate: ... insufficient_scope`，要求新的交互授权，避免误刷新无法增加权限范围的旧凭据。提交 `5f4b69c0`；完整用户 OAuth 授权仍未实现。
- **压缩重试误判修复**：仅把实际 API 错误标记为上下文超限的成功传输结果用于自动重试，正常摘要文字中提及此词不再错误地触发历史删减。提交 `df73725b`。
- **自动压缩涵盖本轮输入与小上下文模型**：将待提交的多模态用户消息一同计入压缩阈值；较小的配置窗口预留四分之一空间，对无效窗口配置直接给出错误提示。提交 `cb78fc9d`、`8f83feb8`。

本轮直接读取完整 claude-code-2.1.88-main(1).zip；对照 source/src/services/compact/compact.ts::truncateHeadForPTLRetry、source/src/services/compact/grouping.ts::groupMessagesByApiRound、source/src/services/api/errors.ts::parsePromptTooLongTokenCounts、source/src/services/tokenEstimation.ts::roughTokenCountEstimationForMessages、source/src/services/compact/autoCompact.ts::shouldAutoCompact、source/src/services/mcp/client.ts 中 401/重新连接处理和 source/src/services/mcp/headersHelper.ts。当前源代码提交均只标记“源码已补齐并提交，待验证”，不得勾选 Issue #3 冻结 362 项。

- **压缩报错按 Token 差额分组删减**：src/agent.rs::compact_token_gap、compact_round_token_estimate、truncate_compact_rounds 保留用户可见的历史轮次边界，能解析 `prompt is too long: X tokens > Y` 并按超限量估算所需删减轮次，无法解析时使用原版 20% 回退，重试前清理前轮标记。提交 7de6e7ca。Rust 现有 Message 无原版流式 assistant ID，仍缺跨流片段完整同 ID 分组、取消传播和模型精确 tokenizer。
- **多模型自动压缩触发**：src/agent.rs::ask_with_blocks 按结构化内容、工具结果和媒体块估算上下文 Token；可通过 ATLAS_CONTEXT_WINDOW_TOKENS 提供当前模型窗口，保留原版 13k 预留空间；ATLAS_DISABLE_COMPACT/ATLAS_DISABLE_AUTO_COMPACT 允许关闭对应模式，不影响手动模式的独立开关。提交 08271117。未提供窗口时维持原有约 25k Token 触发阈值；模型元数据动态窗口、压缩取消及更多外部事件仍待完善。
- **同一模型回合内按需重新连接**：src/mcp.rs::call 在真正执行新工具请求前识别已失效的服务器状态，按原版 ensureConnectedClient 的行为重连并恢复动态凭据；不重放已经失败的原工具调用。提交 b30acce3。
- **远程 MCP 403 需要提升权限**：src/mcp_http.rs::post 识别 WWW-Authenticate 的 insufficient_scope，给出明确重新授权提示，避免把普通权限范围不足误当可通过刷新旧令牌解决。提交 5f4b69c0。完整交互 OAuth 授权及 scope 持久化仍待补齐。
- **远程 MCP 动态认证失效后重连**：src/mcp_http.rs::post/poll_notifications 在 HTTP 401 返回明确认证过期，src/mcp.rs::call/poll_resource_updates 将对应连接标记 failed，后续 reconnect_failed 使用已有配置重新建立连接、重新执行 headersHelper 获取凭据；不会无确认地重放可能已产生副作用的工具调用。提交 91a461b6、3d23068a、9e890a5f。完整 OAuth PKCE、刷新令牌、403 权限升级和撤销生命周期仍待补齐。

本轮 4 条不同的用户业务能力已获得源码补强并推送；均未进行 Cargo/测试/CI，最终等价验收 0。其余候选缺口保持未完成，无法将源码路径、静态文件或同一链的内部修复重复计为 100 个不同功能。


## 2026-10-29：会话 Git 分支检索、任务更新校验及后台任务参数（源码已提交、待验收）

对照用户完整 2.1.88 原始 ZIP 的 `source/src/utils/agenticSessionSearch.ts:110–139,202–253`、`source/src/tools/TaskUpdateTool/TaskUpdateTool.ts:33–65,158–274`、`source/src/tools/TaskOutputTool/TaskOutputTool.tsx:30–34` 和 `source/src/utils/semanticBoolean.ts`，本次完成三条独立用户可感知功能的 Rust 代码补齐：

1. **历史会话按创建时 Git 分支查找**：`src/conversation_context.rs::branch_at_start` 在新会话创建时读取实际分支；`src/agent.rs::Agent::new` 仅在空的新会话设置，`src/session.rs` 兼容旧会话持久化，`src/session_control.rs::list/search` 及 `src/semantic_session_search.rs::prompt` 将分支加入本地预筛和模型排序，语义排序提示遵循原版标签、标题、分支、摘要、全文优先级，并以 UTF-16 单元截取首条消息。提交 `398be3f7`、`fd4c5ef8`、`65bcd09a`、`6f1cb0d6`、`a6b7205a`、`b93bfc26`。未覆盖旧会话缺失分支元数据的历史补录和完整跨项目日志。
2. **任务更新失败时不发生部分修改**：`src/task.rs::dispatch(task_update)` 在改变任务标题、状态、备注前一次性校验所有提供字段；非法状态或错误类型直接返回错误，保留任务原值。提交 `fb380559`。该入口为现有本地会话任务工具，TaskGraph V2 的完整 Hooks、依赖和团队行为仍需单独对照。
3. **后台任务输出输入校验**：`src/semantic_inputs.rs` 不再错误地将 TaskOutput 的数字字符串超时转换为数字；`src/tools.rs::dispatch(task_output)` 按原版 strictObject 拒绝未知字段，接受精确布尔或已规范化的字符串布尔值，超时只允许 0–600000 毫秒数字，保留默认 30000 毫秒；防止超界被静默截断或错误类型触发不必要等待。提交 `12a0cc2e`、`edaaab06`。完整多类型后台任务与跨进程生命周期仍未等价。

以上 **3 项源码已提交、待行为验收**；本轮尚未形成 100 个独立功能缺口的完整预先排查和实施方案，剩余 97 项未交付，不以源码文件或提交次数充数。按现行指令未运行 Cargo、测试或 CI。Issue #3 冻结 362 项继续开放，原迁移矩阵的已验收数量不变。


## 2026-10-29：定时任务遗留差异续接：会话压缩与记忆召回（源码已提交、待验收）
- **自动/手动压缩 Hook 来源**：src/agent.rs::compact_history_with_trigger 使 PreCompact 与 PostCompact 获得正确 auto/manual trigger，并传递压缩摘要；独立的手动 /compact 保留原命令入口。提交 fbbcccd7，待行为验收。
- **自动压缩失败后的提问连续性**：按原版 source/src/services/compact/autoCompact.ts 的连续失败熔断（3 次），为 Rust Agent 增加会话级失败计数。自动压缩失败继续用户提问，三次后停止无效自动压缩尝试，手动压缩成功/清空会话重置计数。提交 2e9ba408；阈值模型上下文计算、取消和重试时序仍待验收。
- **失败重试的模型用量**：超长上下文以模型文本响应返回时记录该次用量，压缩返回空摘要、异常工具调用或重试耗尽时保存已发生的用量；不提前清空历史。提交 3c7e910b。

完整原始依据：source/src/services/compact/compact.ts::truncateHeadForPTLRetry/compactConversation/stripImagesFromMessages、source/src/services/compact/grouping.ts::groupMessagesByApiRound、source/src/utils/attachments.ts::readMemoriesForSurfacing、source/src/memdir/findRelevantMemories.ts。冻结 Issue #3 的 362 条不勾选。

- **压缩重试与历史保护**：src/agent.rs::compact_history/truncate_compact_rounds 对超长上下文错误最多进行三次按对话轮次截断重试，保留原会话，候选会话落盘成功后才交换；PostCompact Hook 阻断时不清空历史。提交 d529af4d。原版 token-gap 定量截断、取消及完整失败时序仍待补齐。
- **工具结果媒体净化**：原版工具结果为 user 块，Rust 为 role=tool；现两者均清除用于压缩的图片/文档二进制，只更改摘要请求副本。提交 038d4971。其他嵌套附件格式待核实。
- **记忆实际名额和累计预算**：只按成功召回的文件计算五个名额，跳过丢失/损坏文件，并逐文件校验会话 60 Ki UTF-16 单元额度。提交 145d07bf。
- **结构化记忆选择**：通过 Message::visible_text 读取多段模型文本块，而非只读取首块或扁平字段。提交 6fd66806。

按用户现行开发任务指令，本轮未运行 Cargo、单元/集成测试或 CI；变更统一为“源码已提交、待验收”。不能将同一业务功能的多处补强拆分为独立完成数。后续优先处理压缩 token-gap 精确截断/取消、记忆召回取消、MCP 动态认证与权限生命周期。


## 2026-10-29：PowerShell 授权界面的破坏性操作提醒（源码已提交，待验收）

- **原版对照**：完整原始 ZIP `source/src/tools/PowerShellTool/destructiveCommandWarning.ts::getDestructiveCommandWarning`（12–108 行），以及 `source/src/tools/PowerShellTool/UI.tsx` 的权限界面。
- **Rust 真实缺口**：`src/tools.rs::ToolExecutor::require_approval` 仅在 Bash 的授权界面展示危险操作提示；PowerShell `Remove-Item -Recurse -Force`、`Clear-Disk`、`Format-Volume`、Git 强制推送与清理等操作缺少原版可见警告。
- **实际补齐**：`src/powershell_permission_matching.rs::destructive_command_warning` 按原版模式与优先级识别危险操作，包含组合删除标志、PowerShell 别名、磁盘/数据库/系统命令、Git 清理干运行排除；`src/tools.rs::require_approval` 在真实 PowerShell 命令的交互授权前显示提示，保留既有拒绝优先和安全检查，不用提示替代权限判定。
- **代码提交**：`d44012a93c06789d22660391e31f5b62634a60af`、`3b8e96e8e0cc32309a3b6dcc2efc358a3eb45aba`、`f6e598d3992312948c05d49f3b24fca545d30dfb`、`418ac48f99c1f6736d101afa8db01ae69ff40b61`、`73ac27027c47a6a02b37ca542a5440e0ecdd0c7f`。
- **状态**：本轮新增 **1 项独立用户业务功能**，与 Hook 7 项合计 **8/100 项源码已提交、待验收**，尚差 92 项。仍缺原版完整 PowerShell AST、权限 UI 动画、动态命令解析、取消与并发时序；Issue #3 冻结项不勾选。按本任务要求未运行 Cargo、测试或 CI。


## 2026-10-29：Hook 标准输入、工作区、超时和模型可见输出（源码已提交，待验收）

本次对照完整用户原始 ZIP 的 `source/src/utils/hooks.ts`（166–181、877–879、986–1027、2499–2695、2743–2880 行）、`source/src/utils/ShellCommand.ts`（337–346 行），并检查 Rust 的 `Agent::attach_hooks/ask/finish` 与 `HookRunner::run/run_command` 真实可达链，补齐下列 7 条相互独立的用户业务行为：

1. **逐行读取 Hook 输入**：在 JSON 标准输入后写入换行，用户脚本 `read -r line` 能正常解析完整事件，不再因无换行在 EOF 时跳过分支（`457dabb`）。
2. **Hook 在正确的项目目录运行**：Agent、子代理与工作树通过 `with_workspace` 将实际工作区传入 Hook 进程，设置当前目录及中立环境变量 `ATLAS_PROJECT_DIR`，修复相对路径读取错误（`e867516`、`c598420`）。
3. **退出时的 Hook 有独立超时**：SessionEnd 使用原版 1500ms 上限，可通过 `ATLAS_SESSIONEND_HOOKS_TIMEOUT_MS` 调整，避免退出卡在普通十分钟 Hook 超时（`457dabb`、`f80ccba`）。
4. **正常 Hook 允许较长工作**：普通命令 Hook 缺省超时由 30 秒改为原版 600 秒，保留显式 1–600 秒设置（`457dabb`）。
5. **Hook 可见输出真正进入对话**：成功的 SessionStart/UserPromptSubmit/PreToolUse/PostToolUse 普通文本及 JSON `systemMessage` 进入 Agent 上下文和模型可见工具结果，避免用户脚本输出丢失（`457dabb`、`dd17705`）。
6. **启动/退出 Hook 的非阻断错误可见**：SessionStart/SessionEnd 的执行失败、超时警告进入终端及结构化 SDK 事件，不再被丢弃（`fd370ca`）。
7. **超时/取消杀掉 Hook 子进程树**：Unix Hook 在独立进程组执行，取消或超时清理后代进程，防止脚本后台子进程继续写文件或占住管道（`2526ef1`）。

**本轮计数 7/100 项真实用户业务链源码补强，已提交、待验收；尚差 93 项。** 相关修改提交：`457dabbad4efcc2cd46b5059647dbdd844bd6c91`、`f80ccba52f63afec27a554468a8157bf58d9ebbf`、`dd177053a521a9ba38a3a0690159e0e80aa486f3`、`e867516960c6e10fb0a7ae5c5ef34629dbc8657e`、`c5984200f1a61e30bc12da9d683977aed1f634e1`、`fd370ca7dd13a42a93318a3a436d08194965718f`、`2526ef14e2d2732893b6fbb030d72e82283c68f8`。

**剩余真实差异**：AsyncHookRegistry 注册、响应轮询及中断/重试；HTTP/Agent/Prompt Hook；完整权限询问与 allow 决策传递；跨平台进程树终止、全部事件的附件 UI；Hook 动态来源与热重载。冻结 Issue #3 不勾选。遵循当前开发要求未运行 Cargo、测试或 CI；本轮未完成 100 项预先逐项确认的完整清单，不将源文件或静态候选充数。


## 2026-10-29：多模型结构化文本贯通 Agent、命名及三类搜索（源码已提交、待验收）

原版依据：`source/src/utils/messages.ts::getAssistantMessageText`、`source/src/utils/sessionTitle.ts`、`source/src/utils/agenticSessionSearch.ts`、`source/src/utils/transcriptSearch.ts`。新增 `src/protocol.rs::Message::visible_text` 统一提取 SDK `text` 块并在缺少结构化文本时使用扁平内容，落实以下五个独立用户业务入口：

1. Agent 普通对话：`src/agent.rs::ask_with_blocks` 从多模型结构化响应提取所有可见文本，终端返回与 Stop Hook 反馈不再变成空字符串（提交 `4634048f`）。
2. 会话自动命名：`src/agent.rs::rename_session_or_generate` 接受结构化 JSON 文本；`src/session_control.rs::conversation_text_for_name` 保留多模态用户／助手的完整文本顺序（`4634048f`、`db741998`）。
3. 语义会话搜索：`src/semantic_session_search.rs::search` 读取结构化模型排序结果，候选摘要不重复拼接扁平和多模态文本（`4e325679`）。
4. 本地会话搜索：`src/session_control.rs::preview/searchable_transcript` 优先使用多模态可见文本，后续文本块可参与标题与本地关键词检索（`db741998`、`4af38b3e`）。
5. 对话全文搜索：`src/transcript_search.rs::stored_message_search_text` 避免扁平／结构化文本与工具调用重复索引，恢复结构化工具结果中的可见文本（`36cda3f5`、`5f90fce7`）。

本次累计 **11/100 项真实 Rust 源码补齐并提交、待验收**（前述记忆与压缩 6 项 + 本节 5 项）；尚差 89 项。未运行 Cargo、测试或 CI，未核销 Issue #3 冻结 362 项，未预先完成全部 100 个独立缺口的逐项审计，后续仍需处理并发／取消、MCP/权限与更多完整业务链。

## 2026-10-29：记忆召回与多模态压缩主链（源码已提交、待验收）

对照完整原版 `source/src/memdir/memoryScan.ts`、`findRelevantMemories.ts`、`source/src/utils/attachments.ts::collectSurfacedMemories/readMemoriesForSurfacing/filterDuplicateMemoryAttachments`、`source/src/utils/frontmatterParser.ts`、`source/src/services/compact/compact.ts::stripImagesFromMessages` 与 `source/src/utils/messages.ts::getAssistantMessageText`，完成以下六条独立用户行为补齐：

1. **记忆跨轮去重**：`src/memory.rs` 在选择模型请求前剔除已呈现主题；`src/session.rs` 持久保存已呈现路径和额度；`src/agent.rs` 在实际注入后保存，恢复会话继续去重，压缩历史后清零。
2. **已读文件去重**：Agent 将 `files_context::paths` 重建的成功文件工具记录并入排除集，避免已通过文件工具打开的记忆再次占用五个召回名额。
3. **记忆读取和总量预算**：主题文件按原版最多 200 行／4096 字节流式受限读取，超限保留前文并显示截断说明；单会话 60 Ki UTF-16 单元预算，超额停止额外选择但保留索引；拒绝指向外部的 MEMORY.md 符号链接。
4. **记忆 YAML 描述解析**：解析 frontmatter 完整分隔符、引号转义、注释和多行 `|`/`>` 描述，识别原版四种记忆类型；复杂 YAML 语法仍待完整解析器对照。
5. **多模态压缩媒体净化**：`src/agent.rs::compact_history` 在送入模型前替换用户消息和嵌套工具结果中的图片／文档二进制块为文本标记，原始持久历史保持不变。
6. **多模态压缩摘要恢复**：压缩模型返回结构化 `text` 块时按原版拼接为摘要，避免仅支持扁平 `content` 的错误。

实际代码提交：`30d2b018`、`e3443884`、`e044232e`、`ad75f8ea`、`5fe1c444`、`4a7bb1d4`、`ac120f46`、`2f93e430`；并行任务一度覆盖 Agent 接线，已从同期最新源码恢复并保留其他改动。只计 **6/100 项源码补齐、待验收**，未运行 Cargo/测试/CI；冻结 Issue #3 的 362 项仍全部未验收。后续需核对内存附件与原版消息级生命周期、复杂 YAML、完整多模态压缩重试、取消及预算边界。未完成其余 94 项，且未预先完成 100 项逐业务缺口核实，不以候选路径充数。

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

原始路径固定基线为 1,906 条，其中 22 条经用户明确排除；其余路径须逐一完成 Rust 等价实现与对应验收。**最新部分实现、未迁移、已验收、剩余数量，统一从逐文件台账动态计算**，避免并行提交时重复统计失配。

```sh
python3 scripts/port_workbench.py counts --repo .
```

该命令直接读取 [FILE_PORT_MATRIX.md](FILE_PORT_MATRIX.md)，返回 `partial`、`unported`、`accepted`、`remaining` 等真实数量。状态报告记录历史修复和已知缺口，动态数量只以台账为事实源。已经有单元测试的局部行为不自动升级为完整等价验收。

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

## 2026-10-29：远程桥消息回放公共执行层（保持部分实现）
对照 `src/bridge/flushGate.ts`、`src/bridge/sessionIdCompat.ts`、`src/bridge/pollConfigDefaults.ts`，新增 `src/bridge_core.rs`，接入真实 SDK NDJSON 控制入口。历史回放中的出站新消息 FIFO 排队、传输替换保留队列、永久关闭清空、会话隔离、ID 双向转换及轮询默认值均实现。本地 SDK 子进程实测 1/1 通过，Rust 单元测试 5/5 通过。完整 HTTP/WebSocket 实际传输、身份刷新、轮询调度和服务端协议仍待迁移，三条原路径只登记部分实现。

## 2026-10-29：远程入站图像消息兼容（部分实现）
对照 `src/bridge/inboundMessages.ts` 与 `src/utils/imageResizer.ts`，补齐 SDK 用户消息/UUID 提取、缺失媒体类型的图像签名检测及 camelCase `mediaType` 兼容；显式 snake_case 优先，非法格式保留拒绝。直接 SDK 与排队控制入口共用多模态解析，不建立第二状态源。本地新测：4/4 Rust 单测及 1/1 真实 SDK 子进程 + 模拟 HTTP 模型测试通过，JPEG/WebP 图片 URL 进入模型请求。远程网络传输、附件及恢复仍未等价。


## 2026-10-29：远程入站文件附件处理（保持部分实现）
- 代码：`src/bridge_inbound_attachments.rs`、`src/main.rs` 和 `tests/integration_cli.rs`，提交 `66da6baee5d984dd4dbcef022d38f432f8c25a82`。
- 原版对照 `src/bridge/inboundAttachments.ts`：字段整组校验、附件下载、文件名清洗、当前会话目录、文本及混合内容的末尾文本块 @引用插入、无 token/失败时保留用户原文。用户明确配置 `ATLAS_BRIDGE_ATTACHMENT_BASE_URL` 和 `ATLAS_BRIDGE_ATTACHMENT_TOKEN`；不调用厂商专属服务。与原版相比加入单文件 16MiB、最多 8 个附件及 create_new 隔离机制。
- 本地验证：新增 3 项附件模块测试（含真实 HTTP 下载、授权头及目录、异常输入）；新增 1 项真实 SDK 子进程+两台 HTTP 服务（文件服务和模型请求）测试。合并后的本地快照 `cargo test --offline --locked --all-targets` 共 576 项通过，`cargo check --offline --locked --all-targets`、`cargo fmt --all -- --check` 通过。
- 当前仅标记部分实现：远端完整桥接协议、身份刷新、断线重连、取消传播及真实多会话并发未达等价；新的 GitHub HEAD 必须单独核验。


### 原版路径模块与任务查询回归（2026-10-29）

- 统一 Rust 路径逻辑 `src/path_support.rs`，由工作区授权入口 `src/workspace_dirs.rs` 实际调用；保持原有真实路径与符号链接权限边界。
- `src/utils/path.ts` 仍登记部分实现：其展开、NFC、相对路径、配置键、目录探测、穿越检查及清洗行为已有对应逻辑，但 Windows 原生特殊路径、旧会话路径兼容以及其它动态下游仍有未对照项。
- `src/task_graph.rs` 只读非法任务 ID 的结果恢复为“Task not found”，修改仍拒绝非法 ID；针对对应真实回归测试修复。
- 本地在离线源码快照中运行 `cargo fmt --all -- --check`、`cargo check --offline --all-targets -q` 与 `cargo test --offline --all-targets -q`；557 项测试通过，0 失败。最新远端 HEAD 的 CI 另行核实。
- 六条完整业务链的串行推进顺序及逐路径完成标准见 `docs/PORT_CLOSURE_PLAN.md`。


### JSON、JSONC 与 JSONL 源码迁移（2026-10-29）

- 原路径 `src/utils/json.ts` 对照源码写入 `src/json_runtime.rs`：BOM、安全解析和容量 50 的小文本 LRU、JSONC 注释与尾逗号解析、保留已有注释的数组追加、JSONL 损坏行容错、100MB 文件尾部读取。
- `src/keybindings.rs` 的真实用户配置入口与 `src/session.rs` 的会话恢复入口复用统一 JSON 解析，新增带 BOM 的磁盘配置回归测试。
- 新模块专项测试在首次编写后 5 项通过；修改后的源码运行了 Rust 格式检查。最新全部测试与 GitHub CI 需要按新 HEAD 完成核验。
- 待补充源码差异：原版 jsonc-parser 的更复杂编辑布局、终端 setup 调用、其它配置与插件入口、不同文件编码等，保留部分实现状态。


### 非抛出子进程公共执行链（2026-10-29）

- 对照原版 `src/utils/execFileNoThrow.ts` 新增 `src/exec_file.rs`，处理 argv 执行、工作目录、环境、stdin/EOF、双流读取、非零退出码、保留/隐藏失败输出、超时取消及输出限制；所有结果为非抛出结构化值。
- `src/mcp_http.rs` 的远程 MCP 动态 HTTP 凭据 helper 和 `src/portable_configuration.rs` 的 gh 认证检测采用共享执行器，保留原有环境隔离及权限入口。
- 新增 9 项专项测试已本地通过。当前版本另有 Stop Hook 集成测试调整（已由并行提交完成）；本次保留该变更。
- 未完差异：Windows cmd/.bat 规则、显式 shell 选项、全部同步/异步原版调用方接线及跨平台故障恢复验证，继续登记为部分实现。


### 原版同步 Shell 包装层补全（2026-10-29）

- 对照 `src/utils/execFileNoThrowPortable.ts` 和 `src/utils/execSyncWrapper.ts`，新增 `src/exec_sync_compat.rs` 同步执行公共层，支持 stderr/stdout 并行有限缓冲、stdin、工作目录、环境、超时、取消、stdio 模式和 Shell 命令。
- 对非抛出旧 API 保留 stdout trim/空值回退和非零状态下的 stdout；严格旧 API 在非零状态报错；`src/portable_configuration.rs::exec_sync` 真实接入该执行器并保留现有状态码三元组返回。
- 源码自包含验证 6 项测试通过，覆盖运行状态/非零、stdio、环境/工作目录、输入、超时、取消与溢出。主仓库 `cargo check --offline --all-targets`、`cargo fmt --all -- --check` 通过；现有全量单元测试在本地构建过程中耗时中断，最新 CI 另行核对。
- 剩余差异：IDE/macOS钥匙串等上游同步调用方收敛、Windows shell 实际行为、全量跨平台入口测试仍待补齐；保持部分实现。

## 2026-10-29：队友空闲前任务 Hook、阻断继续与错误反馈（待行为验收）

原版完整包 SHA-256 `23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`；本轮阅读 `source/src/query/stopHooks.ts::handleStopHooks`（330–455 行）、`source/src/utils/hooks.ts::executeStopHooks/executeTeammateIdleHooks/executeTaskCompletedHooks`（3639–3830 行）与 Rust `src/agent.rs::run_team/run_task_graph`、`src/task_graph.rs::all`。提交 `dd67ffc1a5802afa12171f85a4448951398e066c`、`90fc78bf46417748bf85ac0ee3f2cafb994a0648`、`984410a6f67be892ef8860278221e639b43a6069`、`4e058e875f11da7eecf35c77a53d282642fee32a`；迁移矩阵同步 `7245deca10a2c40db1de0a4012a6c7cf3d35a007`。

本轮保守计为 **5 个互不重复、真实可感知的业务功能补齐，源码已提交、待行为验收**：

1. **队友在办任务的完成前检查**：真实共享任务图查询该成员 `in_progress` 任务，依原版先运行 SubagentStop、再对每个在办任务执行 TaskCompleted、最后执行 TeammateIdle；任务 Hook 补齐实时 `permission_mode`。涉及 `src/task_graph.rs::in_progress_owned_by`、`src/agent.rs::run_team/task_hook_input`。
2. **阻断反馈后的队友继续工作**：SubagentStop、TaskCompleted、TeammateIdle 的阻断原因进入下一轮真实模型对话，重新检查直到通过或达到原有模型步数边界；`continue:false` 聚合处理、保留 `stopReason` 并向结构化 SDK 发出停止事件；模型失败仍调用 SubagentStop。涉及 `src/agent.rs::run_team`。
3. **任务及队友 Hook 的非阻断错误可见**：TaskCreated、TaskCompleted、SubagentStart、SubagentStop、TeammateIdle 的脚本错误保留为终端警告及 SDK `hook_warning` 事件，不会直接误拒绝工具或跳过其他任务。涉及 `src/agent.rs::report_hook_warnings` 与真实调用点。

4. **会话启动 Hook 阻断不终止会话**：原版 `source/src/utils/sessionStart.ts::processSessionStartHooks`（118–159 行）明确忽略 SessionStart 阻断；Rust `src/agent.rs::attach_hooks` 现保留上下文与警告、继续启动，避免启动脚本返回退出码 2 使会话无法进入。提交 `f2d2193365eab66c47c1a5d383df0d3f7cb4c41c`。
5. **会话压缩先落盘再运行后置 Hook**：原版 `source/src/services/compact/compact.ts`（700–740 行）在 PostCompact 之前完成压缩标记和摘要保存；Rust `src/agent.rs::compact_history_with_trigger` 现先原子保存摘要，再调用 PostCompact；脚本失败保留用户可见警告且不回滚压缩。提交 `1b652b8b11ae7a4dfd8409a7c14ab1b0c35eb0cf`。

独立子代理的 SubagentStart/SubagentStop Hook 现使用子会话及隔离工作树上下文，阻断反馈会继续实际模型轮次，模型失败仍执行停止 Hook；提交 `f3b9bac59755fe30658bf0b3ea3fa39bf958d49e`。此项属于上述第 2 项的同一业务功能，不重复计数。迁移矩阵进一步同步 `4667cd26a68a338929475148cf93eebd8ce1d020`。

**精确未闭环差异**：原版 Hook 进度消息、取消信号与外部中断、异步/HTTP/agent/prompt/callback Hook、SDK 控制回调、真实队友邮箱唤醒、Hook 阻断的原版元消息类型、并发事件时序与跨进程恢复尚未等价；当前继续工作通过带反馈的下一轮提示触发，原版元消息标记尚未对齐；超过步数上限时明确报错并保留会话。冻结 Issue #3 的 362 条继续全部开放，0 条据此升级完整等价。本轮按要求**未运行 Cargo、CI 或任何测试**，只做原版/Rust 源码及远端提交静态核对。100 项硬目标仍未完成，本轮缺口 95 项；本轮亦尚未形成经逐项证明的完整 100 项先行问题清单，后续须先完成该步骤并去重，禁止把本轮 3 项重复计入。


### 插件市场来源与本地安装链（2026-10-29）

- `src/utils/plugins/parseMarketplaceInput.ts` 依据原版源码补入 `src/marketplace_source.rs`，覆盖 SSH/HTTPS/GitHub/ADO/本地 JSON 与目录来源分类。
- `src/marketplace_registry.rs` 实现本地市场 JSON 清单的校验、原子持久化、锁保护、重复来源去重、名称冲突检测、删除与完整刷新预校验；兼容历史市场清单目录与中立新目录。
- `src/plugin_manager.rs` 真实 CLI 支持 `plugin marketplace add/list/remove/update` 及 `plugin install plugin@marketplace` 的本地来源安装；解析后必须验证来源路径仍属于市场根目录，避免符号链接越界。
- 本地执行 `cargo check --offline --all-targets`、`cargo fmt --all -- --check`、`cargo test --offline --all-targets`：581 项通过，0 失败；包括 14 项市场专项测试和真实 CLI 端到端安装测试。
- 未完成：远端 Git/HTTP 市场安全下载、信任验签、缓存和更新、完整跨平台路径语义和插件市场外部插件安装。对应原路径保持部分实现，等待跨链验收。


### 项目指令 @引用递归加载（2026-10-29，源码已提交、待验收）

- 原版依据：完整 2.1.88 `source/src/utils/claudemd.ts::extractIncludePathsFromTokens/processMemoryFile`（448–537、615–685 行），以及 `source/src/context.ts` 注入入口。原版对 Markdown 文本中的 `@path` 进行引用扩展，过滤代码块/HTML 注释，最多 5 层并防止循环；非文本文件跳过，未授权外部项目引用不能直接注入。
- Rust 实际执行链：`src/conversation_context.rs::project_instructions` → `add_file_inner/include_paths` → `src/agent.rs::system_prompt`。新增项目与个人指令文件内部的 `@./path`、相对/绝对/用户目录引用解析、文本文件扩展名白名单、递归去重与深度限制、代码围栏/行内代码/HTML 注释过滤；工作区外项目引用不注入，用户级引用限制在配置目录。源码提交 `b7dd2cf0`、`1b670581`、`ae1f934b`、`2403ea31`（复查原版 `processMemoryFile` 实际为父文件先入队，已纠正注入顺序）、`d76ce876`（按原版剔除注入内容中的 HTML 注释，代码块与行内代码保留原文），迁移矩阵提交 `6ea01a78`。
- 保守按 **1 条用户可感知业务功能链** 计入本次源码补齐（不把解析、过滤、递归拆分刷数）。本轮未形成经逐项原版/Rust 证实的 100 个独立业务问题清单及全部实施方案，100 项硬目标尚差 99 项；**完整等价验收新增 0**。遵循本任务要求未运行 Cargo/CI/测试，仅做源代码及远端提交静态审阅。
- 明确未闭环：完整 Markdown 词法（复杂嵌套代码/HTML）、外部引用用户审批及批准状态、跨 Git 工作树去重、按访问文件动态加载 `paths` 条件规则、SDK 附加目录及记忆排除设置；原版与 Rust 完整行为仍需后续验证。Issue #3 冻结 362 项继续开放。

### 2026-10-29：Hook 历史工具名与团队队长会话重命名（源码已提交、待验收）

- **历史 Hook 匹配**：对照原版完整 ZIP `source/src/utils/hooks.ts::matchesPattern` 与 `source/src/utils/permissions/permissionRuleParser.ts::normalizeLegacyToolName/getLegacyToolNames`，修复 Rust `src/hooks.rs::canonical_hook_tool/HookMatcher::matches`。原版的 `Task`、`KillShell`、`AgentOutputTool`、`BashOutputTool` 旧名称及对应正则现在可映射到 Agent、TaskStop、TaskOutput 的真实 Hook 调度入口；避免升级后旧 Hook 无声失效。代码提交 `e2529c2c7a0df9a514bdd0b1457ef73e995c81e0`。
- **队长重命名与身份恢复**：对照原版 `source/src/commands/rename/rename.ts::call` 与 `source/src/utils/teammate.ts::isTeammate`，发现 Rust `src/agent.rs::run_team_tools(TeamCreate)` 把 `session.agent_name` 写成 `team-lead`，而 `src/session_control.rs::rename` 把任何同时含 team_name/agent_name 的会话都误当队友拒绝。修复后队长可以手动或通过模型自动命名；保存自定义标题时保留 `team-lead` 的路由身份，历史会话列表展示新标题，重新加载仍能进入协调者分支。普通会话与队友的原有限制不变。代码提交 `c082318e8ce57bd0316278886556093f5e5e0318`、`cc7b5933f896e5d5967712e8b02e048d535b1afe`。
- 本次按独立用户行为保守计 **2 项已提交、待行为验收**。只做原版/Rust 源码、可达入口、远端文件及提交静态复核；未运行 Cargo、测试或 CI。完整 100 项预先缺口清单与 100 项代码补齐尚未完成，不以这些提交宣告本轮达标。Issue #3 固定 362 项继续开放，0 项升级完整等价。
- **剩余差异**：Hook 仍缺原版动态注册/取消、按插件与技能来源隔离去重、HTTP/SDK 回调与异步调度；队长重命名尚缺远程通用会话标题同步、原版完整 UI 更新及取消传播，跨进程/行为测试未做。下一续接优先原版 `hooks.ts` 的动态注册与权限请求 Hook，复核并行 HEAD 去重后再接线。

### 2026-10-29：Hook 异常匹配、超时、Agent 入口与 Windows Shell 补强（待验收）

- **无效 Hook 正则隔离**：原版 `source/src/utils/hooks.ts::matchesPattern` 遇到错误正则仅令该匹配不命中，继续处理其他规则；Rust `src/hooks.rs::HookRunner::new` 原先直接拒绝整个 Hook 配置并阻断会话启动。现保留无效匹配器的非命中语义，其他脚本继续执行。提交 `8badc1e0cf89e8e40fa9a1333693eadf38914723`、静态借用修正 `cb61c4ff66e00015429931b2b4072d8f90bd2952`。
- **Hook 超时设置**：原版 `source/src/utils/hooks.ts::execCommandHook` 的显式 `timeout:0` 使用默认 600 秒，用户指定 3600 秒等更长超时有效；Rust 先前将 0 和大于 600 秒全部当作配置错误。现在配置入口规范化 0→600 并保留更长超时，原有超时取消进程组逻辑不变。提交 `476353e950e1bd48adf4075d969dd9dabca73581`。
- **Agent 工具 Hook 可达性**：原版 Agent 工具名为 `Agent`，Rust 真实执行入口为 `spawn_agent`/`spawn_team`；现将二者接入 Hook 精确及正则匹配，使 `Agent`/`Task` 的原版规则可在子代理和团队启动前触发。提交 `7d0d89a075af8bb0c9cd83c7e32b0b5637ef86b3`、回归代码 `d8cfcef96d0f5554f83c6e9daed55bdfd9c15458`。
- **Windows 默认命令 Hook 的 Bash 语义**：原版 `source/src/utils/hooks.ts::execCommandHook` 在 Windows 默认使用 Git Bash，必要时给 `.sh` 脚本补 `bash` 前缀；Rust 原先通过 cmd.exe 执行，导致 POSIX Hook 语法失效。现改用现有 `ShellKind::Bash.build_command` 并接入 `.sh` 检测。提交 `a9b475a2dd12f932ed61f52e08d59fc0c9c6b5c5`、`7293a44c5da668108b5247b6afc2fcfdd4ec5424`、字符串转义修复 `8125f74d158efc34cd7eeb4de0ae28cf326360c5`。
- 与前段 2 项合计本次 **6 项不同用户业务行为的源码补齐，均已提交、待行为验收**；其余 94 项未完成。本轮未形成经逐项证实的 100 项预先缺口清单，不能据此宣称流程 A 或 100 项目标达标。未运行 Cargo、CI 或测试。Issue #3 固定 362 条继续开放，0 项最终等价验收。
- **剩余**：原版 Hook 的独立 `shell:powershell`、插件来源去重、动态/异步/HTTP/SDK 回调、权限请求与取消传播仍缺；Windows Bash 运行及全部改动尚未做行为测试，均保持待验收。

### 2026-10-29：Hook 独立 Shell、Windows Bash 定位与路径注入（待验收）

- **每条 Hook 独立 Shell**：原版 `source/src/utils/hooks.ts::execCommandHook/getMatchingHooks` 允许 `shell:'bash'|'powershell'`，且同一 command 的两种 Shell 必须独立执行。Rust `src/hooks.rs::HookRule` 新增可反序列化的可选 `shell`；未配置默认 Bash，PowerShell 使用 `ShellKind::Powershell.build_command` 的 `-NoProfile -NonInteractive -Command`，去重键加入 Shell 身份。保留现有 34 处 HookRule 代码初始化及源兼容序列化。提交 `4d0774db0a764b833df96fe8c956d1cf12c1a9b5`、`c12995583878385b0a93974db9acbd550448d1df`、`76870c577c582f8bfe3d59685d265bef34159785`、`19f87ab7f72c6b922dd3ff54a56204d298a8aed3`。
- **Windows Git Bash 自动定位**：原版 `source/src/utils/windowsPaths.ts::findGitBashPath` 会发现 Git Bash 并拒绝错误的显式路径。Rust `src/shell_runtime.rs::ShellKind::build_command` 现使用已有 `src/portable_platform.rs::find_git_bash_path` 查找标准安装目录，支持厂商中立 `ATLAS_GIT_BASH_PATH` 严格覆盖，供实际 Bash 工具及命令 Hook 共用。提交 `da7b8a8965968b33c9b8060e7afda56d9b88bbe5`。
- **Windows Hook 项目路径注入**：原版 `windowsPathToPosixPath/toHookPath` 给 Git Bash Hook 传入 `/c/...` 和 `//server/share` 路径，PowerShell 保留 Windows 原生路径。Rust `src/portable_platform.rs::windows_path_to_posix_path` 已接入 `src/hooks.rs::run_command` 的 `ATLAS_PROJECT_DIR` 环境变量，工作目录仍使用本地原生路径。提交 `834d655e4ad3e24ed3d4e344dadbe8dd5a359f79`、`74eced9b82ed9b1864e2f3e8726ec39806683ac5`。
- 队友无历史时自动重命名的权限提示顺序已与原版对齐，提交 `c2010a41556055d13c45658bda7c66dfd26d0a3f`，属于本轮已计入的队长/队友改名业务链，不另计。
- 本轮目前保守计 **9 项独立用户业务功能已提交、待验收**；其余 91 项尚未完成。100 项预先缺口清单与覆盖全数的执行方案尚未完成，禁止视为本轮达标；Issue #3 固定 362 条保持原状。未运行 Cargo、CI 或测试。
- **剩余**：插件/技能 Hook 的来源级隔离与变量替换、异步/HTTP/agent/prompt/callback Hook、Windows 脚本参数细节与完整跨平台行为验证、权限请求及取消传播仍未等价；Windows Git Bash 探测仍有从 Git 可执行路径推导安装位置的差异。


## 2026-10-29：SessionStart 初始用户消息进入真实无输入会话（已提交，待验收）

- **原版依据**：`src/utils/hooks.ts:627–635` 解析 `hookSpecificOutput.initialUserMessage`；`src/utils/sessionStart.ts:20–31,129–155` 保留一次性首条用户消息；`src/cli/print.ts:697–705` 在无头模式把它放入用户消息队列。原始 ZIP：`claude-code-2.1.88-main(1).zip`。
- **原有差异**：`src/hooks.rs::run_command` 忽略该字段，`HookRunner::run` 丢弃汇总值，`Agent::ask_with_blocks` 对空输入一律拒绝；无头自动化即使 SessionStart Hook 提供首条任务也无法开始模型交互。
- **本次真实 Rust 代码**：`src/hooks.rs` 校验后解析、按原版 Hook 注册顺序汇总最后一条有效初始消息；`src/agent.rs::attach_hooks/clear_history` 将消息保留为单次消费的会话用户输入，`ask_with_blocks` 在空输入时使用该消息，有显式输入时将启动消息置于其前面作为独立用户轮次；`src/main.rs::sdk` 在发出初始化消息后、读取外部 SDK 输入前自动执行首条用户任务。交互终端不自动注入该消息；`SessionStart` 其他 Hook 上下文和阻断警告继续保留。
- **提交证据**：`1311f062d94782fd9300b2aff7e0410df39d948a`、`7485c83fecee206c5b724cb46590d35a32e36156`、`1e055ef50f6aa80d39f0d0c151e40e383dab010e`、`974dd6767248fa5f404e9bc5616e580c7dc8dbeb`。只进行源码和可达入口审查，未运行 Cargo、CI 或行为测试，状态为**已提交、待验收**。
- **未闭环差异**：SDK 首条消息与外部并发输入的完整队列时序、`watchPaths` 动态文件监听、插件 Hook 动态注册、异步 Hook 生命周期和全事件取消传播；冻结 Issue #3 的 362 项保持开放。本次只计 **1 项独立用户可感知业务功能**，本轮 100 项先行缺口清单与 100 项补齐目标仍未达成。


## 2026-10-29：文件读后编辑/覆盖与 SDK 已读状态恢复（源码已提交、待行为验收）

- **原版依据**：完整原始 ZIP 的 `source/src/cli/print.ts:3017–3054,2168–2181`（`seed_read_state` 的 mtime 保护与跨查询合并）、`source/src/tools/FileReadTool/FileReadTool.ts:814–846`（读取状态）、`source/src/tools/FileEditTool/FileEditTool.ts:275–309,451–465,519–525`（必须先读、外部修改拒绝、编辑后更新状态）、`source/src/tools/FileWriteTool/FileWriteTool.ts:198–220,281–300,331–336`（覆盖已有文件的读后写保护）。
- **实际 Rust 调用链**：`src/main.rs::sdk_nested_control(seed_read_state)` → `src/agent.rs::Agent::seed_read_state` → `src/tools.rs::ToolExecutor::seed_read_state`；`Agent` 共享文件和 Notebook 已读缓存 → 每轮 `ToolExecutor::read_file/edit_file/write_file/notebook_edit`。新文件写入后也登记可继续编辑的状态；文件内容与此前读取不符、读取范围不完整或显示结果被截断时拒绝直接编辑/覆盖，避免模型误写已变化文件。
- **代码提交**：`ae165e76207a653aa7510a39df903f1b258dff87`、`2206e0f9a8b0f22feea4bf02dff486e5c019df26`、`9743d9bcff011836b5239f6a0d077cb3ef15d90c`、`af2534e9ec3b38cf43a9bd1070275f766d058474`。已在原 Draft PR #2 的 `feat/full-rust-parity` 分支提交。
- **计数**：保守计为 3 条独立用户业务链：①完整读后安全编辑及跨轮保持，②已有文件覆盖前的读后写防误覆盖，③ SDK `seed_read_state` 外部已读状态恢复。Notebook 跨轮读取作为①的相邻状态修复，不额外拆分凑数。**3/100 已提交、待行为验收，剩余 97 项；新增完整等价验收 0**。
- **后续源码复查补强**：压缩或 `/clear` 切换会话后清除旧读取授权（`babac698`）；统一符号链接、相对路径别名的真实文件身份，避免同一文件读取后因不同拼写错误拒绝编辑（`0d8acb61`）；修复手工 Agent 构造的字段初始化（`893dffdc`）。同时按原版 `src/utils/fileStateCache.ts` 恢复 100 条、25 MiB 的 LRU 容量与访问更新顺序（`3ab469bd`、`a1d54375`），避免大量读取使会话内存无限增长。这些均属于上述同一业务链，不另计完成项。
- **剩余差异**：恢复历史会话时从完整对话重建读取状态、原版更细的部分读取/缓存合并时序、文件编码和换行完整保真、原子替换与并发文件竞争、SDK 动态 MCP/插件重载等仍未实现；本轮未预先逐项确认全部 100 个独立业务缺口，不能宣称完成该流程。
- 按本任务现行要求，**没有运行 Cargo、测试或 CI**；仅作源代码接线与提交差异静态核对。Issue #3 冻结 362 项全部保持开放，未改变已验收数量。


## 2026-10-29：FileEdit 原版弯引号容错与排版保留（源码已提交、待行为验收）

- 原版依据：`source/src/tools/FileEditTool/utils.ts:31–36,73–92,104–199,581–657` 与 `source/src/tools/FileEditTool/FileEditTool.ts:313–317,470–475`。原版先尝试精确匹配，再对英文直引号和弯引号做等价查找；实际替换保留文件原有的弯引号样式，普通文本移除行尾空白，Markdown 的双空格换行必须保留。
- Rust：`src/tools.rs::actual_edit_text/preserve_curly_quotes/normalize_edit_replacement/ToolExecutor::edit_file`，已接通真正的文件编辑与读后写校验，Unicode 字符索引按原文件字节边界取实际匹配片段，避免中文、表情和弯引号切片错误。
- 提交：`9eed00bee33d97e367076e151930243de05eab91`、`645510ed51d21a767cf816af7527903005d30ac6`、`f17bb1c92e038a6b5e173efd6c6f121c091ff28b`。本项按一个独立用户可感知的编辑排版业务行为计数，**本轮累计 4/100，剩余 96**；其余细节不拆分计数。
- 剩余：原版 `desanitizeMatchString` 全部转义恢复、完整多编辑输入、特殊编码与行尾格式、UI 差异视图仍待逐项移植。未运行 Cargo、测试或 CI，新增完整等价验收 0；Issue #3 冻结 362 项不勾选。


## 2026-10-29：原版 Read/Edit/Write 三条工具调用链（源码已提交、待行为验收）

- **原版依据**：`source/src/tools/FileReadTool/FileReadTool.ts:227–243,496–507,1019–1036` 的 `Read(file_path,offset,limit)`、`FileReadTool/limits.ts` 的默认 256 KiB 文件限制；`FileEditTool/types.ts:5–19` 的 `Edit(file_path,old_string,new_string,replace_all)`；`FileWriteTool/FileWriteTool.ts:56–65` 的 `Write(file_path,content)`。
- **Rust 真实执行链**：`src/tool_names.rs::canonical_name` 将原版工具名路由到已有 Rust 文件操作；`src/agent.rs` 在 PreToolUse 前执行原版严格输入门禁；`src/semantic_inputs.rs::normalize_source_file_tool` 转换参数并拒绝未知字段、非法类型及未实现的 PDF pages；`src/tools.rs::ToolExecutor::dispatch/read_file_with_mode/edit_file/write_file` 执行真实文件读写，沿用会话级已读状态、文件权限与变更历史。原版 Read 无 limit 时读取整个授权文本文件（256 KiB 上限），其结果不再被通用 16K 字符限制截断。
- **代码提交**：`41404581f044ebe5bc354cff8e88a0d90e44d733`、`d061352e2050c916f644cc1ea0ec870dd7666885`、`223c5e08719b2e8134a7b5a2ab161c425f68b3f0`、`7959a53125f7bbc6cbdc87e67fbad1523c2ac75c`。分别按 Read、Edit、Write 三条独立用户可感知工具链计数；**本轮累计 7/100，剩余 93 项**。
- **剩余差异**：Read 的 PDF/图片/Notebook 多媒体完整输出、25K token 实际计数、重复读取去重、原版格式化行号、各工具完整结构化结果和 UI/SDK 事件、更多输入校验及原版文件状态恢复仍待补齐；不将已有通用执行器的历史实现重复计数。未运行 Cargo、测试或 CI，新增完整等价验收 0；Issue #3 冻结 362 项仍全部开放。


## 2026-10-29：Read 同范围重复读取去重（源码已提交、待行为验收）

- 原版依据：`source/src/tools/FileReadTool/FileReadTool.ts:523–568`。相同文件、相同 offset/limit、同一磁盘修改时间且前次内容仍在上下文时，Read 返回文件未变化提示，节省重复文本和模型上下文。
- Rust：`src/tools.rs::FileReadState/read_file_with_mode` 保存原版 Read 范围信息，区分 Edit/Write/SDK seed 后的缓存；按路径规范化、修改时间和读取范围判断是否复用，读取期间文件变化不登记可复用结果。会话压缩或清空后原有缓存已清除，不引用丢失的上下文。
- 代码提交：`cf83f4b6c4bccc20eb07ca612c16c9fc3e810382`、`1e8b095ad4f0608d7ec3fe1238ac2b248ce03564`、`1b661e191cebeadc864c9cc6fe615a2c5a86dbdc`。作为独立的 Read 去重用户业务行为计 **1 项**，本轮累计 **8/100，剩余 92 项**。
- 原版完整结构化 file_unchanged 输出、恢复历史会话后的缓存重建及动态配置开关仍待补齐。按要求未运行 Cargo、测试或 CI；已提交、待行为验收，新增最终等价验收 0，Issue #3 冻结 362 项继续开放。


## 2026-10-29：Hook 条件执行与工作树删除后的清理恢复（源码已提交，待验收）

- **原版源码证据**：完整 `claude-code-2.1.88-main(1).zip` 的 `source/src/utils/hooks.ts:1390–1420` 定义工具事件的 `if` 条件匹配，`1603–1848` 规定按工具实际输入判定、按条件分别去重；`928–938` 在临时工作树被删除后使用原始工作目录继续启动清理 Hook。
- **Rust 真正调用链**：`Agent::attach_hooks` → `HookRunner::new/with_workspace` → `before_tool_with_context/after_tool_with_context` → `HookRunner::run` → `run_command`。新增 `HookRule.if_condition` 对 JSON 的 `if` 字段反序列化，工具事件按当前 `tool_name/tool_input` 与现有权限规则匹配器过滤，无法证明匹配的条件保持不执行；同命令、同 shell 但不同 `if` 条件独立保留。旧配置缺省 `if` 不受影响，已同步 `src/hooks.rs` 与 `src/agent.rs` 的构造入口。
- **工作树清理**：`HookRunner::new` 保存启动时有效工作目录；执行时如会话工作树已删除，则在原始目录启动 SessionEnd 等命令 Hook，同时继续提供项目目录环境变量；原始目录也无效时显式报告错误，避免静默忽略。
- **提交**：`fbef84db81f60c3a19b247f106afd89eedf47c5d`、`aed233d9e259a0abd9f280a27f6a98500768c857`、`cd6d175d37ef8c651ce6a063af83c2947e9081e6`。按 **2 项独立用户业务行为** 记录：①Hook `if` 条件过滤；②删除临时工作树后的清理 Hook 继续执行。已在远端静态复核新增字段、全部现有 HookRule 构造与真实执行入口；按本任务最新流程未运行 Cargo/测试/CI，**源码已提交、待行为验收**。
- **剩余差异**：原版 `preparePermissionMatcher` 的全部工具专用 AST、复杂 Bash/PowerShell 参数匹配尚需逐函数对照；PowerShell 的 `if` 条件现复用现有独立 PowerShell 权限匹配器（`d233258e`），避免误走 Bash/通用路径；原版 `Read/Edit/Write` 的 `file_path` 已在条件判断前复制转换成 Rust 权限匹配所需的 `path`，原 Hook 输入保持不变（`dcaf34f5`）；Hook 的插件/技能来源隔离、异步执行、HTTP/prompt/agent 回调、动态注册、完整取消与事件时序仍缺。此轮尚未形成经逐项原版与 Rust 验证的 100 项问题清单，100 项目标未完成；Issue #3 冻结 362 条继续全部开放，完整等价验收新增 0 项。

### 2026-10-29：隔离子代理 Hook 的稳定项目根目录（源码已提交，待验收）

- **原版依据**：`source/src/utils/hooks.ts:813–815,881–885` 规定项目根目录在进入 worktree 后不变化；`928–938` 区分临时执行目录与原始工作目录。
- **Rust 调用链**：`Agent::new` 保存 `project_root`；`run_subagent` 和团队成员创建把父代理的稳定项目根目录传给子代理；`attach_hooks` → `HookRunner::with_project_root` → `run_command`，Hook 实际工作目录仍是隔离 worktree，`ATLAS_PROJECT_DIR` 则指向原始项目根目录。避免脚本把 worktree 误当项目根目录，或工作树清理后无法找到项目文件。
- **提交**：`88325593a15f1d0f50982af1599300d31dede60f`、`a1a28bb20f70228ec6dc9b9d40e1f3d581c51e7d`。这是与 Hook `if` 条件、工作树删除后清理恢复不同的**第三项独立用户可感知业务功能补齐**。仅完成静态接线复查，未运行 Cargo/测试/CI；源码已提交、待验收。尚缺跨工作树重启恢复、全部插件/技能 Hook 根目录与环境变量、异步生命周期等。Issue #3 冻结 362 项保持开放，新增完整验收 0 项。


### 2026-10-29：插件 Hook 来源隔离与根目录替换（源码已提交，待验收）

- **原版证据**：`source/src/utils/hooks.ts:813–855,887–908` 在命令启动前检查插件目录并替换根目录变量；`1445–1455,1688–1715,1735–1756` 按插件来源和 Hook 命令去重，两个插件使用相同模板也必须分别执行。
- **Rust 可达链**：`PluginSet::load` → `HookRule.plugin_root`（仅加载器内部赋值，不接受用户 JSON 冒充来源）→ `Agent::attach_hooks` → `HookRunner::run` 的按来源去重 → `run_command`。每个插件使用自身已校验的目录替换 Hook 命令中的插件根目录变量，并向脚本提供 `ATLAS_PLUGIN_ROOT`；插件运行期间被移除时给出可见非阻断错误，避免误用退出码 2 阻断用户操作。普通设置 Hook 无插件上下文，保留旧行为。
- **提交**：`2b1697fbb797ea10538e7efc3bf6ff6a9ffb9d30`、`4ef570c3aa046122c89a10347bbe2ca364e632d3`、`0206d5f136aeb906d6e5cabcaa1e848f7c290047`。按**第 4 项独立用户业务功能**记录，已提交、待行为验收。未运行 Cargo/测试/CI，完整等价新增 0。
- **剩余差异**：插件自定义选项、独立持久数据目录、技能来源、动态 Hook 注册与注销、HTTP/prompt/agent/async Hook、跨平台引号与命令注入边界、完整取消/权限/重启路径仍待对照。Issue #3 固定 362 条保持开放；本轮 100 项问题清单和 100 项代码补齐均尚未完成。


### 2026-10-29：Grep 与 Glob 搜索结果完整性和排序（源码已提交，待行为验收）

- **原版依据**：完整 2.1.88 原始 ZIP 的 `source/src/tools/GrepTool/GrepTool.ts::call`（使用 `--hidden`、仅排除 .git/.svn/.hg/.bzr/.jj/.sl、`--max-columns 500`，没有 2MiB 整文件限制）；`source/src/utils/glob.ts::glob`（`--files --sort=modified`、默认 `--no-ignore --hidden`，且没有对 VCS、插件、构建目录的一刀切排除）；`source/src/tools/GlobTool/GlobTool.ts::call` 实际调用 `glob`。
- **Rust 可达入口**：`ToolExecutor::dispatch("grep_files"/"glob_files")` → `src/search.rs::grep_files/glob_files` → `files` 目录遍历与筛选。此前两工具共享硬编码排除，导致隐藏文件、.atlas、target、node_modules 等合法目标不可见，且 Grep 整文件超过 2MiB 即被跳过；Glob 排序也与原版相反。
- **实际修改**：按工具区分目录遍历：Grep 包含隐藏文件，只排除六种 VCS 目录并取消 2MiB 整文件跳过；Glob 保留隐藏/忽略开关且不再额外排除 VCS、插件、构建目录，按修改时间由旧到新排序。未增加静态生成文件、空壳或厂商绑定。
- **Rust 提交**：`47684bbc618b17595a551d4336a86d7403cf5682`、`b734011b2b8e9b090be11011d6d1d7bec5e1a8de`；迁移矩阵 `a35f011d507898259d4b672f11b5245bbe07cecf`。
- **保守计数**：两个独立用户可感知业务链（Grep 隐藏/大文件内容检索、Glob 文件发现及时间排序）**2/100**。只做源码审阅和远端接线核查，未运行 Cargo、测试或 CI；均为**源码已提交、待行为验收**，新增完整等价验收 0。
- **剩余差异**：Grep 的完整 ripgrep 正则/列宽截断、忽略规则、文件删除竞态、取消/错误时序及多平台路径行为；Glob 的孤立插件版本排除、动态上限、取消和全部 glob 边界。当前仍未逐项确认本轮恰好 100 个独立可处理缺口和完整实施方案，其余 98 项没有本轮代码证据。Issue #3 冻结 362 条全部保持开放。


### 2026-10-29：搜索参数在 Hook 与权限审批之前严格校验（源码已提交，待行为验收）

- **原版依据**：`source/src/tools/GrepTool/GrepTool.ts::inputSchema` 与 `source/src/tools/GlobTool/GlobTool.ts::inputSchema` 使用 `z.strictObject`；Grep 的数字/布尔字段采用 `semanticNumber/semanticBoolean`，未知字段、错误类型、缺少 pattern 必须拒绝。
- **原 Rust 缺口**：`ToolExecutor::dispatch` 在搜索参数结构检查前执行读取权限规则，Agent 还可能先执行 PreToolUse Hook，非法搜索请求也会触发授权与脚本副作用。
- **真实接线修复**：`src/search.rs::validate_tool_input` 检查两工具的原版允许字段、必需参数、类型和输出模式，复用已有语义转换；`src/agent.rs` 在 PreToolUse 之前拒绝无效搜索调用；`src/tools.rs` 在读取权限之前再次校验，直接执行器入口同样受保护。
- **提交**：`f7fd05f0978d4258e523e30aeb063435e975de06`、`cc5bcc027c51537f7057b7ba3068b0fb65244479`、`a24db99891b075485209338f2bcd6c6055508627`。与检索结果完整性、Glob 文件排序不同，保守计为本轮**第 3 条独立用户可感知业务链**，本轮 **3/100**。仅静态检查调用入口与差异，未运行 Cargo、测试或 CI；源码已提交、待行为验收，完整等价验收新增 0。
- **未闭环**：完整原版搜索路径错误提示、取消/竞态、列宽截断、插件版本排除、文件类型映射和全部多平台行为；另外 97 项本轮目标未落实，100 项独立缺口的前置确认与完整实施方案仍未完成。Issue #3 冻结 362 项继续开放。


## 2026-10-29：Grep 搜索输出与混合编码文件补强（源码已提交、待验收）

- **原版源码**：完整原始 ZIP 的 `source/src/tools/GrepTool/GrepTool.ts::call`（`--max-columns 500`、内容/计数/文件路径、上下文分组与分页），以及 `source/src/utils/ripgrep.ts::ripGrep` 的实际文本搜索输出。
- **Rust 实际链路**：`src/agent.rs` 的 `grep_files` 工具 → `src/tools.rs::ToolExecutor::execute` → `src/search.rs::grep_files`。修复 500 字节长行按原版输出省略提示、上下文中断与跨文件分隔符参与分页；非 UTF-8 文本不再因整文件解码失败被遗漏，目录搜索排除含 NUL 的二进制文件，显式二进制文件搜索保留匹配提示；补充常用 ripgrep 语言类型扩展名。
- **真实源码提交**：`b72b803de4350592119a90858de68440f7510d6a`、`e311d9424f9744ab0108838716a0967d25926be7`、`ee329ec4407614a660e32d2204ecb38164f4560a`、`2c7d3ca7312451dcbe8d2a26e8a9377fea4d6c6e`；迁移矩阵同步提交 `847484f5`、`47a7f644`。
- **保守计数**：归为同一条 Grep 用户搜索业务链，**本轮 1/100 项，剩余 99 项**。静态检查了 Agent 入口、工具分发及改动文件；遵循本轮要求未运行 Cargo、单元/集成/端到端测试或 CI，完整等价验收新增 0，Issue #3 冻结 362 项不核销。
- **未实现差异**：完整 ripgrep 类型与忽略规则、取消/超时、流式/二进制边界、文件系统并发与异常传播仍未等价；本轮未完成先行确认 100 条独立功能缺口与其完整实施方案，不将源路径候选数或内部修正数计入完成。
