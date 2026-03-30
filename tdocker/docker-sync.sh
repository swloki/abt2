#!/bin/bash
set -e

cd /app
echo ">>> 强制更新到最新版本..."
export RUSTUP_DIST_SERVER="https://rsproxy.cn"
export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
git fetch origin
git checkout -f .
git reset --hard origin/master

echo ">>> 构建项目..."
export DATABASE_URL="postgres://postgres:123456@172.17.0.1:5432/abt"
cargo build --release -p abt-grpc

echo ">>> 构建完成！"
ls -la target/release/abt-grpc 2>/dev/null || true


MONITOR_DIR="./target/release/abt-grpc"
TARGET_USER="weichen"
TARGET_HOST="119.29.23.115"
TARGET_DIR="/data/abt2"
SSH_PASSWORD="chenxi,,0514"

echo "开始"

#sshpass -p $SSH_PASSWORD rsync -avz $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
#sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "cd $TARGET_DIR && /home/weichen/.cargo/bin/cargo build --release"
sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "rm -f /data/abt2/abt-grpc"
echo "删除成功";
sshpass -p $SSH_PASSWORD  rsync -avz  $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
# MONITOR_DIR="./dist/server"
# TARGET_DIR="/data/cnstrip/dist/"
# sshpass -p $SSH_PASSWORD  rsync -avz  $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"



sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "sshpass -p chenxi,,0514 sudo docker restart abt2-grpc"
