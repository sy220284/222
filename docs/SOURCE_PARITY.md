# 222 原版驱动 Rust 测试与全量验收

## 目标与事实源

Rust 生产代码必须按原版 2.1.88 的输入、输出、分支、异常、权限、状态、调用顺序、副作用和恢复行为验收。已确认的原版实际结果固化为可重复运行的 Rust 单元测试、集成测试、端到端测试和回归测试；原版双端执行只针对新增、关键或有争议的行为复核，避免每次修改都依赖私有原版环境。

冻结范围继续以 `docs/PORT_BASELINE.json`、`docs/FILE_PORT_MATRIX.md` 和 `docs/REMOVED_SCOPE.json` 为准：1,906 条原版路径，192 条用户批准排除，1,714 条必须迁移并验收。`docs/PORT_EVIDENCE.jsonl` 中的历史测试名称只算旧证据，可由新体系中真实执行并核验的黄金测试替代；新增路径无需伪造旧提交记录或旧 CI 运行编号。只有原版行为、真实 Rust 测试及完整功能链一致才能晋级验收。

## 测试与门禁结构

`scripts/source_test_gate.py` 是统一验收入口：

- `lint`：在 Cargo 编译前核对原版冻结 SHA、黄金样例、真实 Rust 文件和测试登记结构；不需要私有原版及 Rust 运行清单，发现任何错误直接阻断格式与静态分析 job。
- `check`：核对已验收文件的历史证据、本次编译实际发现的测试、Rust 生产文件、冻结源码 SHA，以及新增黄金测试的准确类别和样例。增量登记错误阻断 CI。
- `gate`：在 `check` 之外，要求全部 1,714 条保留路径均已移植验收且纳入原版驱动测试体系。部分实现、未迁移或没有原版黄金结果的路径一律失败，防止以绿色单测虚报完整等价。
- `report`：提供完整缺口报告，不当作验收通过。
- `verify-original`：在提交者自己工作区中执行真实原版 TypeScript 函数，验证黄金文件每条预期结果与原版一致；不得自动覆写原版预期。

`docs/SOURCE_TEST_REGISTRY.jsonl` 每条源路径登记：
`path`、`source_sha256`、`profile`、`rust_entrypoints`、`golden`、`tests`、`source_ids`（差异复验身份）、`adapter`（实际 Rust 生产执行入口及输出观察字段）、`differential_kinds`（源码实际适用的差异行为类型）。`tests` 明确测试名和类别，所有测试名称都必须出现在本次 Cargo 编译后 `--list` 的真实清单中。

按源码真实职责选择验收层次：

1. `pure` 纯函数：单元测试 + 原版黄金回归。
2. `stateful` 状态模块：单元测试 + 黄金回归 + 集成测试。
3. `user_flow` 用户功能链：黄金回归 + 集成测试 + 端到端测试。

按源码实际存在的路径追加异常、权限、并发、取消、重启、流式、副作用等测试。取消以前把 22,021 个静态源码审计对象机械乘以十类场景的要求。函数索引仍用于查找、定位、覆盖分析，不能直接证明功能已验收。

`tests/source_parity/golden/*.json` 保存源码 SHA、原版导出函数、带 ID、`kind`（明确的正常/边界场景）、真实输入与预期输出；正常和边界输入必须实际不同。

多导出源码可以为每个黄金场景设置 `export` 和 `source_ids`，同一文件的所有登记函数须有真实场景覆盖。需要注册监听器、调用多步状态或处理异步结果时，可将经过审查的原版执行桥接脚本放入 `scripts/source_parity_drivers/*.mjs`，通过登记项 `original_driver` 调用提交者私有工作区中的**真实原版模块**；脚本只提交执行过程，禁止复制或改写原版实现。通过 `verify-original` 重放后，标准输出必须提供与黄金结果一致的按 ID 对应的观察序列，错误即拒绝。`tests/source_golden_registry.rs` 自动遍历登记的每一条原版黄金样例，调用编译后的 Rust 真实生产适配器并比较实际输出；新增登记必须提供可编译的适配器及正确的输出观察字段。`tests/source_contract_golden.rs` 仍保留具体功能的直接函数断言和执行入口回归，现有集成与端到端测试均保留。现有 CLI/SDK/Agent/MCP 测试保留，全量编译运行，不缩减原有范围。

