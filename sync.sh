

MONITOR_DIR="./tdocker/abt-grpc"
TARGET_USER="weichen"
TARGET_HOST="119.29.23.115"
TARGET_DIR="/data/abt2"
SSH_PASSWORD="chenxi,,0514"

echo "开始上传"
ssh-keyscan -H $TARGET_HOST >> ~/.ssh/known_hosts

#sshpass -p $SSH_PASSWORD rsync -avz $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
#sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "cd $TARGET_DIR && /home/weichen/.cargo/bin/cargo build --release"
sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "rm -f /data/abt2/abt-grpc"
echo "删除成功";
sshpass -p $SSH_PASSWORD  rsync -avz  $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"
# MONITOR_DIR="./dist/server"
# TARGET_DIR="/data/cnstrip/dist/"
# sshpass -p $SSH_PASSWORD  rsync -avz  $MONITOR_DIR "$TARGET_USER@$TARGET_HOST:$TARGET_DIR"



sshpass -p $SSH_PASSWORD ssh $TARGET_USER@$TARGET_HOST "sshpass -p chenxi,,0514 sudo docker restart abt2-grpc"
