# 2.1.88 Rust 重写明确排除的 192 项与原因

> 根据当前 PR #6 分支的唯一事实源 `docs/FILE_PORT_MATRIX.md` 和用户批准的 `docs/REMOVED_SCOPE.json` 逐条列示。原有 22 条 + 2026-10-30 追加授权 170 条 = **192 条**。本文件仅披露现有批准范围，不扩大排除、不把缺失功能自动判定为排除。

## 保留通用功能的边界

原厂账号登录、厂商模型独有 API、官方云端额度/营销、专属遥测和跨设备 Bridge/托管任务等已明确排除；**本地文件、Git、模型 API 多提供商、普通 SDK、MCP、Hooks、会话、Agent、用户主动触发的本地任务、普通代码审查与本地规划仍需迁移**。是否排除以本页具体路径和仓库授权为准，禁止按整个目录名扩大。

## 原因分布（按原登记原因，不合并不同理由）

| 原因 | 文件数 |
|---|---:|
| 跨设备远程 Bridge | 31 |
| 排除：行为／环境遥测回传 | 22 |
| 厂商托管 GitHub App 安装流程；普通 Git 保留 | 14 |
| 远程 SDK 专属传输 | 7 |
| 云端 Teleport 界面 | 5 |
| 原厂服务端托管设置；本地设置保留 | 5 |
| 团队记忆云端同步（保留本地记忆） | 5 |
| 官方超额用量付费入口 | 4 |
| 远程会话与权限桥接 | 4 |
| 官方云端 Teleport API | 4 |
| 官方远程环境初始化 | 3 |
| 云端直连会话 | 3 |
| 厂商语音流服务 | 3 |
| 远程控制 CLI 命令 | 2 |
| 原厂在线反馈上传 | 2 |
| 厂商托管 Slack 自动化 | 2 |
| 原厂账号登录 | 2 |
| 原厂账号退出 | 2 |
| 官方云端移动端伴随入口 | 2 |
| 官方订阅权益通行证 | 2 |
| 原厂订阅额度 | 2 |
| 官方远程环境配置 | 2 |
| 云端专项审查（保留普通本地审查） | 2 |
| 官方远程会话链接命令；本地会话保留 | 2 |
| 原厂订阅升级 | 2 |
| 官方云端语音 CLI | 2 |
| 远程任务视图 | 2 |
| 设置云端同步（保留本地设置） | 2 |
| 远程代理转发 | 2 |
| 远程后台 Agent | 2 |
| 远程会话标准输入输出 | 1 |
| 远程 Bridge 诊断 | 1 |
| 官方 Teleport 入口 | 1 |
| 远程 Bridge 状态界面 | 1 |
| 官方桌面客户端营销 | 1 |
| 官方推荐权益营销 | 1 |
| 官方超额信用额度营销 | 1 |
| 官方语音提示 | 1 |
| 官方语音状态组件 | 1 |
| 远程控制开通提示 | 1 |
| 远程环境管理界面 | 1 |
| 官方订阅窗口用量界面；保留通用 API 用量及费用 | 1 |
| Claude.ai 托管 MCP 菜单；本地标准 MCP 保留 | 1 |
| 官方语音状态 | 1 |
| Claude.ai 订阅切换提示 | 1 |
| 远程会话 Hook | 1 |
| 远程 Bridge Hook | 1 |
| 云端 Teleport 恢复 | 1 |
| 官方云端语音录入 | 1 |
| 官方云端语音资格 | 1 |
| 官方语音集成 | 1 |
| 远程控制迁移 | 1 |
| 官方组织管理员申请 API | 1 |
| 原厂 CLI 服务端启动配置 | 1 |
| 原厂云端文件服务（保留本地文件） | 1 |
| 原厂组织首次使用数据 | 1 |
| 官方账户信息服务 | 1 |
| 原厂服务端统计开关 | 1 |
| 官方超额用量额度服务 | 1 |
| 原厂推荐奖励 | 1 |
| 官方远程会话持久化 API | 1 |
| 官方云端审查配额 | 1 |
| 官方订阅配额 API；本地费用统计保留 | 1 |
| Claude.ai 订阅限额；保留第三方 API 用量统计 | 1 |
| Claude.ai 订阅限额 Hook | 1 |
| Claude.ai 组织托管 MCP；保留标准 MCP | 1 |
| 远程 Agent 计划任务技能 | 1 |
| 远程 Agent 执行任务 | 1 |
| 仅用于原厂 Statsig 内部行为及环境遥测的事件类型 | 1 |
| 仅为原厂事件遥测服务提供账户组织标签的类型，已无合法消费方 | 1 |
| 原厂实验遥测类型 | 1 |
| 原厂更新服务 | 1 |
| Claude.ai 订阅计费权限 | 1 |
| Claude.ai 超额订阅计费 | 1 |
| 远程会话凭据注入 | 1 |
| 云端 Teleport 运行界面 | 1 |
| 官方云端规划专用工具 | 1 |
| 厂商语音模式授权 | 1 |

