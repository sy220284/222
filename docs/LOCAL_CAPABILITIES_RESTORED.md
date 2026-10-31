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
