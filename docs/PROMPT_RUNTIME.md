# 提示词运行链及等价验收清单

目标：把用户提供的 Claude Code 2.1.88 可通用提示词行为接入 Atlas Rust 模型调用链，吸收 2.1.310 对执行证据、信息来源和恢复的强化约束。保持多模型、无需原厂账号、无监测与遥测回传。

## 真实执行路径

1. `src/prompt_runtime.rs` 集中定义主系统提示、协调者、Worker、规划、探索、独立验收、通用委派及会话压缩提示词。
2. `src/agent.rs::system_prompt` 构造主 system，添加工作区路径、`AGENTS.md`、`.atlas/instructions.md` 和可选的 `ATLAS_OUTPUT_STYLE=default|explanatory|learning` 或 `.atlas/output-styles/<名称>.md` 自定义风格。
3. `Agent::ask_with_blocks` 根据子 Agent 角色/团队身份选择专用 system；对于有效的 `.atlas/agents/<name>.md`，加载 frontmatter 后的用户自定义角色指令。角色指令保留在 system 中，独立于任务的 user 消息。
4. 随后按运行状态加入会话上下文、真实环境、会话摘要、Hook 上下文、本地相关记忆与授权目录；动态工具清单仅加入已发现的实际工具。
5. `src/provider.rs` 将 `ModelRequest.system` 传给 OpenAI-compatible 或 Gemini。工具专用提示词作为工具定义的 description 传递。
6. `Agent::compact_history` 发送不带工具的专用摘要请求，保留用户原话、已验证动作、失败原因及待办。
7. 团队 Worker 重启后仍能通过团队身份重新加载 Worker 规则；其他子 Agent 使用当前委派类型决定角色规则。
8. MCP stdio/HTTP 初始化的 `instructions` 通过 `McpManager::instruction_context` 带来源标记与 JSON 转义加入 system，不能授予权限。每一次模型请求都从当前连接状态重新读取（在动态工具重发现之后），断开、禁用或移除的服务器说明不会继续遗留在当前用户轮次的后续请求中。
9. SDK 后台任务通知经 `sdk_queue::notification_model_context` 标记为低信任状态数据，JSON 编码防止通知正文伪造新用户输入。

## 原始源码对照及已接线情况

| 原版提示词领域 | 222 实际实现 | 状态 |
|---|---|---|
| `src/constants/prompts.ts` 主身份/任务/行为边界 | `src/prompt_runtime.rs::BASE_SYSTEM` 和 `agent.rs` | 真实模型请求已接通；全部条件分支尚未逐项等价 |
| `src/utils/systemPrompt.ts` 优先级和角色选择 | `Agent::ask_with_blocks` | 内置和项目角色有真实接线；所有覆盖策略和功能门控尚未等价 |
| `src/tools/AgentTool/built-in/*` 角色指令 | `role_instructions` 与委派/团队调用 | 探索、规划、验收、Worker 已接入；可用工具的强制只读隔离仍按工具执行器权限单独验收 |
| `src/tools/*/prompt.ts` 工具描述 | `src/tools.rs::definitions`、其余各工具定义 | 文件/Git/Shell/Skills 12 个描述已实质补齐，其他工具需逐项核验 |
| `src/services/compact/prompt.ts` | `compact_prompt` 与 `Agent::compact_history` | 独立模型摘要请求已接线，原版各种压缩模式未完全移植 |
| `src/services/SessionMemory/prompts.ts` | `src/memory.rs / src/memory_extraction.rs` | 记忆检索及 system 注入运行，新增显式开启的本地增量提取与安全文件更新；原版 fork-agent 模式与 Dream 整理待验收 |
| `src/constants/outputStyles.ts` | `output_style_label`、`output_style_instructions`、`available_output_styles`、SDK initialize | 三种内置风格已恢复原版名称和详细指令，SDK 实际列表支持用户级及项目级 .atlas/output-styles（项目优先）；原版插件/管理级目录、完整设置交互仍待对齐 |
| `src/utils/messages.ts` 与新版来源区分 | 主提示词、`sdk_queue::notification_model_context`、SDK 事件结构 | 后台任务通知作为低信任状态数据接入；其余来源标记/调度权限仍需补齐 |
| `src/constants/prompts.ts` MCP 指令 | `McpManager::instruction_context`、`Agent::ask_with_blocks` | 已修复同一用户轮次内动态重连/禁用后的旧说明滞留；新增重连/撤销回归测试。原版全部动态来源与策略仍待完整验收 |
| 原版自动模式权限分类器与外部规则模板 | `permission_rules.rs`、`permission_mode.rs` | 静态权限规则运行；模型分类器缺失，Auto 模式保持拒绝，防止误授权 |
| Dream、旁路 Observer、即时提示、Artifact 评论 | `src/memory_extraction.rs::dream_if_due` 和既有源码迁移模块 | 本地 Dream 已有显式启用后的真实 Stop 门控与模型请求；Observer、即时提示、Artifact 评论仍未接通 |

