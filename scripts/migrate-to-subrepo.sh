#!/bin/bash
# migrate-to-subrepo.sh
# 将 submodule 迁移到 subrepo
#
# 用法:
#   ./scripts/migrate-to-subrepo.sh [--dry-run] [--backup]
#
# 选项:
#   --dry-run  只显示将要执行的操作，不实际执行
#   --backup   执行前备份整个仓库
#
# 前置条件:
#   1. 已安装 git-subrepo (运行 scripts/install-git-subrepo.sh)
#   2. 当前分支为 test，且工作区干净
#   3. 有推送权限到各个子仓库

set -e

# 配置
DRY_RUN=false
DO_BACKUP=false
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 解析参数
for arg in "$@"; do
  case $arg in
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --backup)
      DO_BACKUP=true
      shift
      ;;
    *)
      echo "Unknown argument: $arg"
      exit 1
      ;;
  esac
done

echo "=== Submodule to Subrepo Migration ==="
echo "Repository: $REPO_ROOT"
echo "Dry run: $DRY_RUN"
echo ""

# 所有 submodule 定义
declare -A SUBMODULES
SUBMODULES["tg-rcore-tutorial-ch1"]="git@github.com:rcore-os/tg-rcore-tutorial-ch1.git:test"
SUBMODULES["tg-rcore-tutorial-ch2"]="git@github.com:rcore-os/tg-rcore-tutorial-ch2.git:test"
SUBMODULES["tg-rcore-tutorial-ch3"]="git@github.com:rcore-os/tg-rcore-tutorial-ch3.git:test"
SUBMODULES["tg-rcore-tutorial-ch4"]="git@github.com:rcore-os/tg-rcore-tutorial-ch4.git:test"
SUBMODULES["tg-rcore-tutorial-ch5"]="git@github.com:rcore-os/tg-rcore-tutorial-ch5.git:test"
SUBMODULES["tg-rcore-tutorial-ch6"]="git@github.com:rcore-os/tg-rcore-tutorial-ch6.git:test"
SUBMODULES["tg-rcore-tutorial-ch7"]="git@github.com:rcore-os/tg-rcore-tutorial-ch7.git:test"
SUBMODULES["tg-rcore-tutorial-ch8"]="git@github.com:rcore-os/tg-rcore-tutorial-ch8.git:test"
SUBMODULES["tg-rcore-tutorial-user"]="git@github.com:rcore-os/tg-rcore-tutorial-user.git:test"
SUBMODULES["tg-rcore-tutorial-checker"]="git@github.com:rcore-os/tg-rcore-tutorial-checker.git:test"
SUBMODULES["tg-rcore-tutorial-console"]="git@github.com:rcore-os/tg-rcore-tutorial-console.git:test"
SUBMODULES["tg-rcore-tutorial-easy-fs"]="git@github.com:rcore-os/tg-rcore-tutorial-easy-fs.git:test"
SUBMODULES["tg-rcore-tutorial-kernel-alloc"]="git@github.com:rcore-os/tg-rcore-tutorial-kernel-alloc.git:test"
SUBMODULES["tg-rcore-tutorial-kernel-context"]="git@github.com:rcore-os/tg-rcore-tutorial-kernel-context.git:test"
SUBMODULES["tg-rcore-tutorial-kernel-vm"]="git@github.com:rcore-os/tg-rcore-tutorial-kernel-vm.git:test"
SUBMODULES["tg-rcore-tutorial-linker"]="git@github.com:rcore-os/tg-rcore-tutorial-linker.git:test"
SUBMODULES["tg-rcore-tutorial-sbi"]="git@github.com:rcore-os/tg-rcore-tutorial-sbi.git:test"
SUBMODULES["tg-rcore-tutorial-signal"]="git@github.com:rcore-os/tg-rcore-tutorial-signal.git:test"
SUBMODULES["tg-rcore-tutorial-signal-defs"]="git@github.com:rcore-os/tg-rcore-tutorial-signal-defs.git:test"
SUBMODULES["tg-rcore-tutorial-signal-impl"]="git@github.com:rcore-os/tg-rcore-tutorial-signal-impl.git:test"
SUBMODULES["tg-rcore-tutorial-sync"]="git@github.com:rcore-os/tg-rcore-tutorial-sync.git:test"
SUBMODULES["tg-rcore-tutorial-syscall"]="git@github.com:rcore-os/tg-rcore-tutorial-syscall.git:test"
SUBMODULES["tg-rcore-tutorial-task-manage"]="git@github.com:rcore-os/tg-rcore-tutorial-task-manage.git:test"

# 按依赖层级排序迁移顺序（底层组件优先）
MIGRATION_ORDER=(
  # Layer 0: 基础组件 (无内部依赖)
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
  # Layer 1: 中间组件
  "tg-rcore-tutorial-syscall"
  "tg-rcore-tutorial-signal"
  "tg-rcore-tutorial-sync"
  # Layer 2: 上层组件
  "tg-rcore-tutorial-signal-impl"
  "tg-rcore-tutorial-user"
  # Layer 3: 内核章节
  "tg-rcore-tutorial-ch1"
  "tg-rcore-tutorial-ch2"
  "tg-rcore-tutorial-ch3"
  "tg-rcore-tutorial-ch4"
  "tg-rcore-tutorial-ch5"
  "tg-rcore-tutorial-ch6"
  "tg-rcore-tutorial-ch7"
  "tg-rcore-tutorial-ch8"
)

cd "$REPO_ROOT"

# 前置检查
echo "=== Pre-flight Checks ==="

