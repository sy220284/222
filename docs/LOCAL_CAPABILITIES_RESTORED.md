# 任意模型可用的本地通用能力恢复（2026-10-31）

## 恢复清单与命令

- 原 `src/commands/review/ultrareviewCommand.tsx`、`src/commands/review/ultrareviewEnabled.ts`：恢复 `atlas-agent ultra-review [PR]` 和 `/ultrareview [PR]`，使用当前模型执行深度本地代码审查，要求完整读源码、调用链、边界和测试；在只读计划权限下运行。
- 原 `src/commands/ultraplan.tsx`、`src/utils/ultraplan/keyword.ts`：恢复 `atlas-agent ultra-plan "目标"` 和 `/ultraplan 目标`，使用当前模型和本地工程工具生成可验收计划，启用只读计划权限。
- 本地附件：SDK `user` 消息增加 `file_attachments` 或 `fileAttachments` 输入，可传工作区内真实文件，文本 UTF-8、图片 PNG/JPEG/GIF/WebP，转换为既有多模型内容块，不调用 Bridge 或下载服务。

SDK 示例：

```json
{"type":"user","content":"请检查源码和示意图","file_attachments":[{"path":"src/main.rs"},{"path":"assets/design.png"}]}
```

安全边界：附件最多 8 个、单文本 1 MiB、单图片 5 MiB、总计 12 MiB；解析符号链接后的实际路径必须处于工作区，旧的 `file_uuid`、URL、厂商凭据字段均不能作为远程下载。错误要阻断模型请求。

上述四条原路径目前登记“部分实现”，不凭新增入口、提示词和单测宣称与 2.1.88 完全等价；继续逐函数对照原版 UI、状态、调用语义和全链路验收。原厂 UltraReview 配额、UltraPlan CCR 会话、云端任务、远程 SDK、原厂 GitHub App/Slack 安装器等仍保持排除。普通本地任务、Git、MCP、Hooks、文件、会话等已保留，优先复用。

## 验收指令

`cargo fmt --all -- --check`、`cargo test --all-targets`、`python3 scripts/port_workbench.py --help`，并核实 PR #6 最新 HEAD 的 GitHub Actions。 

## 持久化本地任务触发

原 `src/tools/RemoteTriggerTool/{prompt.ts,RemoteTriggerTool.ts,UI.tsx}` 的通用任务触发能力重新纳入源码对齐，以 `src/local_triggers.rs` 实现不依赖原厂云端的明确执行入口。

- `atlas-agent trigger set review_check 检查代码安全与单元测试`
- `atlas-agent trigger list`
- `atlas-agent trigger run review_check`
- `atlas-agent trigger delete review_check`
- 交互会话可用同名 `/trigger set/run/list/delete`。
- 本地存储与源代码无关的提示词，执行时直接使用当前配置的模型和 Agent 工具权限；不调用云端 Worker，不会主动执行或添加定时任务。

截至本次补充，排除 192 条，纳入对齐 1,714 条；上述 7 条重新纳入路径全部按“部分实现”继续源代码级验收。

## 2026-10-31 源码级复核修复

已从资料库原始 `claude-code-2.1.88-main(2).zip` 逐文件核对 `source/src/utils/ultraplan/keyword.ts`、`commands/ultraplan.tsx`、`commands/review/ultrareviewCommand.tsx`、`tools/RemoteTriggerTool/{prompt.ts,RemoteTriggerTool.ts,UI.tsx}` 与 `bridge/inboundAttachments.ts`。

- 新增 `src/local_advanced_keywords.rs`，覆盖原版的有效关键词边界、成对引号及括号、代码路径、疑问句、斜杠命令跳过及仅替换首个 `ultraplan` 的行为，交互聊天走真实模型执行入口。
- 本地高级规划/审查改为只在**该次请求**使用计划权限，退出或异常时恢复此前用户权限，避免原实现通过 `set_sdk_permission_mode` 永久更改会话。
- 多模态 SDK 本地附件在最后一个原始文本块之前插入，修复附件导致最后文本块/斜杠命令无法识别的问题；SDK 从最后一个**文本块**读取命令，而不假定最后一块一定是文本。
- 本地任务支持源版五类动作对应的 `create/get/update/run/list`，以及额外的 `set/delete`；严格区分创建已存在与修改不存在的报错。本地任务仍仅为用户显式调用，**没有云端调度、周期触发和自动派生 Agent**。

**尚未等价的部分（不可标记 verified）：** 原版高级审查、规划的原厂计费/云端会话及远端执行已按用户要求排除；原版 UltraPlan 的多阶段独立探索、seedPlan 细化、状态/取消/计划回传 UI 尚缺本地完整等价实现。原压缩包未包含其 `source/src/utils/ultraplan/prompt.txt` 原文。RemoteTrigger 的 `isReadOnly`、模型工具调用入口、输出结果格式及调度元数据尚未完成本地等价，当前 CLI 是独立用户入口。SDK 的关键词自动高级工作流也仍待单独完成贯通。全部保持**部分实现**，不以新增提示词或 CI 通过替代等价验收。

