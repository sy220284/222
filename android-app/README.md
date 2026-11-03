# Codex 移动版（Android 原生移植）

该 Android 工程与上游 `codex-rs` 处于同一仓库。原生核心必须由仓库中的实际 Codex CLI 源码交叉编译生成，绝不可使用不含核心的占位 APK。

## 架构

- Java / Android SDK：中文触控 UI、会话管理、工作区、文件导入、取消任务
- Rust Codex CLI：从 `codex-rs/app-server` 为 `aarch64-linux-android` 编译
- 通过 `codex exec --json` 启动原生核心；JSONL 事件由界面消费，`exec resume` 恢复会话
- `libcodex.so` 是 Android 原生库目录中的 **可执行 ELF**（来自 Codex CLI 的 PIE 程序），并非 JNI .so。利用 Android APK 原生库可执行路径加载，由 Java `ProcessBuilder` 创建独立执行进程
- Codex 状态和工作区只存在应用私有存储中，API 密钥由 Android Keystore AES-GCM 加密

## 构建

需 JDK 17、Gradle 8.13、Android SDK 36、Android NDK 27.2.12479018、Rust 1.95.0 的 aarch64-linux-android target。

先在 `codex-rs` 交叉编译 `codex-app-server` 的 `codex-app-server`，复制生成的 ELF 到：
`android-app/app/src/main/jniLibs/arm64-v8a/libcodex.so`。
然后在 `android-app` 下运行 `gradle :app:assembleDebug`。

主线工作流 `.github/workflows/android-codex.yml` 应验证 ELF AArch64/PIE、生成 APK、检查 APK 中包含真实核心后才发布构建产物。

## 运行限制与验证

上游 Codex 官方原生平台仅包含 macOS/Linux/Windows，Android 移植需要编译和设备验证。Android 不提供 Linux 的完整桌面命令环境、Landlock 或常规 Git 工具。Codex 可以在应用私有工作区读写文件，但依赖的系统命令或额外工具必须逐项适配。出包成功只证明可安装，不等于完整功能和设备运行验收。

应用目前以 API Key 登录，未实现 ChatGPT 账号 OAuth；不可将凭据写入仓库。
