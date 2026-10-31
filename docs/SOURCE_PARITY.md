# 222 原版源码、函数行为与 Rust 等价验收唯一工作台

## 统一真源
原版 2.1.88 共有 1,906 条源码路径，已批准排除 192 条，其余 1,714 条必须有完整的 Rust 实现、真实入口、行为与恢复测试。原版 SHA256 和首次状态存于 docs/PORT_BASELINE.json；实时迁移状态只取 docs/FILE_PORT_MATRIX.md；已授权排除由 docs/REMOVED_SCOPE.json 定义；已执行测试证据写入 docs/PORT_EVIDENCE.jsonl；全函数语义与行为契约统一保存在 docs/SOURCE_PARITY_CONTRACTS.jsonl。禁止从历史报告复制完成率。

## 检查入口
python3 scripts/source_parity_gate.py audit --repo . --output .port-audit
python3 scripts/source_parity_gate.py audit --repo . --source-archive /private/claude-code-2.1.88.zip --output .port-audit
python3 scripts/source_parity_gate.py gate --repo . --source-archive /private/claude-code-2.1.88.zip --spec-archive /private/2.1.88-spec.zip --output .port-audit

统一输出 SOURCE_PARITY.json 和 SOURCE_PARITY.md。audit 可核查缺口，gate 是拒绝未闭环的最终验收。源码缺失、文件哈希/大小/行数不符、函数/行为证据遗漏、仍有未迁移/部分实现、测试凭证不存在，全部使 gate 返回失败。它永远不自动把一条路径改为已验收。原版只用于对照，不能发布到公开 GitHub 仓库。

## 行为契约
每条原版非排除路径使用原路径作为唯一键；每个函数使用函数名与原版起止行号标识，记录输入、输出、条件和功能逻辑、失败、权限、状态、时序、取消、持久化、副作用、真实调用链及测试。全文件需逐分支审查，内部动态入口不能因未导出而漏计；同一业务的 CLI、SDK、工具、MCP 多入口归并为同一功能链，同时保留路径级证据。文档和原版哈希匹配只能证明来源，完整功能仍需当前 Rust 行为与 CI 实测。

## CI 如何获取保密原版源码
仓库 Secrets 必须配置 ORIGINAL_SOURCE_ARCHIVE_URL、ORIGINAL_SPEC_ARCHIVE_URL 两个私有下载地址；两个原版 ZIP 的 SHA256 已作为固定来源指纹写入 CI（源码 `23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`，规格 `3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93`），不再要求单独设置 SHA256 Secret；可选 ORIGINAL_SOURCE_ARCHIVE_TOKEN 与 ORIGINAL_SPEC_ARCHIVE_TOKEN（分别用于源码和规格包的 Bearer 认证，互不转发）。URL 必须 HTTPS 且无需跨站重定向，CI 首先校验两个 ZIP 的整体哈希；门禁再逐项验证全部 1,906 个源文件的冻结 SHA256、大小及行数，以及 5,258 份规格包文件的内部 SHA256 和 1,902 条第一方源文件与详细规格的唯一双向关系。下载地址缺失会使严格验收失败，仍生成完整缺口报告。请使用私有且受控的存储，不要将源码放入公开仓库、发布物或 Actions Artifact。

## GitHub 服务器端保护
原版源码全量等价验收是固定作业名。要阻止 GitHub 网页/接口绕过失败的 CI，需对 main 设置 Ruleset 强制要求该检查且不允许 bypass；当前 GitHub 连接缺乏读取/配置分支保护的管理员权限，不能将工作流失败误称为已开启不可绕过的合并保护。

## 旧工具与历史资料去重
scripts/port_workbench.py：唯一迁移台账读取、证据检查、静态影响范围与空壳检测；其结果汇入本门禁。
scripts/spec_closure_index.py 与 scripts/spec_document_progress.py：外部产品规格/函数入口索引导入工具，仅供生成待办，不再另设验收结论。
scripts/import_original_prompt_literals.py：提示词原文候选采集，不把原文存在当成运行时对齐。
scripts/sync_spec_progress_summary.py：同步显示数字，不再定义独立状态。旧审计报告保存用于追溯，不重新引入第二套工作台。

## 2026-10-31 首轮审计状态
当前 FILE_PORT_MATRIX 登记：排除 192、已验收 38、部分实现 1,212、未迁移 464。剩余 1,676 项必须继续完成逐函数与行为验收。现有 PORT_EVIDENCE.jsonl 仅 11 条历史新增验证记录，且历史的验收项并不具有本门禁所需的全函数契约。因此初次强制门禁预期失败；这是真实问题清单，不能用占位数据改成通过。

