# 版本管理

## 当前版本

| 对象 | 版本 | 说明 |
|---|---|---|
| workspace / 全部 crate | `0.3.0` | 引擎与工具链版本，单一来源 |
| IR 格式 | `1.1` | `charta::IR_VERSION`，JSON 契约 |
| 报告格式 | `1.0` | `charta::REPORT_VERSION`，JSON 契约 |
| CLI | `0.3.0` | 与 workspace 同步 |

## 单一来源

- 所有 crate 使用 `version.workspace = true`，版本只在根 `Cargo.toml` 的 `[workspace.package]` 中维护。
- 内部 crate 依赖在根 `Cargo.toml` 的 `[workspace.dependencies]` 中同时声明 `path` 与 `version`，避免各 crate 各自写版本。
- `charta --version`、报告中的 `engine_version`、`docProps/app.xml` 的 `AppVersion` 均从 Cargo 包版本派生，不手工重复。
- IR 与报告格式各自有独立版本常量；结构变更时单独递增，不随引擎版本自动变化。

## 语义化版本策略

- **0.x 阶段**：workspace 内所有 crate 锁步发布，内部 API 允许在次版本号中调整；补丁版本只做兼容修复。
- **1.0 之后**：核心 crate 遵循 SemVer；删除/重命名公开类型、trait 方法或 JSON 字段属于破坏性变更。
- **IR 格式**：新增可选字段为兼容变更；删除字段、改变字段含义或改变节点结构必须递增 `IR_VERSION` 主版本。
- **报告格式**：`IssueDetails` 新增变体或字段为兼容变更；已有字段改名/删除或 severity 语义变化必须递增 `REPORT_VERSION` 主版本。
- **docx-kit**：与核心保持同一 minor 系列（0.2.x 对应核心 0.2.x）；作为独立 crate 可被其他项目单独依赖。

## 版本递增流程

```bash
# 1. 更新 workspace 版本与内部依赖版本
bash scripts/bump-version.sh 0.3.0

# 2. 更新 CHANGELOG.md，把 Unreleased 内容归入新版本
# 3. 全量检查
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 4. 提交并打标签（仅在确认后执行）
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: v0.3.0"
git tag v0.3.0
```

`scripts/bump-version.sh` 只修改根 `Cargo.toml` 的版本号，不自动改 CHANGELOG、不提交、不打标签，避免把发布决策自动化。

## 发布检查清单

- [ ] `Cargo.lock` 已更新并纳入版本库
- [ ] `CHANGELOG.md` 已记录新增、变更、修复与已知限制
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [ ] `cargo test --workspace` 通过
- [ ] 示例 `notice.md`、`notice.typ` 可生成 docx 与报告
- [ ] IR 与报告格式版本是否需要递增已确认
- [ ] 标签名与版本号一致（`vX.Y.Z`）
