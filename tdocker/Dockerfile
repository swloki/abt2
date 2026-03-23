# ABT 多阶段构建
# Stage 1: 构建阶段 - 编译 Rust 项目
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

# RUN cargo build --release

# 保持容器运行
CMD ["bash", "./tdocker/docker-sync.sh"]
