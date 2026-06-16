#!/usr/bin/env bash
# restart-abt.sh — 停止 → 编译 → 启动 abt-web 服务
# 用法:
#   ./scripts/restart-abt.sh          # 正常重启
#   ./scripts/restart-abt.sh --clippy # 重启前先跑 clippy
#   ./scripts/restart-abt.sh --check  # 只编译检查，不启动

set -euo pipefail
cd "$(dirname "$0")/.."

# 停止旧进程
echo "⏹  停止旧进程..."
powershell -Command "Stop-Process -Name abt-web -Force -ErrorAction SilentlyContinue" 2>/dev/null || true
sleep 1
bunx kill-port 8000

# 可选 clippy
if [[ "${1:-}" == "--clippy" ]]; then
  echo "🔍 运行 clippy..."
  cargo clippy -p abt-web 2>&1 | tail -3
fi

# 编译
echo "🔧 编译 abt-web..."
cargo build -p abt-web 2>&1 | tail -3

# 只检查模式
if [[ "${1:-}" == "--check" ]]; then
  echo "✅ 编译检查完成（--check 模式，不启动）"
  exit 0
fi

# 启动
echo "🚀 启动服务..."
./target/debug/abt-web.exe &

# 等待就绪
echo "⏳ 等待服务就绪..."
for i in $(seq 1 10); do
  if curl -s -o /dev/null -w "%{http_code}" http://localhost:8000/login 2>/dev/null | grep -q "200"; then
    echo "✅ 服务已就绪: http://localhost:8000"
    exit 0
  fi
  sleep 1
done

echo "⚠️  服务启动超时（10s），请手动检查"
exit 1
