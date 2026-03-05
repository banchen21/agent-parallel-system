#!/bin/bash
# 一键更换 Ubuntu 22.04 (Jammy) 源为阿里云镜像，并安装 libssl-dev 和 pkg-config

set -e  # 遇到错误立即退出

# 备份当前源列表（带时间戳）
backup_file="/etc/apt/sources.list.backup_$(date +%Y%m%d%H%M%S)"
echo "📦 备份当前源列表到 $backup_file"
sudo cp /etc/apt/sources.list "$backup_file"

# 写入阿里云镜像源（仅针对 jammy）
echo "🔄 更换源为阿里云镜像..."
sudo tee /etc/apt/sources.list > /dev/null <<EOF
deb http://mirrors.aliyun.com/ubuntu/ jammy main restricted universe multiverse
deb http://mirrors.aliyun.com/ubuntu/ jammy-security main restricted universe multiverse
deb http://mirrors.aliyun.com/ubuntu/ jammy-updates main restricted universe multiverse
deb http://mirrors.aliyun.com/ubuntu/ jammy-proposed main restricted universe multiverse
deb http://mirrors.aliyun.com/ubuntu/ jammy-backports main restricted universe multiverse
EOF

# 更新软件包列表
echo "📡 更新软件包列表..."
sudo apt update

# 安装所需开发包
echo "🔧 安装 libssl-dev 和 pkg-config..."
sudo apt install libssl-dev pkg-config -y

echo "✅ 所有操作完成！现在可以重新运行 cargo build 了。"