## 验收规则

- 对任何提示词，依次确认：源码对照 → 在 Rust 中存在真实文本/模板 → 对应角色或运行条件可达 → 构造进 `ModelRequest.system` 或实际工具定义 → 模拟提供商捕获断言 → 行为测试/失败边界 → 最新 CI 验证。
- `src/generated_ports` 中的静态 AST 字面量及目录扫描结果都不能直接算作生产接入。
- `cargo fmt --all -- --check`、`cargo check --all-targets`、`cargo test --all-targets` 均以最新 Head 对应 CI 为证据；不能复用被新提交取消的构建。

最近验证：模拟 MCP stdio 初始化及 Agent 模型请求测试均在 CI `40891272435` 中通过；该运行的完整测试失败原因是历史台账部分实现计数与矩阵不一致，已按当前矩阵修正，须以新 HEAD 的 CI 重新验收。此次接入没有启用自动权限模式、后台观察者或自动记忆整理。

本文件只记录已真正接线的内容与真实缺口，不将目前的增强提示词声明为两版所有原文的完全等价移植。

## 2026-10-31：MCP 提示词动态更新修复

- 原问题：`Agent::ask_with_blocks_for_message` 在进入多步工具循环前一次性拼接 MCP 初始化说明，MCP 在循环中重新发现或重连时，后续 `ModelRequest.system` 仍含旧说明。
- 修复：将追加调用下沉到每次真正构造 `ModelRequest` 之前，并在 `refresh_changed_tools()` 之后获取当前 MCP 说明；不额外改变原有工具权限。
- 回归：`src/mcp.rs::dead_stdio_process_reconnects_with_new_tool_schema_before_next_model_turn` 增加首次说明、重连后替换说明、禁用后清空说明断言。
- 验收边界：这一修改只修复动态说明的生命周期，不将 120 处提示词入口或其他未迁移分支标为原版完全等价；构建、全套测试和 CI 以本次提交后的最新 HEAD 为准。

## 2026-10-31：输出风格发现与 SDK 初始化一致性

- 修复前：SDK initialize 无条件返回 `output_style=default`、`available_output_styles=[default]`，即使实际启用其他内置/自定义风格。
- 修复后：`prompt_runtime::available_output_styles` 只枚举真正能够加载的用户或项目 Markdown 风格，项目同名风格优先，保留三种内置风格（default、Explanatory、Learning）并对自定义风格排序，跳过空内容、无效 frontmatter、越界符号链接；SDK initialize 读取实际 `ATLAS_OUTPUT_STYLE` 并在启用项不可用时给出明确错误。
- 验证：已增加目录排序、无效文件、越界符号链接、每个返回风格都可成功加载的回归测试。尚未恢复原版所有用户级风格目录与交互配置入口。

## 原版输出风格源码增量核对

- 原版 `src/constants/outputStyles.ts`：`Explanatory` 和 `Learning` 为有独立长篇提示词的内置风格，已恢复名称及主要教学规则（不能以此宣称逐字完全等价）。
- 原版 `src/outputStyles/loadOutputStylesDir.ts`：同名项目风格覆盖用户风格，已在本地 `.atlas/output-styles` 两级路径实现并以临时双目录测试覆盖；插件/管理级风格与 frontmatter 自定义名称另需补足。
- 原版风格中的注入条件与 `keep-coding-instructions` 差异，须继续结合 `src/constants/prompts.ts` 和实际 provider 捕获验收。

## 2026-10-31：记忆提取主运行链（显式开启）

- 原版依据：`services/extractMemories/extractMemories.ts` 由完整 Stop 后触发，跟踪增量消息、主 Agent 直接记忆写入时跳过，并在成功后保存游标。
- Rust 实际执行：`src/memory_extraction.rs::extract` 在 `Agent::ask_with_blocks_for_message` 正常结束后调用，单独向当前 API 模型发送无工具结构化请求，解析后仅可写入本工作区 `.atlas/memory/*.md`，并更新 `MEMORY.md`。由 `session.memory_extraction_cursor` 保存进度，压缩或清空消息时重置。
- 开关：仅显式设置 `ATLAS_AUTO_MEMORY_EXTRACTION=true` 时运行，默认关闭；不会启动定时任务、派生子 Agent 或远程服务。调用失败只影响可选提取，保留主回答与原游标。
- 约束：全量检查模型产出的所有文件名/类型/长度，拒绝目录逃逸、符号链接、重复文件名和索引预算溢出；每轮至多四条。文件采用临时文件并原子替换。
- 剩余源码差异：原版复用主 Agent 提示词缓存的 forked agent、多轮工具隔离、团队共享记忆、停止钩子后台通知及 Dream 尚未完整对齐，本条只标记部分实现。

