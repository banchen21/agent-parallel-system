#!/bin/bash

# 基于LLM的多智能体并行协作系统 - 最小化启动脚本

set -e

echo "🚀 启动基于LLM的多智能体并行协作系统（最小化版）..."

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

# 创建必要的目录
echo "📁 创建必要的目录..."
mkdir -p storage
mkdir -p logs

# 检查环境配置文件
if [ ! -f ".env" ]; then
    echo "⚠️  未找到.env文件，使用.env.example创建..."
    cp .env.example .env
    echo "📝 请编辑.env文件配置您的环境变量"
fi

# 停止可能存在的容器
echo "🧹 清理之前的容器..."
docker compose -f docker-compose.minimal.yml down -v 2>/dev/null || true

# 构建应用镜像
echo "🔨 构建Docker镜像..."
docker build -t agent-parallel-system:latest .

# 启动最小化服务
echo "🐳 启动Docker服务（最小化版）..."
docker compose -f docker-compose.minimal.yml up -d

# 等待服务启动
echo "⏳ 等待服务启动..."
sleep 30

# 检查服务状态
echo "🔍 检查服务状态..."

# 检查数据库
if docker compose -f docker-compose.minimal.yml exec postgres pg_isready -U postgres; then
    echo "✅ 数据库服务正常"
else
    echo "❌ 数据库服务异常"
    docker compose -f docker-compose.minimal.yml logs postgres
    exit 1
fi

# 检查Redis
if docker compose -f docker-compose.minimal.yml exec redis redis-cli ping | grep -q "PONG"; then
    echo "✅ Redis服务正常"
else
    echo "❌ Redis服务异常"
    docker compose -f docker-compose.minimal.yml logs redis
    exit 1
fi

# 等待API服务启动
echo "⏳ 等待API服务启动..."
sleep 30

# 检查API服务
if curl -f http://localhost:8000/health &> /dev/null; then
    echo "✅ API服务正常"
else
    echo "❌ API服务异常，检查日志..."
    docker compose -f docker-compose.minimal.yml logs api
    exit 1
fi

echo ""
echo "🎉 系统启动成功！"
echo ""
echo "📊 服务访问地址："
echo "   - API服务: http://localhost:8000"
echo "   - API文档: http://localhost:8000/api/v1/docs"
echo "   - 健康检查: http://localhost:8000/health"
echo ""
echo "🗄️  数据库："
echo "   - PostgreSQL: localhost:5432"
echo "   - Redis: localhost:6379"
echo ""
echo "🔧 管理命令："
echo "   - 查看日志: docker compose -f docker-compose.minimal.yml logs -f"
echo "   - 停止服务: docker compose -f docker-compose.minimal.yml down"
echo "   - 重启服务: docker compose -f docker-compose.minimal.yml restart"
echo "   - 查看状态: docker compose -f docker-compose.minimal.yml ps"
echo ""
echo "💡 提示："
echo "   这是最小化版本，只包含核心服务（PostgreSQL、Redis、API）"
echo "   如果需要完整功能，请使用 ./scripts/start.sh"