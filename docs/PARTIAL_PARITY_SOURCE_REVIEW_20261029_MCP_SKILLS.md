# 2026-10-29：MCP 提示词附件与技能执行链源码差异（持续补齐）

本记录仅统计有实际 Rust 代码改动且成功推送的用户可感知功能。原始包：资料库根目录 `claude-code-2.1.88-main(1).zip`，`source/` 下完整 2.1.88 源码。开发分支 `feat/full-rust-parity`，仅对应 Draft PR #2。未运行 Cargo、单元测试、端到端测试或 CI；所有项目为**源码已提交、待行为验收**，Issue #3 冻结 362 条保持开放。

## 本轮真实补强：4 个独立业务功能（100 项硬目标尚缺 96）

1. **用户调用 MCP 提示词时接收服务器附带的资源**：原始 `src/services/mcp/client.ts:2478–2590` 中 `transformResultContent` 的 `resource` 与 `resource_link` 分支，以及 `fetchCommandsForClient:2033–2096`。Rust `src/mcp_prompt.rs::render_content` 原来对这两种块直接返回错误，模型拿不到资源文本、图片和链接。现通过 `Agent::expand_mcp_prompt_command → mcp.get_prompt → render_content → Agent::ask_with_blocks` 传递资源文本、图片 base64、链接名/URI/说明；不支持的非图片二进制附件仍明确报错，避免内容丢失。提交 `6df09be5ac097c1946423bf7d6749400564ff5d5`。**仍缺**：原版非图片二进制持久化、图片缩放/尺寸限制、音频附件、完整多模态和跨服务取消/重连。
2. **技能 frontmatter 参数列表的注释和空行处理**：原始 `src/skills/loadSkillsDir.ts:447` 使用 frontmatter 解析后调用 `src/utils/argumentSubstitution.ts:50–74`；Rust `src/skill_arguments.rs::frontmatter_argument_names` 在 YAML 列表中遇到注释/空行会提前结束，且会错误读取嵌套 `arguments`。现跳过列表内部注释/空行、限定顶层字段，经真实 `ToolExecutor::read_skill` 入口使用。提交 `7cc5c73723935dd3e7bb0fdff2d232d3b5791a0c`。**仍缺**：完整 YAML schema 解析、复杂引号/折叠多行和全部 frontmatter 字段。
3. **技能执行时隔离 YAML 元数据、向模型提供技能目录上下文**：原始 `src/skills/loadSkillsDir.ts:344–369` 将 `markdownContent` 与技能根目录组合后替换参数；Rust 原先直接把整个 SKILL.md 连同 frontmatter 当作模型指令，缺少目录上下文。新增 `src/skill_arguments.rs::markdown_body`，`src/tools.rs::read_skill` 对本地和启用的插件技能读取后，剥离闭合 YAML 头，增加真实技能根目录，并提供厂商中立的 `${SKILL_DIR}` 占位符。代码提交 `041d12414df742ef20541dbd3fda0424f9f9aeec`、`55b98e73b7815b9b0514189363b0200c36fb6683`、换行修复 `2ce63316cb12eddc7db9c141a113e5f50e07f84f`。**仍缺**：会话 ID 占位符、技能内命令执行、插件来源信任规则、动态 UI/Hook 生命周期。

4. **技能运行时会话身份占位符**：原始 `src/skills/loadSkillsDir.ts:363–368` 在技能参数替换后注入当前会话 ID。Rust `ToolExecutor::read_skill` 之前完全缺失会话上下文，技能无法在指令中引用当前会话。现新增 `ToolExecutor::with_session_id`，由真实 `Agent::ask_with_blocks → ToolExecutor::new` 使用当前 `self.session.id` 接线，读取本地/插件技能时展开中立 `${SESSION_ID}`；独立执行器无会话上下文时保持原文，避免编造身份。提交 `f0cbe1a5426c6bcbc120efa5d13b4657ed3fb9fa`、`0be6d6b52f57b4860796050d1703ba98f0ff28d8`。**仍缺**：原厂旧占位符兼容取舍、UI 交互调用与技能嵌套执行时序。

## 续接与计数规则

- 本轮有效源码补强：**4/100**；尚未补齐 **96**；达到全链最终等价验收：**0**。
- 继续从 Issue #3 固定路径和全量映射找独立用户行为，按真实源码/可达 Rust 入口逐项核对。不得将上述一个功能的多个分支或多个提交重复计数。
- 由于本轮未做行为验证，不能把上述代码视为最终等价，不能勾选 Issue #3 冻结清单；后续独立验收任务需按正常/异常/权限/取消/重启场景测试。
- 后续优先补齐：MCP Prompt 非图片二进制保存与图像缩放；技能的内联命令、权限隔离与完整会话恢复；完整 YAML frontmatter 解析。