现有工作流此前没有原版 ZIP 输入，外部完整原版档案也未入库，故本次不会声称已完成全量动态等价。按照本门禁持续补齐之后，应从原始源码重新复查所有非排除路径，直到完整真实证据全部通过。

## 实际来源锁定

本轮独立读取用户资料库原版 ZIP：SHA256 23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d；完整逐行规格包：SHA256 3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93。规格包 ZIP CRC 通过，内部 5,258 条哈希记录验证一致，1,902 条第一方映射没有重复、源码 SHA 不匹配或详细文档缺失。该静态来源审计不等于动态运行行为对齐。


## 2026-10-31 契约底稿重构及可执行路径

**现状必须分层显示：** 本次把零字节的 `SOURCE_PARITY_CONTRACTS.jsonl` 建成覆盖 1,714 条应迁移路径的**待审基线**，内容是已有冻结 SHA256、路径、源码行数、Rust 映射和明确的待验证状态。这仅消除了空文件导致的断档；1,714 条当前没有一条可以凭这份底稿通过新版完整行为验收。禁止把 JSONL 记录条数称为验收完成率。

需要真实源码逐函数索引时，在有两份用户授权原版 ZIP 的可信环境运行：

```bash
python3 scripts/source_parity_gate.py index --repo . \
  --source-archive /private/claude-code-2.1.88.zip \
  --spec-archive /private/complete-source-spec.zip \
  --output .port-audit --write-contracts
python3 scripts/source_parity_gate.py report --repo . --output .port-audit
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
```

`index` 在写入前重新校验全部 1,906 个文件及冻结哈希；可选核对 1,902 份第一方详细规格，并生成声明候选、源码位置与规格文件引用。重复执行按源路径覆盖同一个待审底稿，已经验收的人工契约保持原样。提取器捕获的是待复核函数线索，自动登记不生成语义、权限、失败、持久化等运行结论，也不自动调整台账。需要逐个补足 `functions` 的十二种行为维度、真实 Rust 入口及回归测试，再重新审计。

已在本轮独立验证的私有资料：源码 ZIP SHA256 为
`23ee61c936cc73e34dbd556b10ff3ad355d2699a91e4e69cd8fc12791017d33d`，
详细规格 ZIP SHA256 为
`3b291532059e887b20a9adaf779fb789dabcd843042c5343745f3adfdf372b93`。
对原版 1,906 个源码文件及 1,902 份第一方详细规格逐行核对，共计 **513,237 行原文，零差异**。这只说明规格文本准确复现了原版源码，不等于 Rust 功能已等价。

### GitHub 私有资料接入（仓库管理员一次性配置）

GitHub 仓库 `Settings → Secrets and variables → Actions → Repository secrets` 中配置：

- `ORIGINAL_SOURCE_ARCHIVE_URL`：可通过 HTTPS 无重定向获取完整原版 ZIP 的私有地址。
- `ORIGINAL_SPEC_ARCHIVE_URL`：完整逐行规格 ZIP 的私有地址。
- `ORIGINAL_SOURCE_ARCHIVE_TOKEN`：源码服务需要 Bearer 鉴权时提供。
- `ORIGINAL_SPEC_ARCHIVE_TOKEN`：规格服务需要 Bearer 鉴权时提供。

整体 SHA256 已固定在 `.github/workflows/ci.yml`，无须将公开的哈希重复存为 Secret。建议两份档案放在访问受控、支持短时密钥的私有对象存储中；CI 不向日志或工件上传原版 ZIP。

**不能把 ChatGPT 资料库路径当作 GitHub Actions 可访问的路径**。Actions 运行器无权直接读取本会话资料库。仓库管理员完成私有下载地址及 Token 配置前，下载会失败，随后仍执行唯一契约门禁并上传失败原因及缺口报告；这个失败是按设计拒绝放行。

当前 CI 已将台账校验、调用范围审计、迁移报告、规格进度同步与完整源码验收统一从 `scripts/source_parity_gate.py` 进入。旧脚本仍作为内部数据处理模块或兼容入口保留，原始迁移状态继续以单一矩阵为准。

GitHub 网页/API 真正禁止合并仍需要有管理权限的仓库人员设置 `main` Ruleset，要求状态检查 `原版源码全量等价验收` 成功且禁止绕过；PR 保持 Draft，直到所有必需证据闭环。