### 本地任务存储隔离补强

任务提示词会写到当前工作区的 `.atlas/local-triggers/tasks.json`，并在同目录生成独立的 `.gitignore`（忽略目录下的任务数据和锁文件）。因此即使目标工程自己的 `.gitignore` 未配置，也不会通过普通 `git add` 意外提交。首次写入时兼容读取旧路径 `.atlas/local_triggers.json`，持久化成功后移除旧文件。

## 2026-10-31 模型工具全链复核

- **原版位置：** `source/src/tools/RemoteTriggerTool/RemoteTriggerTool.ts` 是 Agent 模型工具，支持 `list/get/create/update/run`，`list/get` 为只读，修改及运行需要正常工具权限；原厂 OAuth、CCR 定时服务和独立远端 Agent 按已授权范围继续删除。
- **此前断点：** Rust 的 `src/local_triggers.rs` 仅由 CLI 和交互命令调用，`src/tools.rs::definitions()` 未发布工具定义，模型无法自行发起这些通用本地操作。
- **修复调用链：** `src/local_triggers.rs::model_definition()` → `src/tools.rs::definitions()` → 当前模型提供商的工具定义 → `ToolExecutor::validate_call_before_hooks` → `ToolExecutor::dispatch` → `local_triggers::model_call` → 独立任务存储。更新/创建与执行请求通过原有 `require_approval`，只读计划模式会拒绝写入或运行；超范围调度参数明确报错。
- **模型工具 run 的边界：** 返回当前会话待完成的保存指令，继续由同一 Agent 工作；没有承诺额外执行器或自动后台启动。CLI/REPL 的 `trigger run` 仍然直接交给用户选定模型执行。
- **发现并修复隐私缺口：** 之前 `get/list` 可沿工作区 `.atlas` 目录或任务文件的符号链接读取其他工作区。现在统一校验所有存储读写路径并拒绝指向工作区外的链接；增加 Unix 符号链接拒绝测试、非法参数和审批拒绝测试。
- **真实工具链验证：** 在 `tests/integration_cli.rs` 中通过模拟 OpenAI 兼容 API 让模型先发 `LocalTrigger.create` 再 `LocalTrigger.get`，确认工具注册、参数、权限、真实磁盘持久化、结果返回和模型下一轮消费；测试结论以本次提交的最新 GitHub CI 为准。

**尚未等价：** 原版 UltraPlan 多阶段状态机及 seedPlan 草稿、UltraReview 原厂云端回填、模型在 SDK 下的高级关键词路由、LocalTrigger 的原版定时元数据与异步调度（已明确排除）、工具运行后的多阶段任务状态和 UI。以上不能因为 CLI 或 Rust CI 通过而标记成原版完整验收。范围仍为原版 1,906 条、排除 192 条、需持续对齐 1,714 条。


## 2026-10-31 SDK 高级工作流入口继续对齐

本次沿「SDK user 消息 → 内容块校验/附件解析 → 高级命令与关键词路由 → 单轮计划权限 Agent → 当前配置模型 API → SDK 结果事件」补齐实际断开的调用链：

- local_advanced_workflows::sdk_user_prompt 统一解析 /ultraplan 目标、/ultrareview [PR] 及普通文本中的有效 ultraplan、ultrareview 关键词；未识别的斜杠命令继续走原有 SDK Skill/MCP 解析。无效参数直接产生 SDK error，不把错误指令送给模型。
- SDK 只读取用户消息的最后一个原始文本块作为命令候选；替换为经过实际文件/Git 检查的本地工作流提示词，同时保留同一消息之前的文本、图片、本地附件及原 UUID。skipSlashCommands=true 时按原有 SDK 契约完全跳过该项路由。
- 新增 Agent::ask_with_blocks_in_plan_mode，将高级工作流限定在本次请求的只读计划权限下，保留图片与消息身份，模型失败或 SDK 中断后恢复之前的权限，不持久改变用户设定。
- 新增真实模拟模型 HTTP 的 SDK 端到端测试，覆盖无效高级命令、图片与附件不丢失、合法高级规划生成和跳过命令标志，并增加 SDK 路由边界与错误后权限恢复测试。

以上仍只推进 **4 条高级审查/规划及关键词源码路径** 的本地等价接线；另外 3 条 LocalTrigger 路径之前已接通本地模型工具、审批与存储链。**7 条全部继续保持“部分实现”**。原版 UltraPlan 的多阶段状态、seedPlan、进度/取消回传 UI，UltraReview 相关本地 UI 与差异回收，LocalTrigger 的交互展示/结构化结果仍需独立源码级比对与验收；用户已授权排除的原厂云端和远程调度仍不纳入恢复。

本轮的真实测试结论应以其对应 PR6 GitHub 提交的 CI 为准，不以前一提交或测试代码存在代替完成验收。