# 检查 git-subrepo
if ! git subrepo version &> /dev/null 2>&1; then
  echo "✗ git-subrepo not installed. Run: ./scripts/install-git-subrepo.sh"
  exit 1
fi
echo "✓ git-subrepo installed: $(git subrepo version)"

# 检查工作区状态
if ! git diff-index --quiet HEAD --; then
  echo "✗ Working directory has uncommitted changes"
  echo "Please commit or stash changes before migration"
  git status --short
  exit 1
fi
echo "✓ Working directory clean"

# 检查当前分支
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo "✓ Current branch: $CURRENT_BRANCH"

# 备份
if [ "$DO_BACKUP" = true ]; then
  BACKUP_DIR="${REPO_ROOT}-backup-$(date +%Y%m%d-%H%M%S)"
  echo "Creating backup at: $BACKUP_DIR"
  if [ "$DRY_RUN" = false ]; then
    cp -r "$REPO_ROOT" "$BACKUP_DIR"
    echo "✓ Backup created"
  fi
fi

# 确认继续
if [ "$DRY_RUN" = false ]; then
  echo ""
  echo "=== Ready to Migrate ==="
  echo "This will convert ${#SUBMODULES[@]} submodules to subrepos"
  echo "Press Ctrl+C to abort, or Enter to continue..."
  read -r
fi

echo ""
echo "=== Phase 1: Remove Submodule References ==="

# 保存 .gitmodules 信息（用于后续恢复 URL）
GITMODULES_BACKUP="/tmp/gitmodules-backup-$$"
cp .gitmodules "$GITMODULES_BACKUP"
echo "✓ Backed up .gitmodules to $GITMODULES_BACKUP"

if [ "$DRY_RUN" = false ]; then
  # 移除 .gitmodules
  git rm -f .gitmodules
  
  # 移除所有 submodule 的 git 引用
  for sub in "${MIGRATION_ORDER[@]}"; do
    echo "  Removing submodule reference: $sub"
    git rm -f --cached "$sub" 2>/dev/null || true
    rm -rf ".git/modules/$sub" 2>/dev/null || true
  done
  
  git commit -m "chore: remove submodule references before subrepo migration

This is the first step of migrating from submodule to subrepo organization.
The actual content directories are preserved and will be converted in next commit."
fi

echo ""
echo "=== Phase 2: Convert to Subrepo ==="

# 转换每个目录
for sub in "${MIGRATION_ORDER[@]}"; do
  config="${SUBMODULES[$sub]}"
  url="${config%%:*}"
  branch="${config##*:}"
  
  echo ""
  echo "Processing: $sub"
  echo "  URL: $url"
  echo "  Branch: $branch"
  
  if [ "$DRY_RUN" = true ]; then
    echo "  [DRY-RUN] Would execute: git subrepo clone $url $sub -b $branch"
    continue
  fi
  
  # 检查目录是否存在且有内容
  if [ ! -d "$sub" ] || [ -z "$(ls -A "$sub" 2>/dev/null)" ]; then
    echo "  ! Directory empty or missing, cloning fresh..."
    git subrepo clone "$url" "$sub" -b "$branch"
  else
    # 目录存在且有内容，需要特殊处理
    # git subrepo init 要求目录已包含来自远程的内容
    
    # 临时备份目录
    TEMP_DIR="/tmp/${sub}-$$"
    mv "$sub" "$TEMP_DIR"
    
    # clone subrepo（会创建新目录）
    git subrepo clone "$url" "$sub" -b "$branch"
    
    # 如果原目录有本地修改，需要合并
    # 这里简单处理：直接覆盖 subrepo clone 的内容
    rm -rf "$sub"
    mv "$TEMP_DIR" "$sub"
    
    # 重新声明为 subrepo
    git subrepo init "$sub" --remote="$url" --branch="$branch"
  fi
  
  echo "  ✓ Converted: $sub"
done

if [ "$DRY_RUN" = false ]; then
  echo ""
  echo "=== Phase 3: Final Commit ==="
  
  # 添加 .git-subrepo 目录（如果存在）
  if [ -d ".git-subrepo" ]; then
    git add .git-subrepo
  fi
  
  git commit -m "chore: convert all submodules to subrepos

Converted ${#SUBMODULES[@]} submodules to git-subrepo format:
- Layer 0 (基础组件): sbi, linker, console, kernel-*, easy-fs, signal-defs, task-manage, checker
- Layer 1 (中间组件): syscall, signal, sync  
- Layer 2 (上层组件): signal-impl, user
- Layer 3 (内核章节): ch1-ch8

New workflow:
- Update subrepos: git subrepo pull <name> or git subrepo pull --all
- Push changes: git subrepo push <name> or git subrepo push --all
- Check status: git subrepo status
"
fi

# 清理备份
rm -f "$GITMODULES_BACKUP"

echo ""
echo "=== Migration Complete ==="
if [ "$DRY_RUN" = true ]; then
  echo "This was a DRY RUN. No changes were made."
else
  echo "✓ All ${#SUBMODULES[@]} submodules converted to subrepos"
  echo ""
  echo "Next steps:"
  echo "  1. Verify: git subrepo status"
  echo "  2. Test build: cd tg-rcore-tutorial-ch3 && cargo build"
  echo "  3. Update documentation (README.md)"
  echo "  4. Push to remote: git push origin $CURRENT_BRANCH"
  
  if [ "$DO_BACKUP" = true ]; then
    echo ""
    echo "Backup location: $BACKUP_DIR"
    echo "Delete backup after verification: rm -rf $BACKUP_DIR"
  fi
fi
