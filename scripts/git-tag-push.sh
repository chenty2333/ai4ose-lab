#!/usr/bin/env bash
# 对 rcore-tutorial-tangrams/ 下的每个 crate 独立 git 仓库执行：
#   git tag v{版本号}
#   git push origin v{版本号}
#
# 用法:
#   bash git-tag-push.sh               # 处理所有 crate
#   bash git-tag-push.sh tg-sbi ch1   # 只处理指定 crate 目录名
#
# 可选环境变量:
#   FORCE=1   强制覆盖已存在的 tag（git tag -f）

set -e

TANGRAMS="$(pwd)"
FORCE="${FORCE:-0}"

# ── 确定要处理的目录列表 ──────────────────────────────────────────────────────
if [ $# -gt 0 ]; then
    TARGET_DIRS=()
    for d in "$@"; do
        TARGET_DIRS+=("$TANGRAMS/$d")
    done
else
    TARGET_DIRS=("$TANGRAMS"/*)
fi

# ── 从 Cargo.toml [package] 段提取字段值 ─────────────────────────────────────
extract_field() {
    local file="$1" field="$2"
    awk -v field="$field" '
        /^\[package\]/ { in_pkg=1; next }
        /^\[/          { in_pkg=0 }
        in_pkg && $0 ~ "^" field " *= *\"" {
            match($0, /"[^"]+"/)
            print substr($0, RSTART+1, RLENGTH-2); exit
        }
    ' "$file"
}

# ── 统计 ─────────────────────────────────────────────────────────────────────
TOTAL=0; OK=0; SKIP=0; FAIL=0

for dir in "${TARGET_DIRS[@]}"; do
    [ -d "$dir" ] || { echo "跳过: $dir (目录不存在)"; continue; }

    cargo_toml="$dir/Cargo.toml"
    [ -f "$cargo_toml" ] || { echo "跳过: $dir (无 Cargo.toml)"; continue; }

    crate_name=$(extract_field "$cargo_toml" "name")
    crate_ver=$(extract_field  "$cargo_toml" "version")

    if [ -z "$crate_name" ] || [ -z "$crate_ver" ]; then
        echo "跳过: $dir (无法解析 name/version)"
        continue
    fi

    TOTAL=$((TOTAL + 1))
    TAG="v${crate_ver}"
    dir_short="${dir#$TANGRAMS/}"

    echo ""
    echo "========================================"
    echo "  crate : $crate_name"
    echo "  版本  : $crate_ver"
    echo "  tag   : $TAG"
    echo "  目录  : $dir_short"
    echo "========================================"

    # 检查是否为 git 仓库
    if ! git -C "$dir" rev-parse --git-dir > /dev/null 2>&1; then
        echo "  ✗ 跳过: 不是 git 仓库"
        SKIP=$((SKIP + 1))
        continue
    fi

    # 检查是否有 remote origin
    if ! git -C "$dir" remote get-url origin > /dev/null 2>&1; then
        echo "  ✗ 跳过: 没有 remote origin"
        SKIP=$((SKIP + 1))
        continue
    fi

    # 检查 tag 是否已存在
    if git -C "$dir" rev-parse "$TAG" > /dev/null 2>&1; then
        if [ "$FORCE" = "1" ]; then
            echo "  ! tag $TAG 已存在，强制覆盖 (FORCE=1)"
            git -C "$dir" tag -f "$TAG"
            git -C "$dir" push origin "$TAG" --force
            echo "  ✓ 强制覆盖完成: $TAG"
            OK=$((OK + 1))
        else
            echo "  - tag $TAG 已存在，跳过 (可设 FORCE=1 强制覆盖)"
            SKIP=$((SKIP + 1))
        fi
        continue
    fi

    # 打 tag 并推送
    if git -C "$dir" tag "$TAG" \
    && git -C "$dir" push origin "$TAG"; then
        echo "  ✓ 完成: $TAG → $(git -C "$dir" remote get-url origin)"
        OK=$((OK + 1))
    else
        echo "  ✗ 失败: $dir_short  tag=$TAG"
        FAIL=$((FAIL + 1))
    fi
done

# ── 汇总 ─────────────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo "  汇总: 共 $TOTAL 个 crate"
echo "    成功: $OK  跳过: $SKIP  失败: $FAIL"
echo "========================================"

[ "$FAIL" -eq 0 ] || exit 1
