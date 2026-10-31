# 原版与 Rust 同场景动态审计用例

本目录只放由提交者逐项实际实现的 JSON 场景。没有用例时全量门禁必须失败；禁止通过占位样本或“已审核”字样冒充实测。

## 一个场景 JSON 的格式（示例，不能直接冒充已验收）

```json
{
  "id": "session-regenerate-parent-normal",
  "kind": "normal",
  "source_ids": ["src/bootstrap/state.ts:FunctionDeclaration:435-450:occurrence-1"],
  "input": {"setCurrentAsParent": true},
  "timeout_seconds": 20,
  "original": ["node", "{ORIGINAL}/test-bridge/session-original.mjs"],
  "rust": ["{REPO}/target/debug/atlas-agent", "parity-session"]
}
```

这只是字段示意：实际函数 ID 必须从本次 `SOURCE_SEMANTIC_FUNCTIONS.jsonl` 取得，原版和 Rust 命令必须对应真正实现的可执行入口。示例所写的原版 bridge 和 Rust 子命令并未确认存在，**禁止复制为实际验收场景**。原版 bridge 应在提交者私人工作区内，不放在公开仓库；需要对其实际调用链进行独立审阅。Rust 执行入口必须通过当前提交编译得到。

`kind` 可以是 `normal`、`edge`、`error`、`permission_allow`、`permission_deny`、`cancel`、`concurrent`、`restart`、`stream`、`side_effect`。每条原版保留审计对象都需要十类中的适用场景，并且必须执行原版和 Rust 双端。确实不适用的行为族，逐个在 `docs/SOURCE_PARITY_CONTRACTS.jsonl` 对应 `functions[]` 中标注：

```json
{
  "source_id": "真实的源函数 ID",
  "not_applicable_scenarios": {
    "permission_deny": {
      "source_lines": [1, 5],
      "source_sha256": "对应冻结原版完整 SHA256",
      "reason": "具体解释为什么这一源码范围不具有权限拒绝路径，并由审查者复核"
    }
  }
}
```

说明必须定位到该源码函数本身的实际起止行，禁止豁免 `normal` 与 `edge`。对同一对象可用多个不同输入与 ID 覆盖同一行为族。

## 双端输出契约

两个命令都从标准输入读取一行 JSON：`{"id":"场景ID","input":{...}}`。都必须打印**唯一 JSON 对象**到标准输出，格式为：

```json
{
  "schema":"source-rust-observation/v1",
  "case_id":"场景ID",
  "executed_source_ids":["实际执行过的原版函数审计 ID"],
  "outcome":{"text":"最终用户可见结果"},
  "errors":[],
  "state":{},
  "calls":[],
  "permissions":[],
  "events":[],
  "side_effects":[],
  "persistence":{},
  "cancellation":{"cancelled":false}
}
```

请真实记录每种状态及调用事件的先后顺序；`executed_source_ids` 必须能通过执行记录、运行时打点及源码调用链进行审阅，不允许直接把用例声明的 ID 照抄为“已执行”。测试适配器不应把系统时间、随机 UUID、路径等未经控制的值直接塞入结果；除经过源码证明的明确等价归一化外，门禁按原版/Rust 结构与内容逐字段严格比较，包括**进程退出码、标准错误哈希**。运行结果差异、任意一端未实际运行、异常、超时、漏列场景均阻断 CI。

动态验收命令依赖的原版代码请放在**自己**的受控工作区，并保持完整 2.1.88 原版哈希；Rust adapter 在仓库中随生产代码审阅、构建。生产网络、密钥与用户真实数据不得进入动态执行环境。