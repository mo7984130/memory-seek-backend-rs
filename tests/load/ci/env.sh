# tests/load/ci/env.sh
# 压测体系全局环境变量与路径约定
# 用法: source "$(dirname "$0")/env.sh" 之后调用其它 ci/*.sh

# ── 目标服务 ──────────────────────────────────────────
: "${REMOTE_HOST:=127.0.0.1}"
: "${SERVER_PORT:=7985}"
# metrics 由主服务暴露: http://${REMOTE_HOST}:${SERVER_PORT}/metrics
# 被测 server 占用核数(控制变量; taskset 绑定, 压测对比不随宿主机核数漂移)
: "${SERVER_CPUS:=2}"

# ── 构建 ──────────────────────────────────────────────
# 含 face-engine(仓库内置 ONNX 模型, 见 models/), 覆盖 face/person 压测
# 注意: 不含 backup(其 feature 与 photo 冲突, 且定时备份任务会干扰压测)
: "${FEATURES:=metrics,auth,user,photo,face-engine}"
: "${BIN_NAME:=memory-seek-server}"

# ── 压测数据与轮次 ────────────────────────────────────
: "${AUTH_USERS:=10000}"
: "${PHOTO_USERS:=2000}"
# 每场景轮次, 归一化时取中位数抗共享环境波动
: "${RUNS:=3}"

# S3 路径开关: CI 暂不模拟 S3, 默认关闭; 本地连真实 OSS 时可置 true
: "${INCLUDE_S3_PATHS:=false}"

# photo 场景中不依赖 S3 的子场景(独立脚本文件名, 对应 scenarios/photo/<name>.js)
# 注: k6 0.52+ 移除 --scenario flag, 无法从 photo.js 按 scenario 挑选, 直接运行独立脚本
PHOTO_NOS3_SERVICES="${PHOTO_NOS3_SERVICES:-collection collection_photo comment comment_like}"

# ── 目录约定 ──────────────────────────────────────────
CI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOAD_DIR="$(dirname "$CI_DIR")"        # .../tests/load
ROOT_DIR="$(dirname "$(dirname "$LOAD_DIR")")"  # .../tests/load -> 仓库根

RESULTS_DIR="${LOAD_DIR}/results"
RUNS_DIR="${RESULTS_DIR}/run"                    # run-<n> 单轮结果
NORMALIZED_DIR="${RESULTS_DIR}/normalized"       # collect.sh 归一化输出
MAX_RUNS_DIR="${RESULTS_DIR}/max-run"            # max 极限压测单轮结果(max-run-<n>)
MAX_NORMALIZED_DIR="${RESULTS_DIR}/max-normalized"  # max 归一化输出
METRICS_DIR="${RESULTS_DIR}/metrics"             # prometheus.sh 快照
BASELINES_DIR="${LOAD_DIR}/baselines"            # 基线(normalized json 同构)

SERVER_CONFIG="${CI_DIR}/server-config.yml"
SERVER_LOG="${CI_DIR}/server.log"
SERVER_PID="${CI_DIR}/server.pid"

export REMOTE_HOST SERVER_PORT SERVER_CPUS FEATURES BIN_NAME
export AUTH_USERS PHOTO_USERS RUNS INCLUDE_S3_PATHS PHOTO_NOS3_SERVICES
export MAX_RUNS_DIR MAX_NORMALIZED_DIR