## 全量源码逐函数测试工单生成

`scripts/source_test_factory.py` 与真实通过的测试登记独立运行。在普通 CI 中，以冻结台账一次产出全部 1,714 条保留路径的完整待办清单、Rust 映射和已登记黄金测试缺口，供提交者按优先级持续实现。不能因为工单已经生成就宣称测试通过。

提交者有自己私有工作区原版 `source/` 和完整逐行规格 `spec.zip` 时执行：

~~~bash
python3 scripts/source_test_factory.py --repo . \
  --source-root /absolute/path/to/private/source \
  --spec-archive /absolute/path/to/private/spec.zip --output .port-audit
~~~

严格先核对 1,906 个冻结源码原文哈希及 513,237 行规格，再生成每个实际 AST 函数、方法、导出和文件级单元的测试任务（约 22,021 个）。每条任务保留函数位置、参数、复杂性风险信号、Rust 对应文件及未关联黄金测试的准确数量，不保存原版源码正文。原版路径不变时重复生成结果稳定；新黄金回归登记后自动消减缺口。这是完整测试建设的可复用任务来源，只有真正运行过的单元、集成及端到端测试才会被验收。

## 本地与 CI 验收

本地普通环境，无须安装原版：

~~~bash
cargo test --locked --bins
cargo test --locked --tests
mkdir -p .port-audit
cargo test --locked --all-targets -- --list > .port-audit/RUST_TEST_INVENTORY.txt
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
python3 scripts/source_test_gate.py check --repo . --inventory .port-audit/RUST_TEST_INVENTORY.txt
python3 scripts/source_test_gate.py gate --repo . --inventory .port-audit/RUST_TEST_INVENTORY.txt
~~~

CI 主流程是：`编译与全部测试` → `原版驱动 Rust 回归与测试登记检查` → `原版全部保留功能完成度强制验收`。Linux/Windows 单元和集成测试、静态分析、模型及安全专项均继续执行。只有前置全部 Rust 测试成功后，才保存并核验本轮发现的测试清单。`gate` 在原版保留路径全部覆盖前必须返回非零。`.port-audit/SOURCE_TEST_SUITE.json` 输出全部缺口，`SOURCE_TEST_SUITE.md` 提供摘要。

老的 `scripts/source_parity_gate.py gate` 转发到新的测试门禁；原有 `report/audit/index`、`scripts/source_function_index.py` 与规格分析仍供查源码、排查遗漏，不作为另一个互相冲突的放行口径。

## 提交者自己工作区复验原版

私有原版只留在个人工作区的 `source/src` 与 `source/vendor`。提交者根据真实原版新增黄金测试后可以主动验证，不上传原版源码到公开 GitHub：

~~~bash
python3 scripts/source_test_gate.py verify-original --repo . \
  --source-root /absolute/path/to/source --output .port-audit

cargo build --locked --bins
python3 scripts/source_diff_gate.py --repo . \
  --source-root /absolute/path/to/source \
  --spec-archive /absolute/path/to/spec.zip \
  --cases tests/source_parity/cases --output .port-audit
~~~

`source_diff_gate.py` 逐文件核验 1,906 条原版源码哈希、原版详细规格与原文逐行匹配，使用完整 AST 验证差异用例的函数 ID，然后执行指定源码函数的双端真实输入输出比较，完成后再次哈希原版目录防止修改。只对实际登记的函数做差异复验；正常/边界必须覆盖；有权限/取消/状态等逻辑才要求相应场景。差异执行桥接程序必须经人工审阅实际调用路径，不能凭自报 `executed_source_ids` 冒充覆盖证据。双端运行环境须禁用生产密钥及外网权限。

