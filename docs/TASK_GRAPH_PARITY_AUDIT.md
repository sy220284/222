# 任务系统全调用链源代码对照审计

原始来源：用户上传 Claude Code 2.1.88；源代码仅用于独立 Rust 等价重写，不提交原厂源文件。

## 原始文件和真实关联

- src/tools/TaskCreateTool/TaskCreateTool.ts：subject、description、activeForm、metadata；createTask、TaskCreated hooks、阻断删除、工具回复。
- src/tools/TaskGetTool/TaskGetTool.ts：按 taskId 读取，双向依赖、状态、描述、失败时 Task not found。
- src/tools/TaskListTool/TaskListTool.ts：忽略 metadata._internal；展示时剔除已完成 blockedBy，磁盘依赖保持原样。
- src/tools/TaskUpdateTool/TaskUpdateTool.ts：更新 owner、status、activeForm、metadata；完成 Hooks、删除、双向依赖、团队邮箱和验证提醒。
- src/utils/tasks.ts：共享团队任务目录、.highwatermark、锁、CRUD、blockTask、claimTask、resetTaskList、团队状态与重新分配、变更订阅。

## 当前可运行 Rust 范围

src/task_graph.rs：工作区内持久化任务图，任务分文件 JSON、编号高水位、文件列表跨进程排他锁、临时文件原子改名、TaskCreate/Get/List/Update 原版名字/入参、双向依赖、删除级联、metadata null 删除、认领及阻塞与成员忙碌。TaskCreated 阻断回滚，TaskCompleted 阻断拒绝，父子和并发团队共享任务列表。

测试：实际磁盘创建/读取/恢复/删除/依赖；12 线程同步创建；状态和认领分支；模拟模型 HTTP 连续调用四种任务工具并核对工具结果和真实磁盘文件。以 Actions 最终 CI 结果为准。

## 仍未全等的缺口

1. 原版任务目录来自用户配置目录和团队环境，当前使用项目 .atlas/task-lists；多项目团队可见性不同。
2. TaskCreated/TaskCompleted hooks 的进度消息、取消信号、上下文细节和 UI 自动展开尚不完全一致。
3. 任务所有者自动分配、邮箱任务通知、团队队员终止时任务收回、验证 Agent 的提示尚未实现。
4. utils/tasks.ts 的 resetTaskList、getAgentStatuses、unassignTeammateTasks、onTasksUpdated 及诊断降级尚需实现或逐函数核实。
5. 异常时部分修改的原子性、原始排序、无效任务 JSON 的降级、跨进程真实团队及完整权限时序仍需对照。
6. 原版 Vendor 专属分支及强制账号策略按用户要求不引入；通用语义必须保留。

因此以上相关五个原文件均仍标记为部分实现，不宣布任何一个完成正式等价验收。
