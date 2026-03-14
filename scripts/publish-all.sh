#!/usr/bin/env bash
# 按依赖拓扑顺序发布所有 crate 到 crates.io
# 用法: bash publish-all.sh
# 需要已登录 crates.io: cargo login <token>

set -e

BASE="$(pwd)"
WAIT_SECS=40   # crates.io 索引刷新等待时间

# 统计
_SKIPPED=()
_FAILED=()

publish_crate() {
    local dir="$BASE/$1"
    local name="$2"
    echo ""
    echo "========================================"
    echo "  发布: $name"
    echo "  路径: $dir"
    echo "========================================"

    # 捕获 cargo publish 的完整输出（stdout + stderr 合并）
    local output
    local exit_code=0
    output=$(cd "$dir" && cargo publish --allow-dirty 2>&1) || exit_code=$?

    if [ $exit_code -eq 0 ]; then
        echo "  ✓ $name 发布完成"
        return 0
    fi

    # 判断是否为"版本已存在"错误
    # crates.io 返回的错误消息形如：
    #   "already uploaded"  /  "already exists"  /  "crate version already exists"
    if echo "$output" | grep -qiE "already uploaded|already exists|crate version already"; then
        echo "  - $name 该版本已存在于 crates.io，跳过"
        _SKIPPED+=("$name")
        return 0
    fi

    # 其他真实错误：打印输出并记录，但不退出（继续后续 crate）
    echo "$output"
    echo "  ✗ $name 发布失败（非'已存在'错误）"
    _FAILED+=("$name")
}

wait_index() {
    echo ""
    echo ">>> 等待 ${WAIT_SECS}s，让 crates.io 索引更新 ..."
    sleep "$WAIT_SECS"
}

# ── Layer 1：无 tg-* 内部依赖 ────────────────────────────────────────────────
publish_crate "tg-rcore-tutorial-sbi" "tg-rcore-tutorial-sbi"
publish_crate "tg-rcore-tutorial-console" "tg-rcore-tutorial-console"
publish_crate "tg-rcore-tutorial-linker" "tg-rcore-tutorial-linker"
publish_crate "tg-rcore-tutorial-kernel-context" "tg-rcore-tutorial-kernel-context"
publish_crate "tg-rcore-tutorial-signal-defs" "tg-rcore-tutorial-signal-defs"
publish_crate "tg-rcore-tutorial-kernel-alloc" "tg-rcore-tutorial-kernel-alloc"
publish_crate "tg-rcore-tutorial-kernel-vm" "tg-rcore-tutorial-kernel-vm"
publish_crate "tg-rcore-tutorial-task-manage" "tg-rcore-tutorial-task-manage"
publish_crate "tg-rcore-tutorial-easy-fs" "tg-rcore-tutorial-easy-fs"
publish_crate "tg-rcore-tutorial-checker" "tg-rcore-tutorial-checker"

wait_index

# ── Layer 2：依赖 L1 ──────────────────────────────────────────────────────────
# tg-syscall       ← tg-signal-defs
# tg-signal        ← tg-kernel-context, tg-signal-defs
# tg-sync          ← tg-task-manage
# ch1              ← tg-sbi
publish_crate "tg-rcore-tutorial-syscall" "tg-rcore-tutorial-syscall"
publish_crate "tg-rcore-tutorial-signal" "tg-rcore-tutorial-signal"
publish_crate "tg-rcore-tutorial-sync" "tg-rcore-tutorial-sync"
publish_crate "tg-rcore-tutorial-ch1" "tg-rcore-tutorial-ch1"

wait_index

# ── Layer 3：依赖 L2 ──────────────────────────────────────────────────────────
# tg-user       ← tg-console, tg-syscall
# tg-signal-impl← tg-kernel-context, tg-signal
# ch2           ← tg-sbi, tg-linker, tg-console, tg-kernel-context, tg-syscall
# ch3           ← 同 ch2
# ch4           ← ch2 + tg-kernel-alloc, tg-kernel-vm
# ch5           ← ch4 + tg-task-manage
# ch6           ← ch5 + tg-easy-fs
publish_crate "tg-rcore-tutorial-user" "tg-rcore-tutorial-user"
publish_crate "tg-rcore-tutorial-signal-impl" "tg-rcore-tutorial-signal-impl"
publish_crate "tg-rcore-tutorial-ch2" "tg-rcore-tutorial-ch2"
publish_crate "tg-rcore-tutorial-ch3" "tg-rcore-tutorial-ch3"
publish_crate "tg-rcore-tutorial-ch4" "tg-rcore-tutorial-ch4"
publish_crate "tg-rcore-tutorial-ch5" "tg-rcore-tutorial-ch5"
publish_crate "tg-rcore-tutorial-ch6" "tg-rcore-tutorial-ch6"

wait_index

# ── Layer 4：依赖 L3 ──────────────────────────────────────────────────────────
# ch7  ← ch6 + tg-signal, tg-signal-impl
# ch8  ← ch7 + tg-sync
publish_crate "tg-rcore-tutorial-ch7" "tg-rcore-tutorial-ch7"
publish_crate "tg-rcore-tutorial-ch8" "tg-rcore-tutorial-ch8"

echo ""
echo "========================================"
echo "  全部 23 个 crate 处理完毕"

if [ ${#_SKIPPED[@]} -gt 0 ]; then
    echo "  跳过（版本已存在）: ${#_SKIPPED[@]} 个"
    for n in "${_SKIPPED[@]}"; do echo "    - $n"; done
fi

if [ ${#_FAILED[@]} -gt 0 ]; then
    echo "  失败（其他错误）: ${#_FAILED[@]} 个"
    for n in "${_FAILED[@]}"; do echo "    ✗ $n"; done
    echo "========================================"
    exit 1
else
    echo "  成功（含跳过）: 无失败项"
    echo "========================================"
fi
