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

## 从真实原版输入输出新增叶子函数回归

`isLocalShellTask` 18 例、`normalizeControlMessageKeys` 23 例、`extractBashCommentLabel` 25 例均已从实际 2.1.88 TypeScript 模块执行并冻结结果；通过同一个 Rust 真实生产调用适配器重放。新增 U+0085 NEL 字符边界测试，并修正 Bash 注释标签的 JavaScript/Rust 空白字符语义差异。

本次新增 66 例，已登记黄金测试累计 120 例。此数字仅代表已编制并接入的明确用例，不能代替尚未迁移路径的完整验收。

## 当前验收结果的含义

原版静态索引已核对 1,902 个第一方文件、513,237 行、20,788 个函数/方法及 1,212 个独立导出值，另有 17 个纯文件级单元和 4 个 vendor 单元，共 22,021 个定位对象。它们是核查范围，不能当作已通过的功能测试数。

当前已登记四组原版黄金回归：`src/utils/slashCommandParsing.ts` 的 15 例（包含 U+0085 空白语义差异），以及 `src/utils/jsonRead.ts` 的 11 例（前导 BOM、双 BOM、空值、NUL、Unicode、尾部 BOM 等）。再加入 `src/utils/xml.ts` 的 18 例，覆盖同一文件两个不同导出函数及其内部调用链。另外增加状态型 `src/utils/commandLifecycle.ts` 的 10 例，使用公开可审阅的原版执行脚本对照监听器替换、清空和事件顺序，Rust 侧运行真实生产状态模块。四组原版行为均纳入数据驱动回归。其他路径在新测试体系中仍需逐项沉淀黄金用例、补齐真实 Rust 功能链并验收。新增测试体系不自动升级任何路径的迁移状态。

仓库管理员仍须在 main Ruleset/分支保护中把完整验收和 Rust 测试设为必需状态并验证执行。工作流失败本身不足以证明服务器强制阻止合并。
