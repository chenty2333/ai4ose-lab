#!/bin/bash
# install-git-subrepo.sh
# 安装 git-subrepo 工具

set -e

echo "=== Installing git-subrepo ==="

# 检测操作系统
OS="$(uname -s)"
case "$OS" in
  Linux*)
    if command -v apt-get &> /dev/null; then
      echo "Detected Debian/Ubuntu, installing via apt..."
      sudo apt-get update
      sudo apt-get install -y git-subrepo
    elif command -v dnf &> /dev/null; then
      echo "Detected Fedora, installing via dnf..."
      sudo dnf install -y git-subrepo
    else
      echo "Manual installation required for this Linux distribution"
      MANUAL_INSTALL=true
    fi
    ;;
  Darwin*)
    if command -v brew &> /dev/null; then
      echo "Detected macOS with Homebrew, installing via brew..."
      brew install git-subrepo
    else
      echo "Homebrew not found, manual installation required"
      MANUAL_INSTALL=true
    fi
    ;;
  *)
    echo "Unsupported OS: $OS"
    MANUAL_INSTALL=true
    ;;
esac

# 手动安装（如果包管理器不可用）
if [ "$MANUAL_INSTALL" = true ]; then
  echo "Performing manual installation..."
  
  INSTALL_DIR="${GIT_SUBREPO_INSTALL_DIR:-/usr/local}"
  SUBREPO_DIR="/tmp/git-subrepo-$$"
  
  # 克隆 git-subrepo 仓库
  git clone https://github.com/ingydotnet/git-subrepo "$SUBREPO_DIR"
  
  # 安装
  cd "$SUBREPO_DIR"
  sudo make install PREFIX="$INSTALL_DIR"
  
  # 清理
  rm -rf "$SUBREPO_DIR"
  
  echo "git-subrepo installed to $INSTALL_DIR"
fi

# 验证安装
if command -v git-subrepo &> /dev/null || git subrepo version &> /dev/null 2>&1; then
  echo "✓ git-subrepo installed successfully"
  git subrepo version
else
  echo "✗ git-subrepo installation failed"
  echo "Please install manually: https://github.com/ingydotnet/git-subrepo#installation"
  exit 1
fi
