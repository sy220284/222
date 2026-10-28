# Rust 全量迁移实况（基于逐文件台账）

**目标：1906 个原始文件的功能逐项重写、等价验证。**

- 明确归入环境/行为遥测删除项：22 个原始文件路径（仅是第一轮整文件分类；其他混合文件也需排除隐式上报）。
- 现有 Rust 模块覆盖部分行为、但未完成逐文件等价：349 个原始文件路径。
- 当前台账仍标注“未迁移”的文件路径：1508 个（与部分实现项互斥，按当前状态字段分类）。
- **达到原始文件纯函数行为/边界等价且有验证证据的路径：27 个（包括独立纯函数、状态模块与固定常量文件；完整调用链仍需逐项验证）；其上下游业务逻辑仍需完整验证。** 其他当前已实现的功能仅在特定测试场景验证，不能把“部分实现”升级为“完成”。
- 非排除项的原始文件级等价验收仍未闭环：1857 个。

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
