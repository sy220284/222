# 2.1.88 原版全量函数 AST 独立复核（2026-10-31）

## 已独立运行的源码核验

输入为用户私有原版 `claude-code-2.1.88-main(2).zip`（SHA256 `23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`），以及 `Claude-Code-2.1.88-全源码逐行复核与复刻规格-最新版.zip`（SHA256 `3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93`）。

从 ZIP 提取第一方 `source/src` 的 1,902 个文件，在本地使用 TypeScript 编译器 AST `createSourceFile` 重新解析每一个文件，递归遍历 `FunctionDeclaration`、`MethodDeclaration`、`ArrowFunction`、`FunctionExpression`、`Constructor`、`GetAccessor`，与规格包 `核验数据/全函数与方法核查.tsv` 逐项匹配**原文件、节点类型、首行与末行及出现次数**。

| 复核项目 | 独立实测 |
|---|---:|
| TypeScript / TSX / JS 原始文件 | 1,902 |
| 语法解析报错 | 0 |
| 原版 AST 函数与方法节点 | 20,788 |
| 规格 TSV 函数与方法记录 | 20,788 |
| 节点类型、源码行范围及数量匹配 | 20,788 |
| 原版独有但规格漏列的节点 | 0 |
| 规格独有但原版没有的节点 | 0 |
| 同一行、同类、同起止行的不同函数记录 | 87 条重复定位现象，按独立节点保留 |

同一行上的多个匿名回调可以有相同起止行和相同显示名称；这 87 条记录具有实际 AST 节点，不得按名称、文件和行号强行去重。**统一索引采用 `源文件 + AST 类型 + 起止行 + 同位置出现次序` 作为独立函数标识**，详见 `scripts/source_function_index.py`。有 87 个第一方文件不包含这六种函数节点，仍必须进行文件级常量、类型、导出和运行入口复核；4 个 vendor 接口独立纳入。

## 语义风险分类与待处理缺口

AST 记录里累计有：205,560 个静态步骤、40,948 个分支节点、23,380 个显式返回、938 个抛错、1,838 个捕获、5,238 个 await、1,209 个状态写入。数字仅来自原版 AST 静态索引，能够帮助确认分析覆盖范围，**不直接表示这些分支在 Rust 中已正确实现**。

特别需要按实际运行和取消、恢复边界核对的源码包括：

| 原版源文件及函数 | 位置 | 静态步骤 / 分支 | 需保持的验收对象 |
|---|---|---:|---|
| `main.tsx` 交互回调 | 1006–3808 | 1481 / 291 | 会话启动、CLI 模式、退出路径 |
| `services/api/claude.ts::queryModel` | 1017–2892 | 583 / 127 | 流式 API、错误、重试、成本、工具调用时序 |
| `query.ts::queryLoop` | 241–1729 | 477 / 110 | 模型循环、权限、消息顺序、结束与取消 |
| `services/tools/toolExecution.ts::checkPermissionsAndCallTool` | 599–1745 | 368 / 94 | 权限和执行前后钩子、工具结果 |
| `QueryEngine.ts::submitMessage` | 209–1156 | 341 / 78 | 消息队列与 SDK 工具循环 |
| `tools/BashTool/bashPermissions.ts::bashToolHasPermission` | 1663–2557 | 319 / 75 | 命令解析、审批、拒绝与绕过场景 |
| `screens/REPL.tsx::REPL` | 572–5005 | 731 / 69 | 输入编辑、状态、终端用户交互 |
| `utils/messages.ts::normalizeAttachmentForAPI` | 3453–4286 | 394 / 109 | 内容块与附件格式和大小边界 |

上述表只按优先度列举，**完整覆盖以 20,788 条唯一函数审计记录为准**，不以这张表代替其它函数。独立复核证实了原始函数清单的结构完整性；目前并无 20,788 条 Rust 行为等价证据，必须继续从每个生产入口做正常/异常/权限拒绝/取消/恢复的实际测试，保持严格门禁为失败。

## 已接入统一验收体系

`scripts/source_function_index.py` 读取可信源码哈希和详细规格函数表，逐记录检查路径、所属原文件哈希、行范围、字段和唯一身份；发现同位置多回调时分别保留。`scripts/source_parity_gate.py` 生成可复用函数语义索引，反向检查现有验收契约是否逐条覆盖 20,788 个真实 AST 函数，以及纯常量文件和 vendor 文件级审查。缺失记录写入验收缺口，不会自动变成已通过。

手工升级 `docs/SOURCE_PARITY_CONTRACTS.jsonl` 为 verified 必须再提交原版函数的完整行为描述、对应 Rust 实际入口/文件哈希、测试断言和已通过的 GitHub Actions 提交证据。源码原文仅保留在私有源包，禁止复制到公开仓库。
