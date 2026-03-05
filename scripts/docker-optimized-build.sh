#!/bin/bash

# 基于LLM的多智能体并行协作系统 - 优化Docker构建脚本

set -e

echo "🚀 开始优化Docker构建..."

# 检查Docker是否安装
if ! command -v docker &> /dev/null; then
    echo "❌ Docker未安装，请先安装Docker"
    exit 1
fi

# 检查Docker Compose是否可用
if ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose不可用，请确保Docker Compose插件已安装"
    exit 1
fi

# 显示系统信息
echo "📊 系统信息："
echo "  Docker版本: $(docker --version | cut -d' ' -f3 | cut -d',' -f1)"
echo "  Docker Compose版本: $(docker compose version | grep -oP 'v\d+\.\d+\.\d+')"
echo "  当前目录: $(pwd)"

# 清理之前的构建缓存（可选）
echo "🧹 清理之前的构建缓存..."
docker builder prune -f 2>/dev/null || true

# 检查.dockerignore文件
if [ -f ".dockerignore" ]; then
    echo "✅ 找到.dockerignore文件，将忽略不必要的文件"
else
    echo "⚠️  未找到.dockerignore文件，建议创建以加速构建"
fi

# 构建参数
IMAGE_NAME="agent-parallel-system"
TAG="latest"
BUILD_ARGS=""

# 检查是否使用国内镜像
if [ "$USE_CHINA_MIRROR" = "true" ]; then
    echo "🇨🇳 使用国内镜像源构建..."
    BUILD_ARGS="--build-arg RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static --build-arg RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup"
fi

# 开始构建
echo "🔨 开始构建Docker镜像..."
echo "  镜像名称: ${IMAGE_NAME}:${TAG}"
echo "  构建参数: ${BUILD_ARGS}"
echo "  开始时间: $(date)"

# 执行构建
start_time=$(date +%s)
docker build \
    -t "${IMAGE_NAME}:${TAG}" \
    -t "${IMAGE_NAME}:$(date +%Y%m%d)" \
    ${BUILD_ARGS} \
    .

end_time=$(date +%s)
build_duration=$((end_time - start_time))

echo "✅ Docker构建完成！"
echo "  结束时间: $(date)"
echo "  构建耗时: ${build_duration}秒"

# 显示镜像信息
echo "📦 镜像信息："
docker images | grep "${IMAGE_NAME}"

# 测试镜像（可选）
if [ "$TEST_IMAGE" = "true" ]; then
    echo "🧪 测试镜像..."
    docker run --rm "${IMAGE_NAME}:${TAG}" --version 2>/dev/null || \
    docker run --rm "${IMAGE_NAME}:${TAG}" --help 2>/dev/null || \
    echo "⚠️  无法运行测试命令，但镜像构建成功"
fi

# 构建优化建议
echo ""
echo "💡 Docker构建优化建议："
echo "========================"
echo "1. 🚀 使用国内镜像源"
echo "   编辑 /etc/docker/daemon.json 添加："
echo "   {"
echo '     "registry-mirrors": ["https://docker.mirrors.ustc.edu.cn"]'
echo "   }"
echo "   然后重启Docker: sudo systemctl restart docker"
echo ""
echo "2. 🗑️  定期清理Docker缓存"
echo "   docker system prune -a -f"
echo "   docker builder prune -f"
echo ""
echo "3. 📦 使用多阶段构建"
echo "   当前Dockerfile已使用多阶段构建，减少最终镜像大小"
echo ""
echo "4. 🔧 优化构建缓存"
echo "   将不经常变化的指令放在前面"
echo "   合并RUN指令减少镜像层数"
echo ""
echo "5. 🐳 升级Docker版本"
echo "   使用最新稳定版Docker提升构建效率"
echo ""
echo "6. 💾 增加系统资源"
echo "   确保有足够的内存和CPU资源"
echo "   关闭其他占用资源的进程"
echo ""
echo "🎉 构建完成！可以使用以下命令运行："
echo "   docker run -p 8000:8000 ${IMAGE_NAME}:${TAG}"
echo "   或使用docker-compose: docker compose up -d"