`.github/workflows/original-differential.yml` 仅提供手动启动的私有 runner 复验，不建立任何定时任务。必须由受信任操作者控制工作流代码来源、运行器权限和原版工作区，禁止运行不可信 fork PR 的代码。没有受控 runner 时在提交者本地执行上述命令。工作流只上传核验摘要，不上传源码。

## 全量原版源码测试建设现状与自动验收路径

冻结的原版包含 1,906 条源码路径，明确排除 192 条，必须迁移并测试的共 1,714 条。原版 AST / 规格索引覆盖 1,902 个第一方文件、513,237 行、20,788 个函数或方法节点、1,212 个独立导出值、17 个纯文件级单元和 4 个 vendor 单元，共 22,021 个待核实对象。**源代码对象数量、登记数量与实际通过的等价功能数量相互独立**。

截至本次 2.1.88 源码驱动测试建设，`docs/SOURCE_TEST_REGISTRY.jsonl` 已接入 **19 条源码路径、389 组真实原版黄金回归场景**。数值仅是当前提交快照，未来变动以 CI 生成的 `.port-audit/SOURCE_TEST_SUITE.json` 和 `.port-audit/SOURCE_TEST_WORKLIST.json` 为准；文档里的旧历史阶段数字不能覆盖这些机器结果。

目前已实际登记测试的范围包括：斜杠解析、JSON BOM、XML 转义、命令生命周期、数组、环形缓冲区、Bash 注释、SDK 控制请求字段、Shell 任务识别、地址路由、命令行参数、消息判定、沙箱违规标签、Set 集合、对象分组、macOS 特殊键、工作树开关、Bash 超时参数与用户提示词关键词。

新增测试的实际源码差异修复包括：U+0085 NEL 在 JavaScript `trim/parseInt` 中不属于可移除的空白；`isMacosOptionChar` 通过普通对象的 `in` 操作符会对 `constructor`、`toString` 等继承键返回真；环形缓冲区零容量场景；部分跨语言集合与分组行为。所有修复仍以 CI 实际执行结果为准。

### 实施原版全量测试的可复用途径

1. 对提交者私有的 **完整原版工作区** 执行 `scripts/source_test_factory.py --source-root ... --spec-archive ...`；它先验证冻结 1,906 条源文件哈希与规格逐行一致，再根据真实 AST 生成 **所有必须迁移路径/函数**的待测工单。无私有原版时，普通 CI 仍对 1,714 条路径生成完整文件级缺口索引，不伪造函数级结果。
2. 为每个真实原版功能补齐可运行的单元测试、回归、按职责需要的集成/端到端测试、来源 SHA、AST 函数身份和黄金预期；通过 `scripts/source_test_gate.py verify-original --source-root ...` 在提交者工作区复验原版结果。
3. Rust 侧必须通过生产模块执行适配器实际调用，再用 `cargo test --locked --bins` 和 `cargo test --locked --tests` 运行回归。任何新登记会被统一黄金回归测试自动发现并重放。注册字段缺失、真实测试名不在本轮编译清单、原版输入与 Rust 结果不同，均不能标记验收。
4. 普通 CI 的 `source-regression` 先验收已实现的真实测试，`source-completeness` 再对全部原版路径与函数契约严格核查。完整范围补齐前始终保持非零退出码，禁止借待办文档、静态索引或部分测试冒充已通过。

**当前尚未完成的语义覆盖**：Set 的 JS 对象引用身份与 SameValueZero 全部数值边界、对象分组中的 Symbol 键及枚举次序、超时参数超出 Rust `u64` 的 JS Number 范围、以及复杂 CLI/Agent/SDK/MCP 的跨模块行为，还需要单独复验和补齐。纯函数黄金样例通过，不自动意味着该源码所在的完整调用链等价。

仓库管理员仍须在 main Ruleset/分支保护中把完整验收和 Rust 测试设为必需状态并验证执行。工作流失败本身不足以证明服务器强制阻止合并。
