# ABT 多阶段构建
# Stage 1: 构建阶段 - 编译 Rust 项目
# Stage 2: 运行阶段 - 轻量 Alpine 镜像

# ============================================
# Stage 1: Builder
# ============================================
FROM rust:latest AS builder

ENV DEBIAN_FRONTEND=noninteractive

# 配置中国镜像源 (清华源)
RUN sed -i 's|http://deb.debian.org|https://mirrors.tuna.tsinghua.edu.cn|g' /etc/apt/sources.list.d/debian.sources
RUN mkdir -p ~/.cargo && printf '[source.crates-io]\nreplace-with = "tuna"\n[source.tuna]\nregistry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"\n' > ~/.cargo/config.toml

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    protobuf-compiler \
    libprotobuf-dev \
    git \
    openssh-client \
    rsync \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

# SSH 配置
RUN mkdir -p /root/.ssh && chmod 700 /root/.ssh

# 复制 SSH 密钥
COPY .ssh_build /root/.ssh/
RUN chmod 600 /root/.ssh/id_rsa 2>/dev/null || true \
    && chmod 644 /root/.ssh/id_rsa.pub 2>/dev/null || true \
    && ssh-keyscan github.com >> /root/.ssh/known_hosts

WORKDIR /app

# 克隆代码
RUN git clone git@github.com:swloki/abt2.git .
RUN git config --local user.email "lokisw@gmail.com" && git config --local user.name "weichen"
RUN git checkout master

# 安装 cargo-chef (增量编译)
RUN cargo install cargo-chef

# 安装 musl 目标
RUN rustup target add x86_64-unknown-linux-musl

# 烹饪依赖
RUN cargo chef prepare --recipe-path recipe.json

# 编译 (仅 abt-grpc 二进制，musl 静态链接)
RUN cargo chef cook --recipe-path recipe.json --release
RUN cargo build --release -p abt-grpc --target x86_64-unknown-linux-musl

# 保持容器运行
CMD ["tail", "-f", "/dev/null"]

# ============================================
# Stage 2: Runtime (Alpine)
# ============================================
FROM alpine:latest AS runtime

LABEL maintainer="weichen"
LABEL description="ABT gRPC Server"

# 安装运行时依赖 (musl 静态链接，只需基本库)
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# 创建非 root 用户
RUN addgroup -g 1000 appgroup && adduser -u 1000 -G appgroup -s /bin/sh -D appuser

WORKDIR /app

# 从 builder 复制 musl 静态链接的二进制
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/abt-grpc /app/abt-grpc

USER appuser

ENTRYPOINT ["/app/abt-grpc"]
