#!/usr/bin/env bash
set -euo pipefail

# git-pr.sh — 基于提交自动生成并创建 PR(标题 + Description)
#
# 用法:
#   git-pr.sh [head] [base] [选项]
#
# 参数:
#   head   发起分支(默认: 当前分支)
#   base   目标分支(默认: main)
# 选项:
#   --yes       跳过预览确认直接发起
#   --draft     创建草稿 PR
#   -t <标题>   手动指定标题(覆盖自动推导)
#   --open      创建后浏览器打开
#   -h/--help   帮助
#
# 依赖: gh(github cli, 已认证)、git。提交需遵循 Conventional Commits 风格。

usage() {
  cat <<EOF
用法: $(basename "$0") [head] [base] [选项]

参数:
  head   发起分支(默认: 当前分支)
  base   目标分支(默认: main)

选项:
  --yes       跳过预览确认直接发起
  --draft     创建草稿 PR
  -t <标题>   手动指定标题(覆盖自动推导)
  --open      创建后浏览器打开
  -h/--help   帮助

示例:
  $(basename "$0")                                  # 当前分支 -> main
  $(basename "$0") develop main                     # develop -> main
  $(basename "$0") develop main --draft --yes       # 草稿, 不确认
EOF
}

log_info() { printf '\033[32m%s\033[0m\n' "$*"; }
log_warn() { printf '\033[33m%s\033[0m\n' "$*" >&2; }
log_err() { printf '\033[31m%s\033[0m\n' "$*" >&2; }

HEAD=""
BASE="main"
YES=0
DRAFT=""
MANUAL_TITLE=""
OPEN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes) YES=1; shift ;;
    --draft) DRAFT="--draft"; shift ;;
    --open) OPEN=1; shift ;;
    -t|--title) MANUAL_TITLE="${2:?缺少标题参数}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    -*)
      if [[ -z "$HEAD" ]]; then HEAD="$1"; else log_err "未知参数: $1"; usage; exit 1; fi
      shift ;;
    *)
      if [[ -z "$HEAD" ]]; then HEAD="$1"; elif [[ -z "$BASE" || "$BASE" == "main" ]]; then BASE="$1"; else log_err "多余参数: $1"; usage; exit 1; fi
      shift ;;
  esac
done

# head 默认当前分支
if [[ -z "$HEAD" ]]; then
  HEAD="$(git branch --show-current)"
fi
if [[ -z "$HEAD" ]]; then
  log_err "无法确定当前分支(HEAD detached?), 请显式指定 head 分支"
  exit 1
fi

# head 已推送校验
if ! git ls-remote --exit-code --heads origin "$HEAD" >/dev/null 2>&1; then
  log_err "head 分支未推送: $HEAD"
  log_err "请先执行: git push origin $HEAD"
  exit 1
fi

# base 分支存在性(本地或远端)
if ! git show-ref --verify --quiet "refs/heads/$BASE" && \
   ! git ls-remote --exit-code --heads origin "$BASE" >/dev/null 2>&1; then
  log_err "base 分支不存在(本地或远端): $BASE"
  exit 1
fi

if ! command -v gh >/dev/null 2>&1; then
  log_err "缺少 gh CLI, 请先安装并执行 gh auth login"
  exit 1
fi

# ── 收集提交(跳过 merge) ─────────────────────────────
RANGE="origin/$BASE..origin/$HEAD"
COMMITS="$(git log --format='%s' "$RANGE" 2>/dev/null || \
           git log --format='%s' "origin/$BASE..$HEAD" 2>/dev/null)"
COMMITS="$(printf '%s\n' "$COMMITS" | grep -vE '^(Merge branch|Merge pull request|Merge remote-tracking|Merge tag)' || true)"

if [[ -z "$COMMITS" ]]; then
  log_warn "head($HEAD) 相对 base($BASE) 无变更, 不创建 PR"
  exit 0
fi

COUNT="$(printf '%s\n' "$COMMITS" | grep -c .)"
log_info "head: $HEAD  base: $BASE  提交数: $COUNT"

# ── 标题推导 ──────────────────────────────────────────
CONV_RE='^(feat|fix|refactor|perf|docs|test|chore|style|build|ci|revert|chore)(\([^)]*\))?: (.+)$'

