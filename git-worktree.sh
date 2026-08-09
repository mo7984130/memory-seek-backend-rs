#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"
WT_ROOT="${MSK_WT_ROOT:-$(dirname "$SCRIPT_DIR")/worktrees}"
CARGO_TARGET_DIR="${MSK_TARGET_DIR:-$SCRIPT_DIR/target}"
DEFAULT_BASE="${MSK_BASE_BRANCH:-develop}"
OPENCODE_CMD="${MSK_OPENCODE:-opencode}"

usage() {
  cat <<EOF
用法: $(basename "$0") <命令> [参数]

命令:
  create <agent名> [基础分支]  创建独立 worktree 与分支 agent/<名>, 并启动 opencode
  open   <agent名>             打开已有 worktree 并启动 opencode
  list                         列出所有 worktree
  remove <agent名>             删除 worktree 与分支
  reset  <agent名>             重置 worktree 到基础分支最新状态
  setenv                       打印 CARGO_TARGET_DIR 供手动 export

环境变量(可选):
  MSK_WT_ROOT        worktree 根目录    (默认: 仓库父目录/worktrees)
  MSK_TARGET_DIR     共享 target 目录   (默认: 仓库根/target)
  MSK_BASE_BRANCH    默认基础分支       (默认: develop)
  MSK_OPENCODE       opencode 命令      (默认: opencode)
EOF
}

log_info() { printf '\033[32m%s\033[0m\n' "$*"; }
log_warn() { printf '\033[33m%s\033[0m\n' "$*" >&2; }
log_err() { printf '\033[31m%s\033[0m\n' "$*" >&2; }

require_agent() {
  local name="$1"
  if [[ ! "$name" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
    log_err "非法的 agent 名: $name"
    exit 1
  fi
  echo "$name"
}

branch_name() { echo "agent/$1"; }
worktree_dir() { echo "$WT_ROOT/$1"; }

create() {
  local name base branch wdir
  name="$(require_agent "${1:?缺少 agent 名}")"
  base="${2:-$DEFAULT_BASE}"
  branch="$(branch_name "$name")"
  wdir="$(worktree_dir "$name")"

  if [[ -e "$wdir" ]]; then
    log_err "worktree 目录已存在: $wdir"
    exit 1
  fi
  if git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$branch"; then
    log_err "分支已存在: $branch"
    exit 1
  fi
  if ! git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$base"; then
    log_err "基础分支不存在: $base"
    exit 1
  fi
  if ! git -C "$REPO_ROOT" diff --quiet HEAD; then
    log_warn "注意: 主仓库存在未提交改动, 不会带入新 worktree"
  fi

  mkdir -p "$WT_ROOT"
  git -C "$REPO_ROOT" worktree add -b "$branch" "$wdir" "$base"
  cat > "$wdir/.wt-env.sh" <<EOF
export CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
EOF

  log_info "已创建 worktree: $wdir"
  log_info "  分支: $branch"
  log_info "  基础: $base"
  log_info "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
  cd "$wdir"
  exec $OPENCODE_CMD
}

open() {
  local name wdir
  name="$(require_agent "${1:?缺少 agent 名}")"
  wdir="$(worktree_dir "$name")"

  if [[ ! -d "$wdir" ]]; then
    log_err "worktree 不存在: $wdir"
    exit 1
  fi
  if [[ -f "$wdir/.wt-env.sh" ]]; then
    # shellcheck disable=SC1091
    source "$wdir/.wt-env.sh"
  fi
  cd "$wdir"
  exec $OPENCODE_CMD
}

list() {
  git -C "$REPO_ROOT" worktree list | while IFS= read -r line; do
    local dir
    dir="$(printf '%s' "$line" | awk '{print $1}')"
    if [[ "$dir" == "$WT_ROOT/"* ]]; then
      printf '%s\n' "$line"
    fi
  done
  printf '\n共享 target: %s\n' "$CARGO_TARGET_DIR"
}

remove() {
  local name branch wdir
  name="$(require_agent "${1:?缺少 agent 名}")"
  branch="$(branch_name "$name")"
  wdir="$(worktree_dir "$name")"

  if [[ ! -d "$wdir" ]]; then
    log_err "worktree 不存在: $wdir"
    exit 1
  fi
  local dirty
  dirty="$(git -C "$wdir" status --porcelain | grep -v '^?? .wt-env.sh$' || true)"
  if [[ -n "$dirty" ]]; then
    log_warn "注意: worktree 存在未提交改动"
  fi
  git -C "$REPO_ROOT" worktree remove --force "$wdir"
  if git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$REPO_ROOT" branch -D "$branch"
  fi
  log_info "已删除: $wdir"
}

reset() {
  local name wdir
  name="$(require_agent "${1:?缺少 agent 名}")"
  wdir="$(worktree_dir "$name")"

  if [[ ! -d "$wdir" ]]; then
    log_err "worktree 不存在: $wdir"
    exit 1
  fi
  log_warn "将强制重置 $name 到 $DEFAULT_BASE 并清理未跟踪文件"
  read -r -p "确认? [y/N] " ans
  if [[ ! "$ans" =~ ^[Yy]$ ]]; then
    log_warn "已取消"
    exit 0
  fi
  local ref="refs/remotes/origin/$DEFAULT_BASE"
  if git -C "$REPO_ROOT" show-ref --verify --quiet "$ref"; then
    git -C "$REPO_ROOT" fetch origin --quiet
    git -C "$wdir" reset --hard "$ref"
  else
    git -C "$wdir" reset --hard "$DEFAULT_BASE"
  fi
  git -C "$wdir" clean -ffdx
  cat > "$wdir/.wt-env.sh" <<EOF
export CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
EOF
  log_info "已重置: $wdir"
}

setenv() {
  printf 'export CARGO_TARGET_DIR=%q\n' "$CARGO_TARGET_DIR"
}

cmd="${1:-}"
case "$cmd" in
  create|open|list|remove|reset|setenv|help|-h|--help)
    ;;
  *)
    usage
    exit 1
    ;;
esac

case "$cmd" in
  create) create "${@:2}" ;;
  open)   open "${@:2}" ;;
  list)   list ;;
  remove) remove "${@:2}" ;;
  reset)  reset "${@:2}" ;;
  setenv) setenv ;;
  help|-h|--help) usage ;;
  *) usage ;;
esac
