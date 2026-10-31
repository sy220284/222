# 222 唯一原版源码与 Rust 全量等价验收门禁

## 结论与边界

唯一强制入口：`scripts/source_parity_gate.py gate`，由 `.github/workflows/ci.yml` 中固定作业 **原版源码全量等价验收** 执行。正常编译、静态扫描、原版文档、函数索引、旧 CI 通过记录，都不能替代原版与 Rust 在同样输入下实际运行的结果。

冻结原版 2.1.88：**1,906 条源码路径**（1,902 条第一方 `src/` 与 4 条 `vendor/`）。仅按已授权范围排除 **192 条**，其余 **1,714 条**都须完整复核。单一事实源：

- `docs/PORT_BASELINE.json`：原版文件路径、SHA256、字节数与行数的冻结来源。
- `docs/FILE_PORT_MATRIX.md`：真实迁移状态及 Rust 映射，禁止因为索引存在自动变更为已验收。
- `docs/REMOVED_SCOPE.json`：经用户批准的剔除范围。
- `docs/SOURCE_PARITY_CONTRACTS.jsonl`：原版逐函数与 Rust 行为契约，每个功能索引的行为及场景适用性。
- `docs/PORT_EVIDENCE.jsonl`：验收提交和 CI 证据，必须与当前 Rust 实际文件哈希一致。

## 提交者自己工作区的源码（不再上传/下载原版）

**原版由每位提交者在自己的受控工作区查阅、执行、复核。** 不要求 GitHub Secrets 中配置私有 ZIP 下载地址，也禁止将原版源码复制到公开仓库、PR、日志或构建工件。工作区目录在仓库外，包含 `source/src/` 和 `source/vendor/`。若只有 `src/` 目录，仍缺 4 个 `vendor/` 原版文件，冻结校验会失败。

源码 SHA256 的原版 ZIP 参考指纹为 `23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`；完整规格 ZIP 指纹为 `3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93`。本门禁逐文件校验目录中的 1,906 个原版文件的冻结哈希、大小和行数，并交叉核验规格中的 1,902 个第一方文件逐行原文。规格 ZIP 的总体 SHA256 另由 CI 校验，内部所有 5,258 个条目的清单哈希也逐个核对。

提交者本地命令：

~~~bash
python3 scripts/source_parity_gate.py report --repo . --output .port-audit

python3 scripts/source_parity_gate.py index --repo . \
  --source-root /absolute/path/to/claude-code-2.1.88/source \
  --spec-archive /absolute/path/to/complete-spec.zip \
  --output .port-audit --write-contracts

python3 scripts/source_parity_gate.py gate --repo . \
  --source-root /absolute/path/to/claude-code-2.1.88/source \
  --spec-archive /absolute/path/to/complete-spec.zip \
  --runtime-cases tests/source_parity/cases \
  --output .port-audit
~~~

`--source-root` 读取**真实目录**，只在本地临时创建中间 ZIP 复用冻结哈希校验流程，操作结束即删除；不得指向当前公开仓库内的目录，也不接受符号链接。`--source-archive` 仍可用于历史静态审计，但完整动态 `gate` 强制同时要求原版工作区目录，不能只交一个历史 ZIP 冒充实际运行。

## 动态对照到底做什么

`scripts/source_function_index.py` 从原版 AST 详细规格构造独立审计 ID：已复核的 20,788 个函数/方法节点 + 1,212 个不同的直接导出值 + 17 个纯文件级单元 + 4 个 vendor 文件级单元，合计 **22,021 个静态定位对象**。它们仅是原版需要核对的对象，不等于 22,021 个已经独立验收的用户功能。同一行的不同回调保留各自身份。旧规格的“状态写入数”存在漏报，已作为不可信历史数据，禁止用它判定状态逻辑已经等价；参见 `docs/SOURCE_AST_VERIFICATION.md`。

动态门禁 `scripts/runtime_parity_gate.py` 读取 `tests/source_parity/cases/*.json` 中的显式场景并执行：

1. 在隔离的两个临时数据目录中，用**同一个 JSON 输入**分别启动工作区原版执行入口和已编译的 Rust 执行入口（直接 argv，无 shell，均限制时间和输出大小）。
2. 要求两个程序都返回 `source-rust-observation/v1`，实际报告运行过的原版函数 ID，且与该场景 `source_ids` 完全一致。无原版实际执行、找不到入口、超时、崩溃、未产生可解析的观察数据都拒绝通过。
3. 严格比较返回值、错误、状态快照、函数/工具调用顺序、权限决定、事件顺序、副作用、持久化快照、取消表现，以及真实进程退出码与标准错误摘要。所有差异均保留到 `DYNAMIC_PARITY.json` 中；不自动归一化或忽略不符合预期的字段。
4. 每个保留的原版审计对象必须覆盖 **normal、edge、error、permission_allow、permission_deny、cancel、concurrent、restart、stream、side_effect** 十种行为族。确实不适用的族，必须在对应 `functions[].not_applicable_scenarios` 中逐项写出源代码起止行号、原版 SHA256、具体理由；**normal 和 edge 禁止豁免**。这些文字仍需由代码审阅者核实，禁止批量虚构。
5. 逐条绑定已审行为契约、Rust 生产入口和最新文件哈希、实际通过的 CI 测试记录及当前迁移状态。只要有遗漏、差异或未迁移项，`gate` 返回失败，**不会自动放行或自动晋级状态**。

