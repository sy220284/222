# 222 Rust 工具链离线恢复（复用 777 缓存模式）

222 仓库锁定 Rust 1.99.1、rustfmt、clippy。已有工作流仅运行检查，没有发布工具链 Artifact；777 的现成产物主要用于 Android 测试，不能直接编译 Rust。

本方案沿用 777 的固定版本、缓存、校验、工件上传思路，替换为 Rust 完整工具链与 Cargo.lock 的离线依赖缓存。

## 生成工件

GitHub Actions -> **222 Rust 1.99.1 离线工具链产物**。可由此 PR 或分支推送启动（没有任何定时计划）。上传工件名称：

`222-rust-1.99.1-linux-x86_64-offline`

包含：完整 Rust sysroot、rustc、cargo、rustfmt、clippy、Linux x86_64 所需 Cargo registry/git 缓存、SHA256SUMS、安装脚本、版本/来源清单。保留期 30 天。

## 本地使用

下载并解压工件 ZIP 到 `/path/to/artifact`。执行：

```bash
bash /path/to/artifact/install-bundle.sh /path/to/artifact "$HOME/.local/share/222-rust-1.99.1"
source "$HOME/.local/share/222-rust-1.99.1/env.sh"
rustc --version
cargo --version
cargo fmt --version
cargo clippy --version
```

拥有 222 仓库源代码后，进入仓库目录执行：

```bash
cargo metadata --offline --locked --format-version 1 --no-deps
cargo check --offline --locked --all-targets
python3 scripts/port_workbench.py check --repo .
```

安装器仅支持 Linux x86_64（glibc），会核对包内 SHA256 与 Rust 版本，不需要访问 GitHub / Rust 下载站 / crates.io，也不会覆盖原有安装目录。编译 Rust 项目需主机提供 C linker；迁移工作台需 Python3。

注：有 Artifact 只能证明离线打包已完成。项目编译是否成功，以工作流中 `cargo check --offline --locked --all-targets` 的实际执行结果为准。