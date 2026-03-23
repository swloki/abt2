# ABT 多阶段构建 (使用 BuildKit 缓存)
# 构建命令: DOCKER_BUILDKIT=1 docker build -t abt-grpc .

# ============================================
# Stage 1: Builder
# ============================================
FROM rust:latest AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV DATABASE_URL="postgres://postgres:123456@172.17.0.1:5432/abt"

# 配置中国镜像源 (清华源)
RUN sed -i 's|http://deb.debian.org|https://mirrors.tuna.tsinghua.edu.cn|g' /etc/apt/sources.list.d/debian.sources
RUN mkdir -p ~/.cargo && printf '[source.crates-io]\nreplace-with = "tuna"\n[source.tuna]\nregistry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"\n' > ~/.cargo/config.toml

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制所有源代码
COPY . .

# 使用缓存挂载加速编译
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release
    #cp /app/target/release/abt-grpc /app/abt-grpc

# ============================================
# Stage 2: Runtime
# ============================================
FROM ubuntu:latest AS runtime

LABEL maintainer="weichen"

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libgcc-s1 \
    libstdc++6 \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/abt-grpc /app/abt-grpc
COPY abt-grpc/config.toml /app/config.toml

ENV TZ=Asia/Shanghai

CMD ["./abt-grpc"]