if [[ -n "$MANUAL_TITLE" ]]; then
  TITLE="$MANUAL_TITLE"
else
  # 各类型计数
  declare -A TYPE_CNT
  FIRST_MSG=""
  FIRST_TYPE=""
  FIRST_SCOPE=""
  while IFS= read -r msg; do
    [[ -z "$msg" ]] && continue
    if [[ -z "$FIRST_MSG" ]]; then
      FIRST_MSG="$msg"
      if [[ "$msg" =~ $CONV_RE ]]; then
        FIRST_TYPE="${BASH_REMATCH[1]}"
        FIRST_SCOPE="${BASH_REMATCH[2]:1:${#BASH_REMATCH[2]}-2}"
      fi
    fi
    if [[ "$msg" =~ $CONV_RE ]]; then
      t="${BASH_REMATCH[1]}"
      TYPE_CNT["$t"]=$(( ${TYPE_CNT["$t"]:-0} + 1 ))
    fi
  done <<< "$COMMITS"

  # 主类型: feat 优先(若存在), 否则取计数最多
  MAIN_TYPE="$FIRST_TYPE"
  if [[ -n "${TYPE_CNT[feat]:-}" ]]; then
    MAIN_TYPE="feat"
  else
    max=0
    for t in "${!TYPE_CNT[@]}"; do
      if [[ "${TYPE_CNT[$t]}" -gt "$max" ]]; then
        max="${TYPE_CNT[$t]}"; MAIN_TYPE="$t"
      fi
    done
  fi

  SCOPE_PART=""
  if [[ -n "${FIRST_SCOPE:-}" ]]; then
    SCOPE_PART="($FIRST_SCOPE)"
  fi
  SUBJECT="$(printf '%s' "$FIRST_MSG" | sed -E "s/^$CONV_RE$/\3/")"
  SUBJECT="${SUBJECT%%[.。]}"
  SUBJECT="${SUBJECT:0:60}"

  TITLE="$MAIN_TYPE$SCOPE_PART: $SUBJECT"
fi

# ── 正文生成 ──────────────────────────────────────────
group_commits() {
  local label="$1"
  local re="^$label(\\([^)]*\\))?: "
  local found=0
  while IFS= read -r msg; do
    [[ -z "$msg" ]] && continue
    if [[ "$msg" =~ $re ]]; then
      found=1
      printf '%s\n' "- $msg"
    fi
  done <<< "$COMMITS"
  if [[ "$found" -eq 1 ]]; then printf '\n'; fi
  return 0
}

BODY="## 概述

本 PR 基于 $HEAD → $BASE 的 $COUNT 个提交自动生成。

## 变更

### 新功能 (feat)
$(group_commits "feat")
### 修复 (fix)
$(group_commits "fix")
### 性能优化 (perf)
$(group_commits "perf")
### 重构 (refactor)
$(group_commits "refactor")
### 文档 (docs)
$(group_commits "docs")
### 测试 (test)
$(group_commits "test")
### 其他 (chore/style/build/ci/revert)
$(group_commits "chore|style|build|ci|revert")
## 验证

- 依赖 CI / 压测工作流结果确认后合并。
"

# ── 预览 ──────────────────────────────────────────────
printf '\n=== PR 预览 ===\n'
printf '标题: %s\n' "$TITLE"
printf '分支: %s -> %s\n' "$HEAD" "$BASE"
printf '%s\n' "----------------------------------------"
printf '%s\n' "$BODY"
printf '%s\n' "----------------------------------------"

if [[ "$YES" -ne 1 ]]; then
  ans=""
  read -r -p "确认发起 PR? [y/N] " ans || true
  if [[ ! "$ans" =~ ^[Yy]$ ]]; then
    log_warn "已取消"
    exit 0
  fi
fi

# ── 发起 ──────────────────────────────────────────────
URL="$(gh pr create --repo "$(git remote get-url origin | sed -E 's#^.*github\.com[:/]##; s#\.git$##')" \
  --base "$BASE" --head "$HEAD" \
  --title "$TITLE" --body "$BODY" $DRAFT)"

log_info "PR 已创建: $URL"
if [[ "$OPEN" -eq 1 ]]; then
  gh pr view --repo "$(git remote get-url origin | sed -E 's#^.*github\.com[:/]##; s#\.git$##')" --web "$URL" >/dev/null 2>&1 &
fi
