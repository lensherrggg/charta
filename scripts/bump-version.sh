#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "用法: bash scripts/bump-version.sh <新版本号>" >&2
  echo "示例: bash scripts/bump-version.sh 0.2.0" >&2
  exit 2
fi

new_version="$1"
if ! [[ "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
  echo "版本号不合法: $new_version（期望 SemVer，例如 0.2.0 或 0.2.0-rc.1）" >&2
  exit 2
fi

cd "$(dirname "$0")/.."

old_version=$(grep -m1 '^version = "' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')
if [ -z "$old_version" ]; then
  echo "无法从 Cargo.toml 读取当前版本" >&2
  exit 1
fi
if [ "$old_version" = "$new_version" ]; then
  echo "版本号未变化: $old_version" >&2
  exit 1
fi

perl -0pi -e 's/(^version = )"[^"]+"/${1}"'"$new_version"'"/mg' Cargo.toml
perl -0pi -e 's/(, version = )"[^"]+"/${1}"'"$new_version"'"/g' Cargo.toml

echo "workspace 版本: $old_version -> $new_version"
echo "下一步:"
echo "  1. 更新 CHANGELOG.md"
echo "  2. cargo fmt --all -- --check"
echo "  3. cargo clippy --workspace --all-targets -- -D warnings"
echo "  4. cargo test --workspace"
