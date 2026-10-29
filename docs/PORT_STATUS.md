# Rust 全量迁移实况（基于逐文件台账）

**目标：1906 个原始文件的功能逐项重写、等价验证。**

- 明确归入环境/行为遥测删除项：22 个原始文件路径（仅是第一轮整文件分类；其他混合文件也需排除隐式上报）。
- 现有 Rust 模块覆盖部分行为、但未完成逐文件等价：363 个原始文件路径。
- 当前台账仍标注“未迁移”的文件路径：1492 个（与部分实现项互斥，按当前状态字段分类）。
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
