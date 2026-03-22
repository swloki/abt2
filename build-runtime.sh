#!/bin/bash
set -e

IMAGE_NAME="${1:-abt-runtime}"
IMAGE_TAG="${2:-latest}"

# 复制 SSH 密钥到项目目录（临时，用于 Docker 构建）
echo ">>> 复制 SSH 密钥..."
cp -r ~/.ssh ./.ssh_build 2>/dev/null || cp -r /c/Users/weichen/.ssh ./.ssh_build

# 从 builder 构建运行镜像
echo ">>> 构建运行时镜像: ${IMAGE_NAME}:${IMAGE_TAG}"
docker build --target runtime -t "${IMAGE_NAME}:${IMAGE_TAG}" .

# 清理临时 SSH 文件
echo ">>> 清理临时文件..."
rm -rf ./.ssh_build

echo ">>> 运行时镜像构建完成: ${IMAGE_NAME}:${IMAGE_TAG}"
echo ">>> 运行: docker run --rm -p 8001:8001 -v \$(pwd)/abt-grpc/config.toml:/app/config.toml ${IMAGE_NAME}:${IMAGE_TAG}"
