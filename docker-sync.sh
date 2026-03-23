#!/bin/bash
set -e

cd /app
echo ">>> 强制更新到最新版本..."
git fetch origin
git checkout -f .
git reset --hard origin/master

echo ">>> 构建项目..."
cargo build --release -p abt-grpc

echo ">>> 构建完成！"
ls -la target/release/abt-grpc 2>/dev/null || true

cp target/release/abt-grpc /app/target/abt-grpc
