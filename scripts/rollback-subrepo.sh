#!/bin/bash
# rollback-subrepo.sh
# 回滚 subrepo 迁移，恢复为 submodule
#
# 用法:
#   ./scripts/rollback-subrepo.sh [--backup-dir=<path>]
#
# 选项:
#   --backup-dir  指定备份目录路径（如果之前创建了备份）
#
# 注意: 此脚本会重置当前分支到迁移前的状态

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BACKUP_DIR=""

# 解析参数
for arg in "$@"; do
  case $arg in
    --backup-dir=*)
      BACKUP_DIR="${arg#*=}"
      shift
      ;;
    *)
      echo "Unknown argument: $arg"
      exit 1
      ;;
  esac
done

cd "$REPO_ROOT"

echo "=== Subrepo to Submodule Rollback ==="
echo "Repository: $REPO_ROOT"
echo ""

# 检查当前状态
if [ ! -f ".gitmodules" ] && git subrepo status &> /dev/null 2>&1; then
  echo "✓ Confirmed: Repository appears to be in subrepo state"
else
  echo "! Warning: Repository may not be in subrepo state"
fi

# 确认
echo ""
echo "This will revert the subrepo migration and restore submodule structure."
echo "All local changes in subrepos will be LOST."
echo ""
echo "Press Ctrl+C to abort, or Enter to continue..."
read -r

# 方式 1: 从备份恢复（如果提供了备份目录）
if [ -n "$BACKUP_DIR" ] && [ -d "$BACKUP_DIR" ]; then
  echo ""
  echo "=== Restoring from Backup ==="
  echo "Backup: $BACKUP_DIR"
  
  # 删除当前仓库内容（保留 .git）
  find . -maxdepth 1 ! -name '.git' ! -name '.' -exec rm -rf {} \;
  
  # 从备份恢复
  rsync -a --exclude='.git' "$BACKUP_DIR/" .
  
  echo "✓ Restored from backup"
  echo ""
  echo "Run: git submodule init && git submodule update"
  exit 0
fi

# 方式 2: Git revert（如果备份不可用）
echo ""
echo "=== Attempting Git Revert ==="

# 查找迁移相关的 commits
echo "Looking for migration commits..."
MIGRATION_COMMITS=$(git log --oneline --grep="submodule" --grep="subrepo" -i | head -10)

if [ -z "$MIGRATION_COMMITS" ]; then
  echo "✗ No migration commits found in recent history"
  echo ""
  echo "Manual rollback options:"
  echo "  1. git reset --hard <commit-before-migration>"
  echo "  2. Restore from backup (if available)"
  exit 1
fi

echo "Found potential migration commits:"
echo "$MIGRATION_COMMITS"
echo ""

# 找到迁移前的 commit
FIRST_MIGRATION=$(git log --oneline --grep="remove submodule references" | head -1 | awk '{print $1}')
if [ -n "$FIRST_MIGRATION" ]; then
  COMMIT_BEFORE=$(git rev-parse "$FIRST_MIGRATION^")
  echo "First migration commit: $FIRST_MIGRATION"
  echo "Will reset to: $COMMIT_BEFORE"
  echo ""
  echo "Press Ctrl+C to abort, or Enter to continue..."
  read -r
  
  git reset --hard "$COMMIT_BEFORE"
  
  echo ""
  echo "✓ Reset to pre-migration state"
  echo ""
  echo "Now run: git submodule init && git submodule update"
else
  echo "✗ Could not automatically determine rollback point"
  echo ""
  echo "Please manually identify the commit before migration and run:"
  echo "  git reset --hard <commit-hash>"
  echo "  git submodule init"
  echo "  git submodule update"
  exit 1
fi

echo ""
echo "=== Rollback Complete ==="
