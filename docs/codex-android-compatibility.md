# Codex Android 17 功能对齐与验收台账

更新：2026-11-03；源码基线：`openai/codex@67f0994b`；目标仓库：`sy220284/222`。

## 验收标准

以**真实的 Android 17 ARM64 运行结果**判断功能是否完成。协议目录存在、编译成功、APK 能启动、完整功能链路通过是四种不同的状态，应分别记录。App Server 是手机端的原生执行核心；Compose 负责触屏 UI、授权和状态展示。原桌面 TUI 不是手机运行时依赖。

| 项目 | 已有工作 | 下一验证 |
| --- | --- | --- |
| Android UI | Kotlin/Java + Compose Material 3，Android CI 独立编译已通过 | 安装后检查导航、键盘、动画、旋转、长列表 |
| Codex App Server | 基于官方 `codex-app-server` 的 Android ARM64 交叉编译分支 | 完成 ELF 编译和 JSON-RPC 初始化 |
| 协议完整性 | 从 Rust 协议提取 183 个方法和官方 JSON Schema；CI 校验同步 | 实际执行并记录每一类请求的成功/失败 |
| 会话与模型 | `thread/start`、`thread/list`、`thread/resume`、`turn/start`、`model/list` 等原生界面接入 | 新建、连续提问、恢复、模型切换 |
| 账户认证 | API 密钥 Keystore + ChatGPT 设备授权 UI | 登录、刷新、退出、网络断开及恢复 |
| 工具执行 | 官方 App Server 原生工具机制 | 文件读写、终端、Git、代码审查、MCP、插件、技能 |
| 多文件输入 | Android 系统文件选择器和独立工作区 | 图像/文本/音频格式及多选边界 |
| 长任务 | Android 前台服务、持续通知和任务中断 | 切后台、系统回收、重启恢复；处理 Android dataSync 运行时间限制 |
| 权限控制 | 审批对话和权限模式 UI 已实现 | Android 原生沙箱执行隔离；严禁把桌面沙箱语义视作已验证 |
| APK 分发 | CI 设计了 ELF 校验、APK 打包、内容比较和构建产物上传 | 实际产出含 Rust ELF 的 APK 并通过安装/启动验收 |

## 已确认的 Android 平台问题

1. 微软 `mxc-sdk` 是 Windows 原生沙箱依赖，错误放在 `mxc-sandbox` 通用依赖列表会让 Android 编译失败。已移至 `cfg(windows)`，保留 Windows 本体。
2. Codex `codex-rs/sandboxing/src/manager.rs::get_platform_sandbox` 仅对 macOS、Linux、Windows 返回实际沙箱类型；Android 当前返回 `None`。应用私有 UID 隔离无法替代“只读/工作区写入”的细粒度保护。须增加 Android 专属执行隔离，进行写入拒绝实测；在这之前不签收该功能。
3. Android 29+ 的可执行文件装载规则需要从 APK `nativeLibraryDir` 启动真实的 AArch64 PIE，不能把命令二进制简单释放到应用可写数据目录。打包时检查原始 ELF 一致性。
4. Android 与桌面在 Git、Shell、常用命令和 CA 信任链方面存在环境差异，需要逐一交付对应的 ARM64 组件或经测试的等效实现。
5. Android 长任务前台服务涉及 `dataSync` 后台运行时长限制，必须处理超时、恢复和进度通知。

## 自动化验证入口

- `python3 android-app/tools/generate_protocol_catalog.py --check`：接口目录与官方源码一致。
- `python3 .github/scripts/check_android_parity.py`：静态接口/专用界面覆盖审计，不将静态映射算作功能验收。
- `python3 .github/scripts/android_smoke.py --apk <path>`：连接 Android 17 ARM64 设备后检查 ELF、APK 安装、启动和 RPC 握手。
- `.github/workflows/android-codex.yml`：源码构建、UI 编译、原生核心及 APK 打包测试。

## 签收原则

只将“完整 APK 含真实 Codex 原生核心 + Android 17 设备启动 + 主要功能逐项测试 + 性能验证”标记为可发布。出现缺失即记录对应链路并修复；保留构建日志和可重复验证步骤。
