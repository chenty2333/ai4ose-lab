#!/usr/bin/env bash
# 全局版本号替换脚本
# 用法: bash bump-version.sh <旧版本号> <新版本号>
# 示例: bash bump-version.sh 0.4.2-preview.2 0.4.2-preview.3

set -e

OLD_VER="${1:-}"
NEW_VER="${2:-}"
REPO_ROOT="$(pwd)"
TANGRAMS="$REPO_ROOT"

# ── 参数校验 ────────────────────────────────────────────────────────────────
if [ -z "$OLD_VER" ] || [ -z "$NEW_VER" ]; then
    echo "用法: bash bump-version.sh <旧版本号> <新版本号>"
    echo "示例: bash bump-version.sh 0.4.2-preview.2 0.4.2-preview.3"
    exit 1
fi

if [ "$OLD_VER" = "$NEW_VER" ]; then
    echo "错误: 新旧版本号相同 ($OLD_VER)"
    exit 1
fi

echo "========================================"
echo "  旧版本: $OLD_VER"
echo "  新版本: $NEW_VER"
echo "  仓库根: $REPO_ROOT"
echo "========================================"

# ── Step 1: 删除缓存目录和编译输出 ────────────────────────────────────────
echo ""
echo ">>> Step 1: 清理缓存目录和编译输出..."
CLEANED=0

# 删除 ch*/tg-user 缓存目录
for dir in "$TANGRAMS"/ch*/tg-user; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "    已删除: ${dir#$REPO_ROOT/}"
        CLEANED=$((CLEANED + 1))
    fi
done

# 删除 ch*/target 编译输出目录
for dir in "$TANGRAMS"/ch*/target; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "    已删除: ${dir#$REPO_ROOT/}"
        CLEANED=$((CLEANED + 1))
    fi
done

# 删除 tg-*/target 编译输出目录
for dir in "$TANGRAMS"/tg-*/target; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "    已删除: ${dir#$REPO_ROOT/}"
        CLEANED=$((CLEANED + 1))
    fi
done

if [ "$CLEANED" -eq 0 ]; then
    echo "    无需清理（所有目录均不存在）"
fi

# ── Step 2: 转义版本号中的特殊字符（用于 sed 正则） ─────────────────────────
OLD_ESC="$(printf '%s' "$OLD_VER" | sed 's/[.]/\\./g')"
NEW_ESC="$(printf '%s' "$NEW_VER" | sed 's/[&/\]/\\&/g')"

# ── Step 3: 查找并替换 Cargo.toml / config.toml ─────────────────────────────
echo ""
echo ">>> Step 2: 搜索包含旧版本号的文件..."

MATCHED_FILES=$(grep -rl "$OLD_ESC" \
    --include="Cargo.toml" --include="config.toml" \
    "$REPO_ROOT" 2>/dev/null || true)

if [ -z "$MATCHED_FILES" ]; then
    echo "    未找到包含 '$OLD_VER' 的文件，无需替换"
    exit 0
fi

FILE_COUNT=$(echo "$MATCHED_FILES" | wc -l)
echo "    找到 $FILE_COUNT 个文件需要替换："
echo "$MATCHED_FILES" | while read -r f; do
    echo "      ${f#$REPO_ROOT/}"
done

echo ""
echo ">>> Step 3: 执行替换 $OLD_VER → $NEW_VER ..."
echo "$MATCHED_FILES" | xargs sed -i "s/$OLD_ESC/$NEW_ESC/g"
echo "    替换完成"

# ── Step 4: 验证 ─────────────────────────────────────────────────────────────
echo ""
echo ">>> Step 4: 验证结果..."
REMAINING=$(grep -rl "$OLD_ESC" \
    --include="Cargo.toml" --include="config.toml" \
    "$REPO_ROOT" 2>/dev/null | wc -l || echo "0")

if [ "$REMAINING" -eq 0 ]; then
    echo "    ✓ 所有文件中的旧版本号已全部替换"
else
    echo "    ✗ 警告: 仍有 $REMAINING 个文件包含旧版本号:"
    grep -rl "$OLD_ESC" \
        --include="Cargo.toml" --include="config.toml" \
        "$REPO_ROOT" 2>/dev/null | while read -r f; do
        echo "      ${f#$REPO_ROOT/}"
    done
    exit 1
fi

echo ""
echo "========================================"
echo "  版本替换完成: $OLD_VER → $NEW_VER"
echo "  清理缓存目录: $CLEANED 个"
echo "  替换文件数量: $FILE_COUNT 个"
echo "========================================"
