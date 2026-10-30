# 222 全仓审计八项问题：代码修复与证据记录（2026-10-30）

## 基线与边界

- 审计基线为 `main@ace3cb0c90f57dbb318247746d75031c1a950671`，修复沿 PR #6 的 `fix/manual-source-parity-20261030` 推进；PR 未全量等价前保持 Draft。
- 唯一原路径台账为 `docs/FILE_PORT_MATRIX.md`：1,906 条路径、22 条排除，其余 1,884 条需独立 Rust 等价。冻结的 `docs/PORT_BASELINE.json` 不等于实时完成度。
- 已登记的 `verified` 数目暂未提升。逐文件功能、用户入口及跨平台端到端验证是独立证据要求。
- 本报告按照实际源码和 GitHub Actions 执行结果更新，不把函数存在、可查询 registry、单模块编译或静态描述当作真正迁移完成。

## 八项问题处理状态

| 编号 | 原审计问题 | 当前修复与证据 | 仍需处理 |
| --- | --- | --- | --- |
| A-01 | MCP HTTP tools/call 响应缺少内存边界 | `src/mcp_http.rs` 对普通响应设 2 MiB、工具响应设 128 MiB；JSON 采用边读边核验，SSE 按累计字节限制；资源变更通知额外设 256 条和 8 MiB 待处理上限，读取后归零；加入边界与背压测试 | 继续复核长时运行、取消及恶意流组合场景 |
| A-02 | 带凭证远程模型或搜索地址允许 HTTP | `src/config.rs`、`src/provider.rs`、`src/websearch.rs`、`src/mcp_http.rs` 保留无密钥回环本地 HTTP，带凭证远程端点要求 HTTPS，关闭自动跳转；增加通用请求头环境变量和动态凭证脚本防护 | 后续仍须按各种服务配置开展跨端到端测试 |
| A-03 | 原版终端 UI 未接真实主交互入口 | 已补若干输入、占位符、渲染/键位语义；`src/main.rs::chat` 仍存在 `stdin.read_line`，尚缺完整逐键 TUI 主循环 | 原版 Ink 组件、快捷键、弹窗、光标、粘贴、多行状态与真实终端回归 |
| A-04 | 自动生成模块无生产调用及等价证据 | `scripts/port_workbench.py reachability` 生成 `generated-reachability.json`，CI 保留工件并区分 `inspect_only` 和 `outside_reference`。在本次本地扫描的 Rust 快照中 409 个哈希生成模块均为 `inspect_only`，0 个常规模块直接引用 | 逐文件核查原始函数；手工实现并接真实业务入口；必须实际执行和验收，不能将静态查询标为完成 |
| A-05 | Bridge 消息缓冲可无限增长 | `src/bridge_core.rs` 增加 16,384 条/64 MiB 双阈值，批次原子拒绝，flush/drop 后回收预算，专项测试覆盖 | 完整远程 Bridge 连接、认证、断线续传与消息重放链尚需等价 |
| A-06 | 迁移验收过度依赖静态证据 | `scripts/port_workbench.py` 对新 `verified` 校验实际 GitHub Actions 运行、测试 HEAD、成功的 Rust 测试作业；明确排除跳过作业与格式检查冒充验证 | 冻结基线 29 条历史登记及 1,855 条未真正完成路径仍需逐函数复审 |
| A-07 | Linux/Windows 完整 CI 被跳过 | 已在 `.github/workflows/ci.yml` 删除两处 `if: false`，Linux 和 Windows 已能真实执行，暴露并推动修复终端占位符、诊断日志和 Windows 工作目录测试问题 | 以当前 PR HEAD 对应 CI 五个作业全绿为准，不采纳被取消或旧 HEAD 的成功记录 |
| A-08 | AGENTS.md 仍指向已合并的 PR #2 | `AGENTS.md` 已更新为 main + 活动 PR #6 + Issue #3，明确测试、验收、代码对照及防止覆盖并行工作 | 随活动 PR 变化时及时更新 |

## 验收命令（本地与 CI 相互复核）

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D clippy::correctness -D clippy::suspicious
cargo test --locked --all-targets
python3 scripts/port_workbench.py check --repo .
python3 scripts/port_workbench.py reachability --repo . --output .port-audit
python3 -m unittest discover -s tests/port_workbench -p 'test_*.py'
```

Linux/Windows 必须分别验证；本地 Linux 结果不能替代 Windows CI。一次提交触发的旧检查如果被后续 HEAD 取消，结论作废。

## 原版源码对照规则

2.1.88 原始源码包是函数级对照的必需输入。当前文件台账、源码索引与历史引用可标识应迁移路径；若本次执行环境未取得原版逐行内容，不可据这些材料宣布函数行为完全一致。保留非用户要求删除的模型无关能力，同时移除原厂强制登录、专属品牌和环境/行为遥测回传。

## 下一项必须完成的主链

按用户实际可用性排序推进终端逐键交互、SDK 控制、Hooks 异步续写、远程 Bridge、插件事务及 409 个 `inspect_only` 模块，所有功能从 Rust 入口至结果恢复完整核验；新提交不能擅自增加 `verified` 数字。