双端测试适配器须分别调用原版和 Rust 的**真实生产代码**，需要在原版工作区中构建/配置可实际运行的测试入口。仅打印预设 JSON 的模拟器即使两端结果一致，也不构成通过验收的证据；必须人工检查适配器的真实调用链与源码断言。测试环境须隔离网络与凭据并控制时间、随机数、路径等可变因素。不能把同一个实现作为双端适配器。

可复核场景格式、约束及增加方式见 `tests/source_parity/cases/README.md`。`SOURCE_PARITY.json` 包含静态/动态验收结论；`DYNAMIC_PARITY.json` 单独输出场景覆盖、实际执行数及差异；`SOURCE_FUNCTION_COVERAGE_GAPS.json` 记录未闭环的原版索引。

## CI 如何强制阻断

GitHub-hosted runner **无法读取提交者电脑的文件**。因此强制作业使用专用标签 `self-hosted, linux, source-parity-222` 的可信隔离运行器；外部 fork PR 改用 GitHub-hosted runner，缺少私有原版，按设计失败，避免让不可信 PR 在携带私有源码的运行器上执行。请将个人原版工作区配置到该独立运行器：

- 缺省源码：`$RUNNER_WORKSPACE/original-2.1.88/source`
- 缺省规格包：`$RUNNER_WORKSPACE/original-2.1.88/spec.zip`
- 可选使用仓库 Variables `ORIGINAL_SOURCE_ROOT` 和 `ORIGINAL_SPEC_ARCHIVE` 指定绝对路径；不依赖 Secret 下载 URL。

运行器要对每次作业进行隔离清理，确保原版目录只读、适配器无生产密钥或外网权限；一位提交者自己的源码工作区**不会自动同步到其它提交者的运行器**。未注册符合标签的可信运行器时，必需验收作业无法成功，按“未获验收”处理。

**合并保护需仓库管理员在 `main` Ruleset 中把 `原版源码全量等价验收` 设为必需成功状态，禁止绕过。** 单纯让工作流检查失败，尚不能保证 GitHub 合并按钮被服务器端拦截。2026-10-31 实查 GitHub `/repos/sy220284/222/rulesets` 返回空列表（`[]`），读取 `main` 分支保护返回 403 权限不足。尚无可证实的强制合并保护；只有仓库管理员设置并重新读取验证后才能宣称不可绕过。

仅归档经过限制的缺口报告，不上传原版源码、原版工作区 ZIP、函数签名/初始化表达式全量索引。CI 取消过期提交的旧作业；仅最新 HEAD 的完整验收结果有效。

## 本轮源码级复查与当前真实状态

对用户提供的原版源码和完整规格 ZIP 核验：两个 SHA256 与冻结指纹匹配；1,902 个第一方文件共 **513,237 行**逐行原文一致；AST 与 TSV **20,788 个函数/方法**匹配；`export` 直接导出变量 **1,276** 项中另有 **64** 项箭头函数已在函数表内，剩余 1,212 项独立入索引。`src/bootstrap/state.ts` 的 `regenerateSessionId` 与 `switchSession` 涉及 `STATE` 字段直接赋值、缓存删除及会话切换通知，旧规格“状态写入数=0”存在漏报；因此新增动态场景必须核对缓存、状态同步及事件时序。更多来源核验见 `docs/SOURCE_AST_VERIFICATION.md`。

当前原有迁移矩阵 38 已验收、1,212 部分实现、464 未迁移、192 已排除。现在的 1,714 条行为契约仍处于待审/未充分证明状态，仓库也尚无覆盖 22,021 个原版审计对象的**真实双端动态执行场景**。**新门禁应失败，不得为取得绿色 CI 而补造测试结果或擅自放宽范围。** 交付时必须核对实际 GitHub Actions 作业及 Rust 测试的最终结论，并继续修复真实功能缺口。

旧 `port_workbench.py`、规格索引和历史审计文档保留为数据导入、台账与影响分析辅助，最终动态和静态结论统一以 `source_parity_gate.py gate` 为准。