## 逐项排除记录

| 原源码路径 | 排除原因 | 授权归属 | 迁移矩阵状态 |
|---|---|---|---|
| `src/bridge/bridgeApi.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeConfig.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeDebug.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeEnabled.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeMain.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeMessaging.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgePermissionCallbacks.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgePointer.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeStatusUtil.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/bridgeUI.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/capacityWake.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/codeSessionApi.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/createSession.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/debugUtils.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/envLessBridgeConfig.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/flushGate.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/inboundAttachments.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/inboundMessages.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/initReplBridge.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/jwtUtils.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/pollConfig.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/pollConfigDefaults.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/remoteBridgeCore.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/replBridge.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/replBridgeHandle.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/replBridgeTransport.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/sessionIdCompat.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/sessionRunner.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/trustedDevice.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/types.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/bridge/workSecret.ts` | 跨设备远程 Bridge | 2026-10-30 追加授权 | 按用户要求删除；跨设备远程 Bridge；不再跟进，通用本地能力保留 |
| `src/cli/remoteIO.ts` | 远程会话标准输入输出 | 2026-10-30 追加授权 | 按用户要求删除；远程会话标准输入输出；不再跟进，通用本地能力与模型 API 保留 |
| `src/cli/transports/HybridTransport.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/SSETransport.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/SerialBatchEventUploader.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/WebSocketTransport.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/WorkerStateUploader.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/ccrClient.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/cli/transports/transportUtils.ts` | 远程 SDK 专属传输 | 2026-10-30 追加授权 | 按用户要求删除；远程 SDK 专属传输；不再跟进，通用本地能力保留 |
| `src/commands/bridge-kick.ts` | 远程 Bridge 诊断 | 2026-10-30 追加授权 | 按用户要求删除；远程 Bridge 诊断；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/bridge/bridge.tsx` | 远程控制 CLI 命令 | 2026-10-30 追加授权 | 按用户要求删除；远程控制 CLI 命令；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/bridge/index.ts` | 远程控制 CLI 命令 | 2026-10-30 追加授权 | 按用户要求删除；远程控制 CLI 命令；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/extra-usage/extra-usage-core.ts` | 官方超额用量付费入口 | 2026-10-30 追加授权 | 按用户要求删除；官方超额用量付费入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/extra-usage/extra-usage-noninteractive.ts` | 官方超额用量付费入口 | 2026-10-30 追加授权 | 按用户要求删除；官方超额用量付费入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/extra-usage/extra-usage.tsx` | 官方超额用量付费入口 | 2026-10-30 追加授权 | 按用户要求删除；官方超额用量付费入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/extra-usage/index.ts` | 官方超额用量付费入口 | 2026-10-30 追加授权 | 按用户要求删除；官方超额用量付费入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/feedback/feedback.tsx` | 原厂在线反馈上传 | 2026-10-30 追加授权 | 按用户要求删除；原厂在线反馈上传；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/feedback/index.ts` | 原厂在线反馈上传 | 2026-10-30 追加授权 | 按用户要求删除；原厂在线反馈上传；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/ApiKeyStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/CheckExistingSecretStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/CheckGitHubStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/ChooseRepoStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/CreatingStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/ErrorStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/ExistingWorkflowStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/InstallAppStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/OAuthFlowStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/SuccessStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/WarningsStep.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/index.ts` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/install-github-app.tsx` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-github-app/setupGitHubActions.ts` | 厂商托管 GitHub App 安装流程；普通 Git 保留 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 GitHub App 安装流程；普通 Git 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-slack-app/index.ts` | 厂商托管 Slack 自动化 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 Slack 自动化；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/install-slack-app/install-slack-app.ts` | 厂商托管 Slack 自动化 | 2026-10-30 追加授权 | 按用户要求删除；厂商托管 Slack 自动化；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/login/index.ts` | 原厂账号登录 | 2026-10-30 追加授权 | 按用户要求删除；原厂账号登录；不再跟进，通用本地能力保留 |
| `src/commands/login/login.tsx` | 原厂账号登录 | 2026-10-30 追加授权 | 按用户要求删除；原厂账号登录；不再跟进，通用本地能力保留 |
| `src/commands/logout/index.ts` | 原厂账号退出 | 2026-10-30 追加授权 | 按用户要求删除；原厂账号退出；不再跟进，通用本地能力保留 |
| `src/commands/logout/logout.tsx` | 原厂账号退出 | 2026-10-30 追加授权 | 按用户要求删除；原厂账号退出；不再跟进，通用本地能力保留 |
| `src/commands/mobile/index.ts` | 官方云端移动端伴随入口 | 2026-10-30 追加授权 | 按用户要求删除；官方云端移动端伴随入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/mobile/mobile.tsx` | 官方云端移动端伴随入口 | 2026-10-30 追加授权 | 按用户要求删除；官方云端移动端伴随入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/passes/index.ts` | 官方订阅权益通行证 | 2026-10-30 追加授权 | 按用户要求删除；官方订阅权益通行证；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/passes/passes.tsx` | 官方订阅权益通行证 | 2026-10-30 追加授权 | 按用户要求删除；官方订阅权益通行证；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/rate-limit-options/index.ts` | 原厂订阅额度 | 2026-10-30 追加授权 | 按用户要求删除；原厂订阅额度；不再跟进，通用本地能力保留 |
| `src/commands/rate-limit-options/rate-limit-options.tsx` | 原厂订阅额度 | 2026-10-30 追加授权 | 按用户要求删除；原厂订阅额度；不再跟进，通用本地能力保留 |
| `src/commands/remote-env/index.ts` | 官方远程环境配置 | 2026-10-30 追加授权 | 按用户要求删除；官方远程环境配置；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/remote-env/remote-env.tsx` | 官方远程环境配置 | 2026-10-30 追加授权 | 按用户要求删除；官方远程环境配置；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/remote-setup/api.ts` | 官方远程环境初始化 | 2026-10-30 追加授权 | 按用户要求删除；官方远程环境初始化；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/remote-setup/index.ts` | 官方远程环境初始化 | 2026-10-30 追加授权 | 按用户要求删除；官方远程环境初始化；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/remote-setup/remote-setup.tsx` | 官方远程环境初始化 | 2026-10-30 追加授权 | 按用户要求删除；官方远程环境初始化；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/review/UltrareviewOverageDialog.tsx` | 云端专项审查（保留普通本地审查） | 2026-10-30 追加授权 | 按用户要求删除；云端专项审查（保留普通本地审查）；不再跟进，通用本地能力保留 |
| `src/commands/review/reviewRemote.ts` | 云端专项审查（保留普通本地审查） | 2026-10-30 追加授权 | 按用户要求删除；云端专项审查（保留普通本地审查）；不再跟进，通用本地能力保留 |
| `src/commands/session/index.ts` | 官方远程会话链接命令；本地会话保留 | 2026-10-30 追加授权 | 按用户要求删除；官方远程会话链接命令；本地会话保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/session/session.tsx` | 官方远程会话链接命令；本地会话保留 | 2026-10-30 追加授权 | 按用户要求删除；官方远程会话链接命令；本地会话保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/teleport/index.js` | 官方 Teleport 入口 | 2026-10-30 追加授权 | 按用户要求删除；官方 Teleport 入口；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/upgrade/index.ts` | 原厂订阅升级 | 2026-10-30 追加授权 | 按用户要求删除；原厂订阅升级；不再跟进，通用本地能力保留 |
| `src/commands/upgrade/upgrade.tsx` | 原厂订阅升级 | 2026-10-30 追加授权 | 按用户要求删除；原厂订阅升级；不再跟进，通用本地能力保留 |
| `src/commands/voice/index.ts` | 官方云端语音 CLI | 2026-10-30 追加授权 | 按用户要求删除；官方云端语音 CLI；不再跟进，通用本地能力与模型 API 保留 |
| `src/commands/voice/voice.ts` | 官方云端语音 CLI | 2026-10-30 追加授权 | 按用户要求删除；官方云端语音 CLI；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/BridgeDialog.tsx` | 远程 Bridge 状态界面 | 2026-10-30 追加授权 | 按用户要求删除；远程 Bridge 状态界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/DesktopUpsell/DesktopUpsellStartup.tsx` | 官方桌面客户端营销 | 2026-10-30 追加授权 | 按用户要求删除；官方桌面客户端营销；不再跟进 |
| `src/components/LogoV2/GuestPassesUpsell.tsx` | 官方推荐权益营销 | 2026-10-30 追加授权 | 按用户要求删除；官方推荐权益营销；不再跟进 |
| `src/components/LogoV2/OverageCreditUpsell.tsx` | 官方超额信用额度营销 | 2026-10-30 追加授权 | 按用户要求删除；官方超额信用额度营销；不再跟进 |
| `src/components/LogoV2/VoiceModeNotice.tsx` | 官方语音提示 | 2026-10-30 追加授权 | 按用户要求删除；官方语音提示；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/PromptInput/VoiceIndicator.tsx` | 官方语音状态组件 | 2026-10-30 追加授权 | 按用户要求删除；官方语音状态组件；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/RemoteCallout.tsx` | 远程控制开通提示 | 2026-10-30 追加授权 | 按用户要求删除；远程控制开通提示；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/RemoteEnvironmentDialog.tsx` | 远程环境管理界面 | 2026-10-30 追加授权 | 按用户要求删除；远程环境管理界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/Settings/Usage.tsx` | 官方订阅窗口用量界面；保留通用 API 用量及费用 | 2026-10-30 追加授权 | 按用户要求删除；官方订阅窗口用量界面；保留通用 API 用量及费用；不再跟进 |
| `src/components/TeleportError.tsx` | 云端 Teleport 界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/TeleportProgress.tsx` | 云端 Teleport 界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/TeleportRepoMismatchDialog.tsx` | 云端 Teleport 界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/TeleportResumeWrapper.tsx` | 云端 Teleport 界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/TeleportStash.tsx` | 云端 Teleport 界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/mcp/MCPRemoteServerMenu.tsx` | Claude.ai 托管 MCP 菜单；本地标准 MCP 保留 | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 托管 MCP 菜单；本地标准 MCP 保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/tasks/RemoteSessionDetailDialog.tsx` | 远程任务视图 | 2026-10-30 追加授权 | 按用户要求删除；远程任务视图；不再跟进，通用本地能力与模型 API 保留 |
| `src/components/tasks/RemoteSessionProgress.tsx` | 远程任务视图 | 2026-10-30 追加授权 | 按用户要求删除；远程任务视图；不再跟进，通用本地能力与模型 API 保留 |
| `src/context/voice.tsx` | 官方语音状态 | 2026-10-30 追加授权 | 按用户要求删除；官方语音状态；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/notifs/useCanSwitchToExistingSubscription.tsx` | Claude.ai 订阅切换提示 | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 订阅切换提示；不再跟进 |
| `src/hooks/useRemoteSession.ts` | 远程会话 Hook | 2026-10-30 追加授权 | 按用户要求删除；远程会话 Hook；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/useReplBridge.tsx` | 远程 Bridge Hook | 2026-10-30 追加授权 | 按用户要求删除；远程 Bridge Hook；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/useTeleportResume.tsx` | 云端 Teleport 恢复 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 恢复；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/useVoice.ts` | 官方云端语音录入 | 2026-10-30 追加授权 | 按用户要求删除；官方云端语音录入；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/useVoiceEnabled.ts` | 官方云端语音资格 | 2026-10-30 追加授权 | 按用户要求删除；官方云端语音资格；不再跟进，通用本地能力与模型 API 保留 |
| `src/hooks/useVoiceIntegration.tsx` | 官方语音集成 | 2026-10-30 追加授权 | 按用户要求删除；官方语音集成；不再跟进，通用本地能力与模型 API 保留 |
| `src/migrations/migrateReplBridgeEnabledToRemoteControlAtStartup.ts` | 远程控制迁移 | 2026-10-30 追加授权 | 按用户要求删除；远程控制迁移；不再跟进，通用本地能力与模型 API 保留 |
| `src/remote/RemoteSessionManager.ts` | 远程会话与权限桥接 | 2026-10-30 追加授权 | 按用户要求删除；远程会话与权限桥接；不再跟进，通用本地能力保留 |
| `src/remote/SessionsWebSocket.ts` | 远程会话与权限桥接 | 2026-10-30 追加授权 | 按用户要求删除；远程会话与权限桥接；不再跟进，通用本地能力保留 |
| `src/remote/remotePermissionBridge.ts` | 远程会话与权限桥接 | 2026-10-30 追加授权 | 按用户要求删除；远程会话与权限桥接；不再跟进，通用本地能力保留 |
| `src/remote/sdkMessageAdapter.ts` | 远程会话与权限桥接 | 2026-10-30 追加授权 | 按用户要求删除；远程会话与权限桥接；不再跟进，通用本地能力保留 |
| `src/server/createDirectConnectSession.ts` | 云端直连会话 | 2026-10-30 追加授权 | 按用户要求删除；云端直连会话；不再跟进，通用本地能力保留 |
| `src/server/directConnectManager.ts` | 云端直连会话 | 2026-10-30 追加授权 | 按用户要求删除；云端直连会话；不再跟进，通用本地能力保留 |
| `src/server/types.ts` | 云端直连会话 | 2026-10-30 追加授权 | 按用户要求删除；云端直连会话；不再跟进，通用本地能力保留 |
| `src/services/analytics/config.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/datadog.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/firstPartyEventLogger.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/firstPartyEventLoggingExporter.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/growthbook.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/index.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/metadata.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/sink.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/analytics/sinkKillswitch.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/api/adminRequests.ts` | 官方组织管理员申请 API | 2026-10-30 追加授权 | 按用户要求删除；官方组织管理员申请 API；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/bootstrap.ts` | 原厂 CLI 服务端启动配置 | 2026-10-30 追加授权 | 按用户要求删除；原厂 CLI 服务端启动配置；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/filesApi.ts` | 原厂云端文件服务（保留本地文件） | 2026-10-30 追加授权 | 按用户要求删除；原厂云端文件服务（保留本地文件）；不再跟进，通用本地能力保留 |
| `src/services/api/firstTokenDate.ts` | 原厂组织首次使用数据 | 2026-10-30 追加授权 | 按用户要求删除；原厂组织首次使用数据；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/grove.ts` | 官方账户信息服务 | 2026-10-30 追加授权 | 按用户要求删除；官方账户信息服务；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/metricsOptOut.ts` | 原厂服务端统计开关 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端统计开关；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/overageCreditGrant.ts` | 官方超额用量额度服务 | 2026-10-30 追加授权 | 按用户要求删除；官方超额用量额度服务；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/referral.ts` | 原厂推荐奖励 | 2026-10-30 追加授权 | 按用户要求删除；原厂推荐奖励；不再跟进，通用本地能力保留 |
| `src/services/api/sessionIngress.ts` | 官方远程会话持久化 API | 2026-10-30 追加授权 | 按用户要求删除；官方远程会话持久化 API；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/ultrareviewQuota.ts` | 官方云端审查配额 | 2026-10-30 追加授权 | 按用户要求删除；官方云端审查配额；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/api/usage.ts` | 官方订阅配额 API；本地费用统计保留 | 2026-10-30 追加授权 | 按用户要求删除；官方订阅配额 API；本地费用统计保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/claudeAiLimits.ts` | Claude.ai 订阅限额；保留第三方 API 用量统计 | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 订阅限额；保留第三方 API 用量统计；不再跟进 |
| `src/services/claudeAiLimitsHook.ts` | Claude.ai 订阅限额 Hook | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 订阅限额 Hook；不再跟进 |
| `src/services/diagnosticTracking.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/services/mcp/claudeai.ts` | Claude.ai 组织托管 MCP；保留标准 MCP | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 组织托管 MCP；保留标准 MCP；不再跟进 |
| `src/services/remoteManagedSettings/index.ts` | 原厂服务端托管设置；本地设置保留 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端托管设置；本地设置保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/remoteManagedSettings/securityCheck.tsx` | 原厂服务端托管设置；本地设置保留 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端托管设置；本地设置保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/remoteManagedSettings/syncCache.ts` | 原厂服务端托管设置；本地设置保留 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端托管设置；本地设置保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/remoteManagedSettings/syncCacheState.ts` | 原厂服务端托管设置；本地设置保留 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端托管设置；本地设置保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/remoteManagedSettings/types.ts` | 原厂服务端托管设置；本地设置保留 | 2026-10-30 追加授权 | 按用户要求删除；原厂服务端托管设置；本地设置保留；不再跟进，通用本地能力与模型 API 保留 |
| `src/services/settingsSync/index.ts` | 设置云端同步（保留本地设置） | 2026-10-30 追加授权 | 按用户要求删除；设置云端同步（保留本地设置）；不再跟进，通用本地能力保留 |
| `src/services/settingsSync/types.ts` | 设置云端同步（保留本地设置） | 2026-10-30 追加授权 | 按用户要求删除；设置云端同步（保留本地设置）；不再跟进，通用本地能力保留 |
| `src/services/teamMemorySync/index.ts` | 团队记忆云端同步（保留本地记忆） | 2026-10-30 追加授权 | 按用户要求删除；团队记忆云端同步（保留本地记忆）；不再跟进，通用本地能力保留 |
| `src/services/teamMemorySync/secretScanner.ts` | 团队记忆云端同步（保留本地记忆） | 2026-10-30 追加授权 | 按用户要求删除；团队记忆云端同步（保留本地记忆）；不再跟进，通用本地能力保留 |
| `src/services/teamMemorySync/teamMemSecretGuard.ts` | 团队记忆云端同步（保留本地记忆） | 2026-10-30 追加授权 | 按用户要求删除；团队记忆云端同步（保留本地记忆）；不再跟进，通用本地能力保留 |
| `src/services/teamMemorySync/types.ts` | 团队记忆云端同步（保留本地记忆） | 2026-10-30 追加授权 | 按用户要求删除；团队记忆云端同步（保留本地记忆）；不再跟进，通用本地能力保留 |
| `src/services/teamMemorySync/watcher.ts` | 团队记忆云端同步（保留本地记忆） | 2026-10-30 追加授权 | 按用户要求删除；团队记忆云端同步（保留本地记忆）；不再跟进，通用本地能力保留 |
| `src/services/voice.ts` | 厂商语音流服务 | 2026-10-30 追加授权 | 按用户要求删除；厂商语音流服务；不再跟进，通用本地能力保留 |
| `src/services/voiceKeyterms.ts` | 厂商语音流服务 | 2026-10-30 追加授权 | 按用户要求删除；厂商语音流服务；不再跟进，通用本地能力保留 |
| `src/services/voiceStreamSTT.ts` | 厂商语音流服务 | 2026-10-30 追加授权 | 按用户要求删除；厂商语音流服务；不再跟进，通用本地能力保留 |
| `src/skills/bundled/scheduleRemoteAgents.ts` | 远程 Agent 计划任务技能 | 2026-10-30 追加授权 | 按用户要求删除；远程 Agent 计划任务技能；不再跟进，通用本地能力与模型 API 保留 |
| `src/tasks/RemoteAgentTask/RemoteAgentTask.tsx` | 远程 Agent 执行任务 | 2026-10-30 追加授权 | 按用户要求删除；远程 Agent 执行任务；不再跟进，通用本地能力与模型 API 保留 |
| `src/types/generated/events_mono/claude_code/v1/claude_code_internal_event.ts` | 仅用于原厂 Statsig 内部行为及环境遥测的事件类型 | 2026-10-30 追加授权 | 按用户要求删除；仅用于原厂 Statsig 内部行为及环境遥测的事件类型；不再跟进 |
| `src/types/generated/events_mono/common/v1/auth.ts` | 仅为原厂事件遥测服务提供账户组织标签的类型，已无合法消费方 | 2026-10-30 追加授权 | 按用户要求删除；仅为原厂事件遥测服务提供账户组织标签的类型，已无合法消费方；不再跟进 |
| `src/types/generated/events_mono/growthbook/v1/growthbook_experiment_event.ts` | 原厂实验遥测类型 | 2026-10-30 追加授权 | 按用户要求删除；原厂实验遥测类型；不再跟进 |
| `src/upstreamproxy/relay.ts` | 远程代理转发 | 2026-10-30 追加授权 | 按用户要求删除；远程代理转发；不再跟进，通用本地能力保留 |
| `src/upstreamproxy/upstreamproxy.ts` | 远程代理转发 | 2026-10-30 追加授权 | 按用户要求删除；远程代理转发；不再跟进，通用本地能力保留 |
| `src/utils/autoUpdater.ts` | 原厂更新服务 | 2026-10-30 追加授权 | 按用户要求删除；原厂更新服务；不再跟进，通用本地能力保留 |
| `src/utils/background/remote/preconditions.ts` | 远程后台 Agent | 2026-10-30 追加授权 | 按用户要求删除；远程后台 Agent；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/background/remote/remoteSession.ts` | 远程后台 Agent | 2026-10-30 追加授权 | 按用户要求删除；远程后台 Agent；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/billing.ts` | Claude.ai 订阅计费权限 | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 订阅计费权限；不再跟进 |
| `src/utils/extraUsage.ts` | Claude.ai 超额订阅计费 | 2026-10-30 追加授权 | 按用户要求删除；Claude.ai 超额订阅计费；不再跟进 |
| `src/utils/fileOperationAnalytics.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/fpsTracker.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/sessionIngressAuth.ts` | 远程会话凭据注入 | 2026-10-30 追加授权 | 按用户要求删除；远程会话凭据注入；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/telemetry/betaSessionTracing.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/bigqueryExporter.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/events.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/instrumentation.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/logger.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/perfettoTracing.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/pluginTelemetry.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/sessionTracing.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetry/skillLoadedEvent.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/telemetryAttributes.ts` | 排除：行为／环境遥测回传 | 此前已排除 | 按用户要求删除；不重写外发通道 |
| `src/utils/teleport.tsx` | 云端 Teleport 运行界面 | 2026-10-30 追加授权 | 按用户要求删除；云端 Teleport 运行界面；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/teleport/api.ts` | 官方云端 Teleport API | 2026-10-30 追加授权 | 按用户要求删除；官方云端 Teleport API；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/teleport/environmentSelection.ts` | 官方云端 Teleport API | 2026-10-30 追加授权 | 按用户要求删除；官方云端 Teleport API；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/teleport/environments.ts` | 官方云端 Teleport API | 2026-10-30 追加授权 | 按用户要求删除；官方云端 Teleport API；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/teleport/gitBundle.ts` | 官方云端 Teleport API | 2026-10-30 追加授权 | 按用户要求删除；官方云端 Teleport API；不再跟进，通用本地能力与模型 API 保留 |
| `src/utils/ultraplan/ccrSession.ts` | 官方云端规划专用工具 | 2026-10-30 追加授权 | 按用户要求删除；官方云端规划专用工具；不再跟进，通用本地能力与模型 API 保留 |
| `src/voice/voiceModeEnabled.ts` | 厂商语音模式授权 | 2026-10-30 追加授权 | 按用户要求删除；厂商语音模式授权；不再跟进，通用本地能力保留 |
