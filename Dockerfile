# ABT Docker Build Image
# 用于在 Docker 中编译 Rust NAPI-RS 项目
# 构建命令: sh build-docker.sh

FROM rust:latest

# 设置环境变量
ENV DEBIAN_FRONTEND=noninteractive

# 配置中国镜像源 (清华源)
RUN sed -i 's|http://deb.debian.org|https://mirrors.tuna.tsinghua.edu.cn|g' /etc/apt/sources.list.d/debian.sources
RUN mkdir -p ~/.cargo && printf '[source.crates-io]\nreplace-with = "tuna"\n[source.tuna]\nregistry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"\n' > ~/.cargo/config.toml

# 安装系统依赖和 Node.js (用于 NAPI-RS)
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    libprotobuf-dev \
    git \
    openssh-client \
    sshpass \
    rsync \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# 安装 bun
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:$PATH"

# 验证安装
RUN rustc --version && cargo --version && node --version && npm --version && bun --version

# 设置 SSH 目录
RUN mkdir -p /root/.ssh && chmod 700 /root/.ssh

# 复制 SSH 密钥 (从构建上下文)
COPY .ssh_build /root/.ssh/
RUN chmod 600 /root/.ssh/id_rsa 2>/dev/null || true \
    && chmod 644 /root/.ssh/id_rsa.pub 2>/dev/null || true \
    && ssh-keyscan github.com >> /root/.ssh/known_hosts

# 设置工作目录
WORKDIR /app

# 克隆代码 (SSH)
RUN git clone git@github.com:swloki/abt2.git .
RUN git config --local user.email "lokisw@gmail.com" && git config --local user.name "weichen"
RUN git checkout master

# 安装 cargo-chef (用于增量编译)
RUN cargo install cargo-chef

# 准备依赖
RUN cargo chef prepare --recipe-path recipe.json

# 安装 NAPI-RS CLI
RUN bun install -g @napi-rs/cli

RUN chmod +x docker-sync.sh

# 默认执行同步脚本
CMD ["bash", "docker-sync.sh"]
