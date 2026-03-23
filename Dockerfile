# ABT 多阶段构建
# Stage 1: 构建阶段 - 编译 Rust 项目
# Stage 2: 运行阶段 - Debian slim 镜像

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
RUN git fetch origin && git reset --hard origin/master && git pull origin master

ENV DATABASE_URL="postgres://postgres:123456@172.17.0.1:5432/abt"
RUN cargo build --release

# 保持容器运行
CMD ["bash", "./docker-sync.sh"]

# ============================================
# Stage 2: Runtime (Ubuntu latest)
# ============================================
FROM ubuntu:latest AS runtime

LABEL maintainer="weichen"
LABEL description="ABT gRPC Server"

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libgcc-s1 \
    libstdc++6 \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 从 builder 复制二进制
COPY --from=builder /app/target/release/abt-grpc /app/abt-grpc
COPY abt-grpc/config.toml /app/config.toml
CMD ["./abt-grpc"]