## 2026-10-31：Dream 本地整理链（显式开启）

- 原版依据：`services/autoDream/autoDream.ts` 的触发顺序为时间门 → 最近会话数门 → 独占锁 → 记忆整理；默认要求距上次整理至少24小时、至少五个其他会话更新。
- Rust 运行：`ATLAS_AUTO_DREAM=true` 时，主 Agent 完成一次真实回答后检查本地 `.atlas/sessions` 与 `.atlas/memory/.dream-consolidated`，满足门限后尝试独占锁，向当前 API 模型提交既有记忆和五个近期会话摘录，模型仅返回数据，写入沿用记忆提取的路径与类型校验、原子文件保存。成功才更新时间戳；失败保持可重试。没有新增系统定时任务或独立 Agent。
- 验证用例：不足五会话不调用模型；达到五会话后运行并刷新时间门；同一天重复调用被抑制；异常 JSON 不推进时间门。
- 源码差异：原版 forked-agent、工具沙箱、实时状态/取消、共享团队记忆、完整四阶段提示词中的日志检索与删除策略仍需等价；两条原未迁移源路径维持「部分实现」，尚未标记完整验收。

## 2026-10-31：2.1.88 可直接复制的运行提示词实链对齐

- 系统动作安全提示：`src/constants/prompts.ts::getActionsSection` 原版 2832 字符无改写保存在 `src/prompt_runtime.rs::SOURCE_ACTIONS`；`agent.rs::system_prompt_with_dirs` 每次真正拼入请求，在运行时将原厂 `CLAUDE.md` 路径替换成多模型构建的 `AGENTS.md`。用户授权边界和风险确认仍由实际执行器落实。
- 原版 Reminders/Hooks 原文也已保存在同模块；为避免把不可信工具结果提权为真正系统指令，Reminders 不直接进运行时 system。保留原文不等于该分支已完全等价。
- 内置 Explore、Plan、general-purpose：恢复各自 2.1.88 的核心角色提示词正文和只读限制，源品牌替换为 Atlas、工具占位符展开为 Rust 真实工具名，经过 `role_instructions` 加入子 Agent 请求；原版验证 Agent 的完整长文本以及动态条件仍待逐字接入。
- 搜索工具：`src/search.rs::definitions` 将 Glob/Grep 提示词作为真正的模型函数 description，而非只放进静态生成模块；Grep 模板中的工具名称按实际 `grep_files`、`run_command` 映射，含正则表达式转义。
- 会话压缩：`src/services/compact/prompt.ts::NO_TOOLS_PREAMBLE` 原文已前置到实际独立摘要请求；`compact_summary_content` 只保存可解析的 `<summary>` 正文，普通文本仍受支持；完整原版 BASE/PARTIAL 长提示词、摘要变体仍待接入。
- 新增运行路由断言：真实 `system_prompt` 含源动作提示，工具定义内容与源文本对照，角色选择和无工具压缩摘要均有测试。当前不把 120 处“部分实现”擅自提升为“完整等价”。
 
## 2026-10-31：原始提示词一次提取和可执行目录

- 源码索引：`docs/original_prompt_literals_2_1_88.jsonl`，按用户提供的 2.1.88 压缩包与来源索引生成，包含120个源码路径、297段字符串候选、原文件 SHA-256、来源分类、以及两项明确排除的远程功能。
- 生成入口：`scripts/import_original_prompt_literals.py`；新版本只需重新运行并比较来源哈希，无须逐字手工搬运。**原文候选条数不等于运行链完工数**，被截断的动态模板、条件渲染和函数调用必须单独验收。
- Rust 读取：`src/source_prompt_catalog.rs` 编译期嵌入来源 JSONL，Glob/Grep 工具定义直接消费原版描述（只替换实际工具名），四种内置角色的完整原版静态模板由目录读取、限定变量替换后返回 `role_instructions()`。
- 压缩完整原文：`src/compact_source_prompts.rs` 逐字保存 2.1.88 的 BASE 4220 字符、PARTIAL 2362 字符、PARTIAL_UP_TO 2326 字符，以及两种分析指令全文。普通压缩请求无旧摘要时使用 BASE，有旧摘要时使用 PARTIAL；UP_TO 保留原文但尚无对应部分消息保留运行入口。所有模型压缩请求仍无工具，并只保存 `<summary>` 主体。
- 当前待对齐：主系统提示词更多动态分支、工具描述的完整可达列表、权限分类器、针对团队/协调模式的运行条件、Web/浏览器专用提示、技能模板动态变量与提取/整理记忆的完整原版指令。只在源码保存、不实际出现在 `ModelRequest` 或工具定义里的条目继续标记为部分或未迁移。
