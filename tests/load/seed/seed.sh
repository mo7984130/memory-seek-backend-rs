#!/usr/bin/env bash
# tests/load/seed/seed.sh
# 灌入压测种子数据: 建表(init.sql) + 压测账号/照片/时间线统计(seed.sql)。
# 幂等: 可重复执行。通过 compose 容器内的 psql 执行(无需宿主机安装 psql)。
#
# 用法: make seed 或直接执行(env.sh 提供默认值)
#   可选覆盖: AUTH_USERS / PHOTO_USERS / PHOTOS_PER_USER

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../ci/env.sh"

: "${AUTH_USERS:=10000}"
: "${PHOTO_USERS:=200}"
: "${PHOTOS_PER_USER:=20}"
# 每个 photo 用户中分配给人物的照片人脸数(也是 photo_person.face_count)
: "${FACES_PER_PERSON:=5}"

# Test123456 的 argon2id 哈希(由 common::utils::HashAlgorithm 生成, 参数 m=16384,t=2,p=1)
PASS_HASH='$argon2id$v=19$m=16384,t=2,p=1$T5U+IfQVViaUNr7dhPHmww$CCUS5IsGLNeg0//M+1Iyuwe1izIKPB0oyRud71qofLY'

PHOTO_COUNT=$((PHOTO_USERS * PHOTOS_PER_USER))

# 通过 compose 容器执行 psql(依赖 docker-compose.load.yml 中 postgres 服务已启动)
# 注意: psql 经 stdin 读取脚本时 -v 变量替换不生效, 故用 sed 做文本替换
PSQL=(
    docker compose -f "$LOAD_DIR/docker-compose.load.yml" exec -T postgres
    psql -U memory_seek -d memory_seek -v ON_ERROR_STOP=1 -q
)

SEED_SQL="$(dirname "${BASH_SOURCE[0]}")/seed.sql"
SEED_TMP="$(mktemp)"
trap 'rm -f "$SEED_TMP"' EXIT

# 变量替换(哈希含 $ / 等字符, 用 | 作定界符; 哈希不含 & 与反斜杠)
sed \
    -e "s|:PASS_HASH|${PASS_HASH}|g" \
    -e "s|:AUTH_USERS|${AUTH_USERS}|g" \
    -e "s|:PHOTO_USERS|${PHOTO_USERS}|g" \
    -e "s|:PHOTOS_PER_USER|${PHOTOS_PER_USER}|g" \
    -e "s|:FACES_PER_PERSON|${FACES_PER_PERSON}|g" \
    -e "s|:PHOTO_COUNT|${PHOTO_COUNT}|g" \
    "$SEED_SQL" > "$SEED_TMP"

echo "[seed] 建表 (init.sql)..."
"${PSQL[@]}" < "$ROOT_DIR/docs/sql/init.sql"

# 校验关键表已建立, 防止建表失败后继续压测导致大面积报错
if ! "${PSQL[@]}" -tAc "SELECT to_regclass('public.auth_user');" | grep -q 'auth_user'; then
    echo "[seed] ERROR: auth_user 表未创建, init.sql 执行失败" >&2
    exit 1
fi

echo "[seed] schema 对齐 (schema_align.sql)..."
"${PSQL[@]}" < "$(dirname "${BASH_SOURCE[0]}")/schema_align.sql"

echo "[seed] 灌入数据 auth_users=$AUTH_USERS photo_users=$PHOTO_USERS photos/user=$PHOTOS_PER_USER ..."
"${PSQL[@]}" < "$SEED_TMP"

# 汇总校验
"${PSQL[@]}" -c "SELECT
    (SELECT count(*) FROM auth_user  WHERE email LIKE 'loadtest_%') AS loadtest_users,
    (SELECT count(*) FROM photo_photo)                                AS photos,
    (SELECT count(*) FROM photo_timeline_stat)                        AS timeline_months,
    (SELECT count(*) FROM photo_face)                                 AS faces,
    (SELECT count(*) FROM photo_person)                               AS persons;"
