#!/bin/bash
set -e

IMAGE_NAME="${1:-abt-runtime}"
IMAGE_TAG="${2:-latest}"
FULL_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"

# 构建运行时镜像（从本地已编译的二进制文件）
echo ">>> 构建运行时镜像: ${FULL_IMAGE}"
docker build -f Dockerfile.runtime -t "${FULL_IMAGE}" ..

echo ">>> 运行时镜像构建完成: ${FULL_IMAGE}"
echo ""
echo ">>> 可用命令:"
echo "    运行容器: docker run --rm -p 8001:8001 ${FULL_IMAGE}"
echo "    导出镜像: docker save -o ${IMAGE_NAME}-${IMAGE_TAG}.tar ${FULL_IMAGE}"
echo "    加载镜像: docker load -i ${IMAGE_NAME}-${IMAGE_TAG}.tar"
echo ""

# 如果带 export 参数，导出镜像
if [ "$3" == "export" ]; then
    OUTPUT_FILE="${IMAGE_NAME}-${IMAGE_TAG}.tar"
    echo ">>> 导出镜像到: ${OUTPUT_FILE}"
    docker save -o "../${OUTPUT_FILE}" "${FULL_IMAGE}"
    echo ">>> 导出完成: ${OUTPUT_FILE}"
fi
 docker save -o export-abt.tar export:abt
