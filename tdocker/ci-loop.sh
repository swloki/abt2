#!/bin/bash
# ABT 本地 CI 容器：循环轮询 origin/main，检测到更新则自动构建 + rsync 部署远程
# 用法：docker run -d --restart always --name abt-ci -e POLL_INTERVAL=60 abt-builder2 bash tdocker/ci-loop.sh
# idle（无更新）只占几十 MB；仅 cargo build 那几分钟吃内存，编译完即释放。
set -u

cd /app

POLL_INTERVAL="${POLL_INTERVAL:-60}"

echo ">>> ABT CI 启动 — 每 ${POLL_INTERVAL}s 轮询 origin/main"

while true; do
    if git fetch origin main 2>/dev/null; then
        LOCAL=$(git rev-parse HEAD 2>/dev/null || echo "")
        REMOTE=$(git rev-parse origin/main 2>/dev/null || echo "")

        if [ -n "$REMOTE" ] && [ "$LOCAL" != "$REMOTE" ]; then
            echo ">>> [$(date '+%F %T')] main 更新（${LOCAL:0:7} → ${REMOTE:0:7}），开始构建部署"
            if bash ./tdocker/docker-sync.sh; then
                echo ">>> [$(date '+%F %T')] 部署成功"
            else
                echo ">>> [$(date '+%F %T')] 部署失败，下个周期重试"
            fi
        fi
    else
        echo ">>> [$(date '+%F %T')] git fetch 失败，下个周期重试"
    fi

    sleep "$POLL_INTERVAL"
done
