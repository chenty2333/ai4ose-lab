#!/bin/bash
# 切换子模块协议：git 或 https
if [ "$1" = "git" ]; then
    # 替换为 git 协议
    sed -i 's/https:\/\/github.com\//git@github.com:/g' .gitmodules
elif [ "$1" = "https" ]; then
    # 替换为 https 协议
    sed -i 's/git@github.com:/https:\/\/github.com\//g' .gitmodules
else
    echo "用法: $0 [git|https]"
    exit 1
fi
# 更新子模块配置
git submodule sync
echo "子模块协议已切换为: $1"