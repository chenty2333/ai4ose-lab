#!/usr/bin/env bash
# 对 rcore-tutorial-tangrams/ 下的每个 crate 独立 git 仓库执行：
#   git add .
#   git commit -m "publish {crate名} {版本号}"
#   git push
#
# 用法:
#   bash git-commit-push.sh            # 处理所有 crate
#   bash git-commit-push.sh tg-sbi ch1 # 只处理指定 crate 目录名

set -e

TANGRAMS="$(pwd)"

# ── 确定要处理的目录列表 ──────────────────────────────────────────────────────
if [ $# -gt 0 ]; then
    # 命令行指定了目录名
    DIRS=("$@")
    TARGET_DIRS=()
    for d in "${DIRS[@]}"; do
        TARGET_DIRS+=("$TANGRAMS/$d")
    done
else
    # 从 crates.txt 读取目录列表
    CRATES_FILE="$(cd "$(dirname "$0")" && pwd)/crates.txt"
    TARGET_DIRS=()
    while IFS= read -r d || [ -n "$d" ]; do
        [ -z "$d" ] && continue  # 跳过空行
        TARGET_DIRS+=("$TANGRAMS/$d")
    done < "$CRATES_FILE"
fi

# ── 工具函数 ─────────────────────────────────────────────────────────────────
extract_field() {
    # 从 Cargo.toml 提取 name 或 version（只取 [package] 段的第一个匹配）
    local file="$1" field="$2"
    awk -v field="$field" '
        /^\[package\]/ { in_pkg=1; next }
        /^\[/          { in_pkg=0 }
        in_pkg && $0 ~ "^" field " *= *\"" {
            match($0, /"[^"]+"/)
            val = substr($0, RSTART+1, RLENGTH-2)
            print val; exit
        }
    ' "$file"
}

# ── 统计 ─────────────────────────────────────────────────────────────────────
TOTAL=0; OK=0; SKIP=0; FAIL=0

for dir in "${TARGET_DIRS[@]}"; do
    [ -d "$dir" ] || { echo "跳过: $dir (不存在)"; continue; }

    cargo_toml="$dir/Cargo.toml"
    [ -f "$cargo_toml" ] || { echo "跳过: $dir (无 Cargo.toml)"; continue; }

    crate_name=$(extract_field "$cargo_toml" "name")
    crate_ver=$(extract_field  "$cargo_toml" "version")

    if [ -z "$crate_name" ] || [ -z "$crate_ver" ]; then
        echo "跳过: $dir (无法解析 name/version)"
        continue
    fi

    TOTAL=$((TOTAL + 1))
    dir_short="${dir#$TANGRAMS/}"

    echo ""
    echo "========================================"
    echo "  crate : $crate_name"
    echo "  版本  : $crate_ver"
    echo "  目录  : $dir_short"
    echo "========================================"

    # 检查是否有 git 仓库
    if ! git -C "$dir" rev-parse --git-dir > /dev/null 2>&1; then
        echo "  ✗ 跳过: 不是 git 仓库"
        SKIP=$((SKIP + 1))
        continue
    fi

    # 检查是否有 remote
    if ! git -C "$dir" remote get-url origin > /dev/null 2>&1; then
        echo "  ✗ 跳过: 没有 remote origin"
        SKIP=$((SKIP + 1))
        continue
    fi

    # 检查工作区是否有变更
    if git -C "$dir" diff --quiet && git -C "$dir" diff --cached --quiet; then
        UNTRACKED=$(git -C "$dir" ls-files --others --exclude-standard | wc -l)
        if [ "$UNTRACKED" -eq 0 ]; then
            echo "  - 无变更，跳过 commit"
            SKIP=$((SKIP + 1))
            continue
        fi
    fi

    COMMIT_MSG="publish $crate_name $crate_ver"

    if git -C "$dir" add . \
    && git -C "$dir" commit -m "$COMMIT_MSG" \
    && git -C "$dir" push; then
        echo "  ✓ 完成: $COMMIT_MSG"
        OK=$((OK + 1))
    else
        echo "  ✗ 失败: $dir_short"
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
