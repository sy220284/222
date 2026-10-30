# B 组｜工具、Shell、权限、任务与文件：独立进度与交接

> 业务源码独占：`src/tools.rs`、`src/background.rs`、Shell/权限、文件历史、任务/团队、Git/Worktree、搜索与网络工具等 B 组指定文件。共享迁移矩阵、Issue #3 冻结表和 PR 描述仅 C 组可写。此登记按真实用户功能链维护，未通过完整行为验收不计最终等价。

## 2026-10-29：后台 Shell 输出落盘失败的进程与状态闭环（待 CI / 行为验收）

- **原版依据**：完整 `claude-code-2.1.88-main(1).zip` 内 `source/src/tasks/LocalShellTask/LocalShellTask.tsx::spawnShellTask`（任务结束先 flush 输出，再更新状态并通知）；`source/src/utils/task/diskOutput.ts::DiskTaskOutput.#drain`（异步输出错误日志、尝试恢复，避免未处理异常）；`source/src/tools/TaskOutputTool/TaskOutputTool.tsx::getTaskOutputData`（用户可查看任务状态、输出和错误）。
- **已确认原 Rust 缺口**：`src/background.rs::start_with_shell_timeout` 启动两条 `spool_merged` 输出任务，但只在 `child.wait` 返回后检查写盘错误。输出文件写满、被撤销权限或发生 I/O 错误时，读管道的任务提前退出，子进程继续写入并堵塞在满管道，直到超时；TaskOutput 可能误报普通超时或无法显示真实错误。
- **本轮 B 组代码**：`src/background.rs::spool_merged_checked` 在输出读/写失败时立即通知 Shell 监督器；`start_with_shell_timeout` 在进程等待和输出排空两个阶段监听失败，终止 Unix 进程组/Windows 子进程树，区分写盘失败、5GiB 限额、超时与主动停止；`TaskRecord.failure_reason` 兼容读取旧记录，写入终态并通过 `task_output`、`background_list` 返回错误原因，支持恢复会话后诊断。调用链：Bash/PowerShell → `ToolExecutor::dispatch` → `BackgroundTasks::start_with_shell_timeout` → `spool_merged_checked` → 进程终止 → `TaskRecord` 持久化 → TaskOutput/TaskList/Read。
- **提交 SHA**：`3f6d2b465ef6ffa29ce8c005c7e7730204d04c98`（即时终止）、`9f31464c3988a618e38978f0ce296558e87a1ad2`（Linux /dev/full 输出失败回归测试）、`cad6e9f67b777a471af8100c292bf302c0395503`（修复测试通知借用与格式）、`424ba030cc27c52b56417f8ebbcc84da8cbe8e7e`（错误持久化）、`fa4967c70a81350e8c0b0cbdcc7a911136563f82`（失败原因优先级）。全部提交到 `feat/full-rust-parity`，PR #2 保持 Draft。
- **测试标准**：Linux /dev/full 写盘失败且生产者保持管道打开时，监督器必须在 3 秒内收到失败通知；真实后台 Shell 输出文件出现 I/O 故障后进程组必须停止，TaskOutput 与重启后读取的任务记录显示 failed 和写盘原因；普通超时、主动停止、输出 5GiB 上限的错误原因不可互相混淆；Windows 需验证实际进程树与文件失败。
- **验证状态**：已静态核对 B 组新增函数与调用链，测试代码已提交；本环境无 `cargo`/`rustfmt` 可执行程序，仓库无法从当前容器直接联网克隆，故本地 Cargo 编译/行为测试尚未执行。GitHub Actions 在新提交后自动排队，**尚未得到最终绿灯**，不能宣布完整闭环。2026-10-30 02:17 UTC 的旧 CI 运行 `40990789559` 编译失败位于 A 组 `src/provider.rs:229` 调用 `src/sse.rs::complete` 参数数量不一致；该错误发生于本轮 B 组提交前，需 A 组处理。
- **未完成差异**：原版任务输出的主动完成通知、交互输入停滞提醒、前台运行超时后无重启地转后台、完整跨平台取消和恢复仍待逐条对照；本轮输出写盘故障链保持 **已提交、待行为验收**，不提升迁移矩阵或 Issue #3 冻结状态。

## 跨组只读交接

- **A 组**：`src/provider.rs:229` → `src/sse.rs::complete` 的 6 实参/5 形参编译错误（CI `40990789559`，E0061）。A 独占模型文件，请核对最新 HEAD 是否已修复，避免 B 越界。
- **C 组**：`docs/status/222_module_C_sdk_ui.md` 当前未建立；请 C 组创建并维护。前台 Shell 运行到超时自动转后台及 TaskOutput 进度通知需要 C 的 SDK/UI 主入口契约，B 已有 `BackgroundTasks` 接口但未确认完整接线；本项仍未闭环。
- **D 组**：`docs/status/222_module_D_extensions.md` 当前未建立；请 D 组创建并维护。Hook/MCP 的独立任务取消与输出事件需 D 组负责其专属入口，B 不修改 `src/hooks.rs` 或 `src/mcp*.rs`。

## 后续优先续接

1. 首先查看本轮最新 CI 的编译、格式、测试及 clippy 结果；若 B 文件失败，按真实日志修复并复测；A/C/D 文件失败登记准确归属交接。
2. 继续补齐 Shell 运行中的真正前台→后台无重启切换、用户取消到整个进程树、TaskOutput 异步通知与会话恢复，按完整原版业务链验收。
3. 接着审计文件历史的 Shell 文件变更追踪、消息级多文件回退一致性、跨进程锁及安全恢复。每个功能先确认原版调用链和 Rust 可达差异，再编码；无固定数量凑项。

**当前状态：B 组本轮 1 条后台输出故障业务链有实质 Rust 修复及回归测试代码，已提交待验证；新增完整等价验收 0。Issue #3 冻结 362 项保持原状。**
