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
