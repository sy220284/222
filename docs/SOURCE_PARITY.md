# 222 原版源码、函数行为与 Rust 等价验收唯一工作台

## 统一真源
原版 2.1.88 共有 1,906 条源码路径，已批准排除 192 条，其余 1,714 条必须有完整的 Rust 实现、真实入口、行为与恢复测试。原版 SHA256 和首次状态存于 docs/PORT_BASELINE.json；实时迁移状态只取 docs/FILE_PORT_MATRIX.md；已授权排除由 docs/REMOVED_SCOPE.json 定义；已执行测试证据写入 docs/PORT_EVIDENCE.jsonl；全函数语义与行为契约统一保存在 docs/SOURCE_PARITY_CONTRACTS.jsonl。禁止从历史报告复制完成率。

## 检查入口
python3 scripts/source_parity_gate.py audit --repo . --output .port-audit
python3 scripts/source_parity_gate.py audit --repo . --source-archive /private/claude-code-2.1.88.zip --output .port-audit
python3 scripts/source_parity_gate.py gate --repo . --source-archive /private/claude-code-2.1.88.zip --output .port-audit

统一输出 SOURCE_PARITY.json 和 SOURCE_PARITY.md。audit 可核查缺口，gate 是拒绝未闭环的最终验收。源码缺失、文件哈希/大小/行数不符、函数/行为证据遗漏、仍有未迁移/部分实现、测试凭证不存在，全部使 gate 返回失败。它永远不自动把一条路径改为已验收。原版只用于对照，不能发布到公开 GitHub 仓库。

## 行为契约
每条原版非排除路径使用原路径作为唯一键；每个函数使用函数名与原版起止行号标识，记录输入、输出、条件和功能逻辑、失败、权限、状态、时序、取消、持久化、副作用、真实调用链及测试。全文件需逐分支审查，内部动态入口不能因未导出而漏计；同一业务的 CLI、SDK、工具、MCP 多入口归并为同一功能链，同时保留路径级证据。文档和原版哈希匹配只能证明来源，完整功能仍需当前 Rust 行为与 CI 实测。

## CI 如何获取保密原版源码
仓库 Secrets 必须配置 ORIGINAL_SOURCE_ARCHIVE_URL 与 ORIGINAL_SOURCE_ARCHIVE_SHA256，可选 ORIGINAL_SOURCE_ARCHIVE_TOKEN（Bearer）。URL 必须 HTTPS 且无需跨站重定向，脚本首先校验 ZIP 整体哈希，再逐项验证全部 1,906 个源文件的冻结 SHA256、大小及行数。Secret 缺失也会使门禁失败。请使用私有且受控的存储，不要将源码放入公开仓库、发布物或 Actions Artifact。

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
