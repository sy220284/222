# 222｜A 组 Agent、模型与上下文源码补齐记录

> 独占业务源码：src/agent.rs、src/agent_definition.rs、src/ask_user_question.rs、src/provider.rs、src/prompt_runtime.rs、src/conversation_context.rs、src/context_usage.rs、src/memory.rs、src/memory_age.rs、src/multimodal.rs、src/cost.rs、src/gemini_sse.rs、src/sse.rs、src/query_guard.rs、src/semantic_inputs.rs、src/side_question.rs、src/stats.rs。
>
> 本文件只由 A 组维护。共享迁移矩阵、全局进度、Issue #3 冻结 362 项和 PR 描述只读。所有下述成果均为**已提交、待行为验收**，未运行 Cargo/测试/CI，完整等价验收新增 **0**。原版 1,906 条路径、非排除 1,884 条及冻结 362 条范围不变。

## 本次同步与交接检查

- 只读检查 Draft PR #2 的 feat/full-rust-parity HEAD、Issue #3、docs/FILE_PORT_MATRIX.md、docs/PORT_STATUS.md、docs/ISSUE3_FROZEN_362_AUDIT.md 和最近提交。PR 仍为 Draft，未修改 main、未新开或合并 PR。
- 读取资料库完整 `/claude-code-2.1.88-main(1).zip`，实际对照 `source/src/services/api/claude.ts`（流式错误、输出上限和空闲超时）、`source/src/services/tokenEstimation.ts`（多种消息块的粗略 token 计算）及关联调用链。多模型 SSE 的传输格式为厂商中立适配，保留原版流式中断、错误与完成判定语义。
- 本轮检查时 `docs/status/222_module_B_tools.md`、`docs/status/222_module_C_sdk_ui.md`、`docs/status/222_module_D_extensions.md` 尚未出现在远端 docs/status 目录，无法从这些指定登记读取交接项；已查看最新 commits 和共享台账作为替代。未触碰其他组文件。
- 并行分支持续有其他任务提交，写入采用每文件最新 blob SHA、冲突后重读重试，无强推；对 `src/agent.rs` 等重叠文件的其他任务修改予以保留。

## 本轮确认的七项独立用户业务缺口与真实源码提交

