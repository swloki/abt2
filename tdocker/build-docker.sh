#!/bin/bash
set -e

# 使用当前时间作为缓存破坏参数
CACHE_BUST=$(date +%s)

# 复制 SSH 密钥到项目目录（临时，用于 Docker 构建）
echo ">>> 复制 SSH 密钥..."
cp -r ~/.ssh ../.ssh_build 2>/dev/null || cp -r /c/Users/weichen/.ssh ../.ssh_build

# 构建编译镜像
echo ">>> 构建编译镜像..."
docker build -f Dockerfile --target builder -t abt-builder --build-arg CACHE_BUST=$CACHE_BUST ..

# 清理临时 SSH 文件
echo ">>> 清理临时文件..."
rm -rf ../.ssh_build

echo ">>> 编译镜像构建完成: abt-builder"
echo ">>> 启动开发容器: docker run --rm -it abt-builder bash"
echo ">>> 容器内执行: bash docker-sync.sh"
