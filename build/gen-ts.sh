#!/bin/bash
# 生成前端 TS 类型契约(target/bindings)
#
# ts-rs 是**测试期**导出:每个 `#[ts(export)]` 类型会生成一个
# `export_bindings_<type>` 测试,运行测试时才写文件。因此必须运行
# "定义了这些类型"的 crate 的测试:
#
#   types-core   强类型 ID(id_type! 生成)
#   common-core  统一响应格式 R(与 AppError 响应实现同 crate)
#   types-visual-token  视觉令牌契约(VisualToken / FaceBBox / ImageDimensions)
#   types-audit  审计查询 DTO
#   types-identity 认证与用户资料 DTO
#   types-visual 视觉实体与 DTO
#
# 这些 crate 合起来覆盖全部 `#[ts(export)]` 类型(ID / 响应包装 / 令牌 /
# 审计 / 身份 / 视觉 DTO)。
#
# 输出目录由 .cargo/config.toml 的 TS_RS_EXPORT_DIR 指定(= target/bindings),
# 具体子目录由各类型的 export_to 属性决定(auth / user / visual / audit / common)。
#
# 用法: ./build/gen-ts.sh
#
# profile: 与仓库约定一致, 一律 --release(见 AGENTS.md 的"构建 profile")。
set -e

cd "$(dirname "$0")/.."

echo '=== 生成 TS 类型契约 ==='
cargo test --release \
  -p types-core \
  -p common-core \
  -p types-visual-token \
  -p types-audit \
  -p types-identity \
  -p types-visual \
  --features ts,orm,axum

echo
echo '=== 产物统计 ==='
find target/bindings -name '*.ts' | wc -l

echo '=== 目录分布 ==='
find target/bindings -name '*.ts' | sed 's|target/bindings/||; s|/[^/]*$||' | sort | uniq -c
