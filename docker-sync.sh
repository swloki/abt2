#!/bin/bash
set -e

cd /app
echo ">>> 强制更新到最新版本..."
git fetch origin
git checkout -f .
git reset --hard origin/master

echo ">>> 构建项目..."
export DATABASE_URL="postgres://postgres:123456@172.17.0.1:5432/abt"
cargo build --release -p abt-grpc

echo ">>> 构建完成！"
ls -la target/release/abt-grpc 2>/dev/null || true

cp target/release/abt-grpc /app/target/abt-grpc
