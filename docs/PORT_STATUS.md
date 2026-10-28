# Rust 全量迁移实况（基于逐文件台账）

**目标：1906 个原始文件的功能逐项重写、等价验证。**

- 明确归入环境/行为遥测删除项：22 个原始文件路径（仅是第一轮整文件分类；其他混合文件也需排除隐式上报）。
- 现有 Rust 模块覆盖部分行为、但未完成逐文件等价：359 个原始文件路径。
- 当前台账仍标注“未迁移”的文件路径：1494 个（与部分实现项互斥，按当前状态字段分类）。
- **达到原始文件纯函数行为/边界等价且有验证证据的路径：31 个（包括独立纯函数、状态模块与固定常量文件；完整调用链仍需逐项验证）；其上下游业务逻辑仍需完整验证。** 其他当前已实现的功能仅在特定测试场景验证，不能把“部分实现”升级为“完成”。
- 非排除项的原始文件级等价验收仍未闭环：1853 个。

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
