#!/usr/bin/env bash
# 222 离线工具链安装器，按仓库 Rust 1.99.1 版本验收
set -Eeuo pipefail
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  echo '用法：bash install-bundle.sh <解压后的工作流产物目录> [安装目录]'
  exit 0
fi
[[ $# -ge 1 && $# -le 2 ]] || { echo '需要提供解压后的产物目录' >&2; exit 2; }
ARTIFACT="$(cd "$1" && pwd -P)"
DEST="${2:-$HOME/.local/share/222-rust-1.99.1}"
[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || { echo '仅支持 Linux x86_64' >&2; exit 1; }
for tool in sha256sum tar mktemp grep; do command -v "$tool" >/dev/null || { echo "缺少 $tool" >&2; exit 1; }; done
for file in rust-toolchain-linux-x86_64.tar.gz cargo-cache.tar.gz SHA256SUMS; do
  [[ -f "$ARTIFACT/$file" ]] || { echo "产物缺失: $file" >&2; exit 1; }
done
(cd "$ARTIFACT" && sha256sum --check --status SHA256SUMS) || { echo 'SHA256 验证失败' >&2; exit 1; }
[[ ! -e "$DEST" ]] || { echo "为避免覆盖本地工具链，目标已存在: $DEST" >&2; exit 1; }
mkdir -p "$(dirname "$DEST")"
TEMP="$(mktemp -d "$(dirname "$DEST")/.222-rust-install.XXXXXXXX")"
trap '[[ -z "${TEMP:-}" ]] || rm -rf -- "$TEMP"' EXIT
mkdir -p "$TEMP/toolchains" "$TEMP/cargo"
tar -xzf "$ARTIFACT/rust-toolchain-linux-x86_64.tar.gz" -C "$TEMP/toolchains" --no-same-owner --no-same-permissions
tar -xzf "$ARTIFACT/cargo-cache.tar.gz" -C "$TEMP/cargo" --no-same-owner --no-same-permissions
BIN="$TEMP/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin"
for tool in rustc cargo rustfmt cargo-clippy; do [[ -x "$BIN/$tool" ]] || { echo "缺少 $tool" >&2; exit 1; }; done
"$BIN/rustc" --version | grep -q '^rustc 1\.99\.1 ' || { echo 'Rust 版本不符' >&2; exit 1; }
"$BIN/cargo" --version
"$BIN/rustfmt" --version
"$BIN/cargo-clippy" --version
cat > "$TEMP/env.sh" <<'ENV'
# 在当前 Shell 中加载 222 离线 Rust 1.99.1
_222_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
export CARGO_HOME="$_222_ROOT/cargo"
export PATH="$_222_ROOT/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
unset _222_ROOT
ENV
cp "$ARTIFACT/SHA256SUMS" "$TEMP/ARTIFACT-SHA256SUMS"
[[ ! -f "$ARTIFACT/manifest.txt" ]] || cp "$ARTIFACT/manifest.txt" "$TEMP/manifest.txt"
mv "$TEMP" "$DEST"
TEMP=''
printf '安装完成：%s\n执行 source "%s/env.sh"\n' "$DEST" "$DEST"
