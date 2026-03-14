#!/bin/bash
# subrepo-commands.sh
# subrepo 常用操作快捷命令
#
# 用法:
#   ./scripts/subrepo-commands.sh <command> [args...]
#
# 命令:
#   status          显示所有 subrepo 状态
#   pull <name>     拉取指定 subrepo 的更新
#   pull-all        拉取所有 subrepo 的更新
#   push <name>     推送指定 subrepo 的本地修改
#   push-all        推送所有 subrepo 的本地修改
#   fetch <name>    获取指定 subrepo 的更新（不合并）
#   branch          显示 subrepo 分支信息
#   clean           清理 subrepo 临时分支
#   list            列出所有 subrepo

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

COMMAND="${1:-status}"
shift || true

# 所有 subrepo 列表
SUBREPOS=(
  "tg-rcore-tutorial-sbi"
  "tg-rcore-tutorial-linker"
  "tg-rcore-tutorial-console"
  "tg-rcore-tutorial-kernel-context"
  "tg-rcore-tutorial-kernel-alloc"
  "tg-rcore-tutorial-kernel-vm"
  "tg-rcore-tutorial-easy-fs"
  "tg-rcore-tutorial-signal-defs"
  "tg-rcore-tutorial-task-manage"
  "tg-rcore-tutorial-checker"
  "tg-rcore-tutorial-syscall"
  "tg-rcore-tutorial-signal"
  "tg-rcore-tutorial-sync"
  "tg-rcore-tutorial-signal-impl"
  "tg-rcore-tutorial-user"
  "tg-rcore-tutorial-ch1"
  "tg-rcore-tutorial-ch2"
  "tg-rcore-tutorial-ch3"
  "tg-rcore-tutorial-ch4"
  "tg-rcore-tutorial-ch5"
  "tg-rcore-tutorial-ch6"
  "tg-rcore-tutorial-ch7"
  "tg-rcore-tutorial-ch8"
)

case "$COMMAND" in
  status)
    echo "=== Subrepo Status ==="
    git subrepo status
    ;;
    
  list)
    echo "=== Subrepo List (${#SUBREPOS[@]} total) ==="
    for sub in "${SUBREPOS[@]}"; do
      if [ -d "$sub" ]; then
        echo "  ✓ $sub"
      else
        echo "  ✗ $sub (missing)"
      fi
    done
    ;;
    
  pull)
    NAME="$1"
    if [ -z "$NAME" ]; then
      echo "Usage: $0 pull <subrepo-name>"
      echo "Available: ${SUBREPOS[*]}"
      exit 1
    fi
    echo "=== Pulling $NAME ==="
    git subrepo pull "$NAME"
    ;;
    
  pull-all)
    echo "=== Pulling All Subrepos ==="
    git subrepo pull --all
    ;;
    
  push)
    NAME="$1"
    if [ -z "$NAME" ]; then
      echo "Usage: $0 push <subrepo-name>"
      echo "Available: ${SUBREPOS[*]}"
      exit 1
    fi
    echo "=== Pushing $NAME ==="
    echo "Note: You may be prompted for commit message"
    git subrepo push "$NAME"
    ;;
    
  push-all)
    echo "=== Pushing All Subrepos ==="
    git subrepo push --all
    ;;
    
  fetch)
    NAME="$1"
    if [ -z "$NAME" ]; then
      echo "Usage: $0 fetch <subrepo-name>"
      exit 1
    fi
    echo "=== Fetching $NAME ==="
    git subrepo fetch "$NAME"
    ;;
    
  branch)
    echo "=== Subrepo Branch Info ==="
    git subrepo branch
    ;;
    
  clean)
    echo "=== Cleaning Subrepo Temporary Branches ==="
    git subrepo clean
    ;;
    
  help|*)
    echo "Subrepo Commands"
    echo ""
    echo "Usage: $0 <command> [args...]"
    echo ""
    echo "Commands:"
    echo "  status          显示所有 subrepo 状态"
    echo "  list            列出所有 subrepo"
    echo "  pull <name>     拉取指定 subrepo 的更新"
    echo "  pull-all        拉取所有 subrepo 的更新"
    echo "  push <name>     推送指定 subrepo 的本地修改"
    echo "  push-all        推送所有 subrepo 的本地修改"
    echo "  fetch <name>    获取指定 subrepo 的更新（不合并）"
    echo "  branch          显示 subrepo 分支信息"
    echo "  clean           清理 subrepo 临时分支"
    echo ""
    echo "Available subrepos:"
    for sub in "${SUBREPOS[@]}"; do
      echo "  - $sub"
    done
    ;;
esac