| # | 用户可感知行为 / 原版依据 | Rust 可达链、真实修复与提交 SHA | 未实现差异 / 状态 |
| --- | --- | --- | --- |
| A-01 | 模型已流出部分文本/工具调用后通过 SSE 发送 error，原版 `services/api/claude.ts` 的 streamingError 路径将错误上抛，阻止把失败回合作为成功。 | `src/sse.rs::consume_frame → Assembler::consume → complete → provider::OpenAiCompatible::complete_stream → Agent`；识别带 error 字段及 `event: error` 的帧，报出受限错误详情并中止；`466ea550b532c065ab8af4a6bed12774e59a150a`。 | OpenAI 兼容供应商的所有自定义 error 格式尚未覆盖；待行为验收。 |
| A-02 | 原版 `services/api/claude.ts:2337–2363` 对流异常结束进行检查；Gemini SSE 原先意外断线也会将部分回复/工具调用当作成功。 | `src/gemini_sse.rs::frame → Accumulator → complete → provider::Gemini::complete_stream → Agent`；追踪显式 [DONE] 或 finishReason，意外 EOF 报错，不执行未完成工具调用；`b91ffe24d727b6ec56cf4fd9a05fb27fff5360f0`。 | Gemini 特定中断/安全终止原因的完整映射待验收。 |
| A-03 | 原版 `services/api/claude.ts:2266–2291` 对 max_tokens/context_window_exceeded 发出截断错误；Rust 原来仅在没有 tool_calls 时阻止截断回合，可能执行半截工具调用。 | `src/agent.rs::ask_with_blocks → ModelProvider → finish_reason → 工具分发`；统一在工具执行前阻止 length/max_tokens/MAX_TOKENS/context overflow，先保存消息、用量及墙钟耗时；`4ec3292cc660b29013611158943c522be1e2d51e`。 | 自动续写/恢复及供应商其他截断原因待验收。 |
| A-04 | 原版 `services/tokenEstimation.ts::roughTokenCountEstimationForBlock` 统计 thinking、redacted_thinking、server_tool_use 和搜索结果；Rust /context 原先把这些消息块算作 0。 | `src/context_usage.rs::block_tokens → Snapshot::count_messages → /context`；增加思考块、脱敏思考块及其他文本类块的 token 估算；`7eba4ebf7140accabcde539be5e9908360fb061c`。 | 精确供应商 token-count API、复杂媒体计数与自动压缩阈值尚需核验。 |
| A-05 | 原版 `services/api/claude.ts:1868–1927` 采用每次接收数据重置的 90 秒空闲 watchdog；Rust 以前统一 180 秒**总** HTTP 超时，会中断持续输出的长推理。 | `src/provider.rs::create/stream_idle_timeout → src/sse.rs::complete / src/gemini_sse.rs::complete`；流式请求只限制连接及响应头/块的空闲时间，非流式仍保留 180 秒期限；可用 `ATLAS_STREAM_IDLE_TIMEOUT_MS` 配置空闲阈值；`a764c234e68c96de710e013d2317027eb296cc04`、`f9f9d69f79fa80453dcff4d07b2d9327debcd85b`、`9308e321551204ac1e8762186fe3276f71d9c994`。 | 90 秒警告 UI、SDK 取消与长时间重试策略尚未等价。 |
| A-06 | 原版按事件处理流式响应；Rust 以前在**解析**前对一次网络读取的总缓冲限制为约 4 MiB，大量正常短帧合并送达时被误判为单帧超限。 | `src/sse.rs::complete`、`src/gemini_sse.rs::complete`；先消费完整 SSE 帧，再限制剩余未完成帧，保持内存界限；`59a7a53c6a3889bf46f8f094855efd72804d6e54`、`47de5cf04e6f3e8aa48493dbeae6461f382b2ea7`。 | 超大单帧增量解析及跨平台真实大流量行为仍待验收。 |
| A-07 | 原版 `source/src/services/api/errors.ts:1184–1207::getErrorMessageIfRefusal` 会将明确的模型拒绝显示为可见错误；Rust 以前在 provider 返回 refusal/content_filter/SAFETY 时可能把空回答当成功，或继续执行已输出的工具调用。 | `src/agent.rs::ask_with_blocks → finish_reason → 工具分发`；识别通用 refusal/content_filter 及 Gemini 安全终止原因，保存会话与耗时、反馈明确错误并阻断工具；`e0129e476afd0323f6f3e036bc03a128702975b7`。 | 完整供应商安全原因及 UI 错误样式待验收。 |

## 待其他模块接线

本轮七项修改均在 A 组已有的模型请求、Agent、上下文可达执行链中，无已确认的跨模块必需接线。SDK/UI 对长流空闲警告、模型截断恢复提示等完整展示涉及 C 组 `src/main.rs`，尚需先核对原版 UI 事件格式后提供精确接口；目前不把这部分计为完成。

## 本轮硬目标与准确续接

- **已确认并提交：7 / 100 个独立用户业务功能，待行为验收；剩余 93 项。** 文档更新、关联改动、多个提交均不重复计数。新增完整等价验收 0 项。
- **流程 A/B 尚未达成：** 未能预先完成恰好 100 项逐条双侧验证的缺口清单及覆盖方案；严禁把仅由文件名或原版函数数推断的条目凑作已确认问题。下一次先读本文件和其他组新建的交接登记，复用本次已核实的原版源码，再继续排查剩余 93 项真实 A 组缺口、形成依赖顺序并编码。
- 下一步优先核查：模型流错误后的可见反馈与重试安全、供应商中断原因、思考/工具多内容块、长流取消传播、上下文真实压缩阈值及模型路由错误恢复；逐项确认原版函数、Rust 实际入口和缺失后才能计数。
- 不修改共享台账、Issue #3 或 PR 描述；保留其他组的并行提交。无 cargo fmt/check/clippy/test、Actions CI、单元/集成/端到端运行记录。
