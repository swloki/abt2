#!/bin/bash
set -e

cd /app
echo ">>> 强制更新到最新版本..."
export RUSTUP_DIST_SERVER="https://rsproxy.cn"
export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
git fetch origin
git checkout -f .
git reset --hard origin/main

echo ">>> 构建项目..."
# DATABASE_URL 从仓库 .env 读取（远程库 119.29.23.115，schema 最新）。
# 原硬编码 172.17.0.1 宿主库 schema 旧（无 GL 表），导致 sqlx::query! 编译期验证 GL 报 E0282。
export DATABASE_URL="$(grep '^DATABASE_URL=' .env | sed 's/^DATABASE_URL=//; s/^"//; s/"$//')"
cargo build --release

echo ">>> 构建完成！"
ls -la target/release/abt-web 2>/dev/null || true

MONITOR_DIR="./target/release/abt-web"
TARGET_USER="weichen"
TARGET_HOST="119.29.23.115"
TARGET_DIR="/data/abt3"
SSH_PASSWORD="chenxi,,0514"

echo "开始"
echo "开始上传"
ssh-keyscan -H $TARGET_HOST >>~/.ssh/known_hosts
#sshpass -p $SSH_PASSWORD rsync -avz $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
#sshpass -p $SSH_PASSWORD rsync -avz $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
#sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "cd $TARGET_DIR && /home/weichen/.cargo/bin/cargo build --release"
sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "rm -f /data/abt3/abt-web"
echo "删除成功"
sshpass -p $SSH_PASSWORD rsync -avz $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"

# 同步 static 文件夹
echo ">>> 同步 static 文件夹..."
sshpass -p $SSH_PASSWORD rsync -avz --delete ./static/ "$TARGET_USER@$TARGET_HOST:$TARGET_DIR/static/"

sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "sshpass -p chenxi,,0514 sudo docker restart abt3"
