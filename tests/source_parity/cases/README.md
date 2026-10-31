# 提交者本机原版/Rust 动态差异场景

此目录中的 JSON 用来针对**已选、已审阅的真实源码函数**重新运行原版与 Rust，两端均使用同一份 JSON 输入。日常验收以原版黄金结果和 Rust 单元、集成、端到端测试为主，参见 ../../docs 以外的 `docs/SOURCE_PARITY.md`。

每条用例至少包含 `id`、`kind`、`source_ids`、`input`、`timeout_seconds`、`original` 和 `rust`；命令必须为真实可执行 argv，支持 `{ORIGINAL}` 和 `{REPO}` 占位符，不经 shell 解析。两端从标准输入接收场景，输出统一的 `source-rust-observation/v1` 对象。必须核对最终结果、错误、状态、事件顺序、权限、磁盘副作用、持久化和取消状态；详情参见 `scripts/runtime_parity_gate.py`。

每个选定原版函数的正常和边界输入必须实际执行。源码确实涉及错误、权限、重启、取消、并发、流式或副作用，再要求相应专项测试；不再给纯函数虚构十种业务行为和冗长的“不适用”说明。

原版函数必须事先在 `docs/SOURCE_TEST_REGISTRY.jsonl` 的 `source_ids` 中登记，并与冻结的 AST 函数身份一致。原版执行入口位于提交者自己的原版目录，Rust 入口调用实际生产代码。`executed_source_ids` 为辅助追踪信息，必须复核真实代码调用和测试断言，禁止自报执行覆盖充当证据。

示例命令：

~~~bash
cargo build --locked --bins
python3 scripts/source_diff_gate.py --repo . \
  --source-root /path/to/private/source \
  --spec-archive /path/to/private/spec.zip \
  --cases tests/source_parity/cases --output .port-audit
~~~

发现差异后，修复 Rust，并将能复现该差异的真实输入和原版预期追加至 `tests/source_parity/golden/` 对应文件，使此后无需重复依赖原版执行即可阻止同类回归。
