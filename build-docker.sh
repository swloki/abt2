#!/bin/bash
set -e

# 复制 SSH 密钥到项目目录（临时，用于 Docker 构建）
echo ">>> 复制 SSH 密钥..."
cp -r ~/.ssh ./.ssh_build 2>/dev/null || cp -r /c/Users/weichen/.ssh ./.ssh_build

# 构建 Docker 镜像
echo ">>> 构建 Docker 镜像..."
docker build -t abt-builder .

# 清理临时 SSH 文件
echo ">>> 清理临时文件..."
rm -rf ./.ssh_build

echo ">>> 镜像构建完成: abt-builder"
echo ">>> 运行构建: docker run --rm abt-builder"
