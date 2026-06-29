#!/bin/bash
set -e

# 复制 SSH 密钥到项目目录（临时，用于 Docker 构建）
echo ">>> 复制 SSH 密钥..."
cp -r ~/.ssh ../.ssh_build 2>/dev/null || cp -r /c/Users/weichen/.ssh ../.ssh_build

# 构建编译镜像
echo ">>> 构建编译镜像..."
docker build -f Dockerfile --target builder -t abt-builder2 ..

# 清理临时 SSH 文件
echo ">>> 清理临时文件..."
rm -rf ../.ssh_build

echo ">>> 编译镜像构建完成: abt-builder2"
echo ""
echo ">>> 单次部署：  docker run --rm abt-builder2"
echo ">>> CI 常驻：    docker run -d --restart always --name abt-ci -e POLL_INTERVAL=60 abt-builder2 bash tdocker/ci-loop.sh"
echo ">>> 查看 CI 日志：docker logs -f abt-ci"
