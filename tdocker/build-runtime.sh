#!/bin/bash
set -e

IMAGE_NAME="${1:-abt-runtime}"
IMAGE_TAG="${2:-latest}"

# 构建运行时镜像（从本地已编译的二进制文件）
echo ">>> 构建运行时镜像: ${IMAGE_NAME}:${IMAGE_TAG}"
docker build -f Dockerfile.runtime -t "${IMAGE_NAME}:${IMAGE_TAG}" ..

echo ">>> 运行时镜像构建完成: ${IMAGE_NAME}:${IMAGE_TAG}"
echo ">>> 运行: docker run --rm -p 8001:8001 ${IMAGE_NAME}:${IMAGE_TAG}"
