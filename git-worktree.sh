#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"
WT_ROOT="${MSK_WT_ROOT:-$(dirname "$SCRIPT_DIR")/worktrees}"
CARGO_TARGET_DIR="${MSK_TARGET_DIR:-$SCRIPT_DIR/target}"
DEFAULT_BASE="${MSK_BASE_BRANCH:-develop}"
BRANCH_PREFIX="${MSK_BRANCH_PREFIX:-feature}"
OPENCODE_CMD="${MSK_OPENCODE:-opencode}"

usage() {
  cat <<EOF
用法: $(basename "$0") <命令> [参数]

命令:
  create <agent名> [基础分支]  创建独立 worktree 与分支 <前缀>/<名>, 并启动 opencode
  open   <agent名>             打开已有 worktree 并启动 opencode
  list                         列出所有 worktree
  merge  <agent名> [目标分支] 用 --no-ff 合并到目标分支(默认 $DEFAULT_BASE), 推送远端后删除分支
                            (加 --keep 保留分支与 worktree)
  rename <agent名> [新agent名] 重命名 worktree 分支(默认只换前缀)
  remove <agent名>             删除 worktree 与分支
  reset  <agent名>             重置 worktree 到基础分支最新状态
  setenv                       打印 CARGO_TARGET_DIR 供手动 export

环境变量(可选):
  MSK_WT_ROOT        worktree 根目录    (默认: 仓库父目录/worktrees)
  MSK_TARGET_DIR     共享 target 目录   (默认: 仓库根/target)
  MSK_BASE_BRANCH    默认基础分支       (默认: develop)
  MSK_BRANCH_PREFIX  分支前缀           (默认: feature)
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

branch_name() { echo "$BRANCH_PREFIX/$1"; }
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

find_target_worktree() {
  # 返回某个分支当前所在的 worktree 目录, 找不到返回空
  local target="$1"
  git -C "$REPO_ROOT" worktree list --porcelain | awk -v branch="branch refs/heads/$target" '
    /^worktree / { dir=substr($0, index($0, " ")+1) }
    $0 == branch { print dir; exit }
  '
}

merge() {
  local name target branch tdir dirty keep
  name="$(require_agent "${1:?缺少 agent 名}")"
  target="${2:-$DEFAULT_BASE}"
  branch="$(branch_name "$name")"
  keep=0
  for arg in "${@:3}"; do
    case "$arg" in
      --keep) keep=1 ;;
      *) log_err "未知参数: $arg"; exit 1 ;;
    esac
  done

  if [[ "$branch" == "$target" ]]; then
    log_err "不能把分支合并到自身: $branch"
    exit 1
  fi
  if ! git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$branch"; then
    log_err "分支不存在: $branch"
    exit 1
  fi
  if ! git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$target"; then
    log_err "目标分支不存在: $target"
    exit 1
  fi

  tdir="$(find_target_worktree "$target")"
  if [[ -z "$tdir" ]]; then
    log_err "目标分支 $target 未被任何 worktree 检出"
    log_err "请先在目标分支所在 worktree 中执行: git checkout $target"
    exit 1
  fi
  dirty="$(git -C "$tdir" status --porcelain | grep -v '^?? \.wt-env\.sh$' || true)"
  if [[ -n "$dirty" ]]; then
    log_err "目标 worktree($tdir) 存在未提交改动, 请先提交或还原"
    git -C "$tdir" status --short
    exit 1
  fi

  log_info "目标 worktree: $tdir"
  log_info "执行: git merge --no-ff $branch"
  if ! git -C "$tdir" merge --no-ff "$branch" -m "Merge branch '$branch' into $target"; then
    log_err ""
    log_err "合并产生冲突, 请按以下步骤解决:"
    log_err "  1. cd $tdir"
    log_err "  2. git status 查看冲突文件"
    log_err "  3. 手动编辑冲突文件, 保留想要的内容"
    log_err "  4. git add <已解决的文件>"
    log_err "  5. git merge --continue (会复用上面的提交信息)"
    log_err "  或放弃合并: git merge --abort"
    log_err "  合并失败, 未推送、未删除分支"
    exit 1
  fi
  log_info "合并完成: $target <- $branch"

  if git -C "$tdir" push origin "$target"; then
    log_info "已推送: origin/$target"
  else
    log_err "推送失败, 请手动执行: git -C $tdir push origin $target"
    exit 1
  fi

  if [[ "$keep" -eq 1 ]]; then
    log_warn "已保留分支与 worktree: $branch"
    return 0
  fi

  if [[ -d "$(worktree_dir "$name")" ]]; then
    git -C "$REPO_ROOT" worktree remove --force "$(worktree_dir "$name")"
  fi
  if git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$REPO_ROOT" branch -D "$branch"
  fi
  log_info "已删除分支与 worktree: $branch"
}

rename() {
  local name newname branch newbranch wdir
  name="$(require_agent "${1:?缺少 agent 名}")"
  newname="${2:-$name}"
  newname="$(require_agent "$newname")"
  branch="$(branch_name "$name")"
  newbranch="$(branch_name "$newname")"

  if [[ "$branch" == "$newbranch" ]]; then
    log_warn "分支名相同: $branch"
    exit 0
  fi
  if ! git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$branch"; then
    log_err "分支不存在: $branch"
    exit 1
  fi
  if git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$newbranch"; then
    log_err "目标分支已存在: $newbranch"
    exit 1
  fi
  wdir="$(worktree_dir "$name")"
  if [[ ! -d "$wdir" ]]; then
    log_err "worktree 不存在: $wdir"
    exit 1
  fi
  git -C "$REPO_ROOT" branch -m "$branch" "$newbranch"
  log_info "已重命名: $branch -> $newbranch"
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
  create|open|list|merge|rename|remove|reset|setenv|help|-h|--help)
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
  merge)  merge "${@:2}" ;;
  rename) rename "${@:2}" ;;
  remove) remove "${@:2}" ;;
  reset)  reset "${@:2}" ;;
  setenv) setenv ;;
  help|-h|--help) usage ;;
  *) usage ;;
esac
