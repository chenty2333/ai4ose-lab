#!/bin/bash

# 批量执行脚本：对app-*目录执行指定命令

usage() {
    echo "用法: $0 -c \"CMD\""
    echo ""
    echo "参数说明:"
    echo "  -c \"CMD\"  要执行的命令字符串（必须包含在引号中）"
    echo ""
    echo "示例:"
    echo "  $0 -c \"cargo build\""
    echo "  $0 -c \"cargo clean\""
    echo "  $0 -c \"cat Cargo.toml\""
    exit 1
}

# 解析命令行参数
while getopts "c:" opt; do
    case $opt in
        c)
            CMD="$OPTARG"
            ;;
        \?)
            echo "无效选项: -$OPTARG" >&2
            usage
            ;;
        :)
            echo "选项 -$OPTARG 需要参数." >&2
            usage
            ;;
    esac
done

# 检查是否提供了命令
if [ -z "$CMD" ]; then
    echo "错误: 必须使用 -c 参数指定要执行的命令"
    usage
fi

# 获取当前目录
SCRIPT_DIR="$(pwd)"

# 查找所有app-*目录并排序
APP_DIRS=$(find "$SCRIPT_DIR" -maxdepth 1 -type d -name "app-*" | sort)

# 统计目录数量
TOTAL=0
SUCCESS=0
FAILED=0

echo "=========================================="
echo "在以下目录执行命令: $CMD"
echo "=========================================="

# 遍历每个app-*目录
for dir in $APP_DIRS; do
    TOTAL=$((TOTAL + 1))
    dir_name=$(basename "$dir")
    
    echo ""
    echo "[$TOTAL/$TOTAL] 进入目录: $dir_name"
    echo "----------------------------------------"
    
    cd "$dir" || {
        echo "错误: 无法进入目录 $dir"
        FAILED=$((FAILED + 1))
        continue
    }
    
    # 执行命令
    eval "$CMD"
    exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo "✓ 成功: $dir_name"
        SUCCESS=$((SUCCESS + 1))
    else
        echo "✗ 失败: $dir_name (退出码: $exit_code)"
        FAILED=$((FAILED + 1))
    fi
done

# 返回脚本目录
cd "$SCRIPT_DIR"

# 输出统计信息
echo ""
echo "=========================================="
echo "执行完成统计"
echo "=========================================="
echo "总计: $TOTAL 个目录"
echo "成功: $SUCCESS 个"
echo "失败: $FAILED 个"
echo "=========================================="

# 如果有失败的命令，返回非零退出码
if [ $FAILED -gt 0 ]; then
    exit 1
fi

exit 0
