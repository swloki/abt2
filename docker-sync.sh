#!/bin/bash
set -e

cd /app
echo ">>> 强制更新到最新版本..."
git fetch origin
git checkout -f .
git reset --hard origin/master

# 确保 docker-sync.sh 有执行权限
chmod +x docker-sync.sh

echo ">>> 烹饪依赖..."
cargo chef cook --recipe-path recipe.json --release || cargo chef cook --recipe-path recipe.json

echo ">>> 构建项目..."
cargo build --release

echo ">>> 构建完成！"
ls -la target/release/abt-grpc 2>/dev/null || true

echo ">>> 保持容器运行..."
tail -f /dev/null
