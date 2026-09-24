#!/bin/bash
# 生成前端 TS 类型契约(target/bindings)
#
# ts-rs 是**测试期**导出:每个 `#[ts(export)]` 类型会生成一个
# `export_bindings_<type>` 测试,运行测试时才写文件。因此必须运行
# "定义了这些类型"的 crate 的测试:
#
#   types-core  强类型 ID(id_type! 生成)
#   types       DTO / 视图 / 共享枚举
#   common      分页与响应包装(CursorPage / R)
#
# 输出目录由 .cargo/config.toml 的 TS_RS_EXPORT_DIR 指定(= target/bindings),
# 具体子目录由各类型的 export_to 属性决定(auth / user / visual / audit / common)。
#
# 用法: ./build/gen-ts.sh
set -e

cd "$(dirname "$0")/.."

echo '=== 生成 TS 类型契约 ==='
cargo test \
  -p types-core \
  -p types \
  -p common \
  --features ts,orm,axum

echo
echo '=== 产物统计 ==='
find target/bindings -name '*.ts' | wc -l

echo '=== 目录分布 ==='
find target/bindings -name '*.ts' | sed 's|target/bindings/||; s|/[^/]*$||' | sort | uniq -c
