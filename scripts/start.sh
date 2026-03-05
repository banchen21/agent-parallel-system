#!/bin/bash

# 基于LLM的多智能体并行协作系统 - 启动脚本

set -e

echo "🚀 启动基于LLM的多智能体并行协作系统..."

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

# 启动服务
echo "🐳 启动Docker服务..."
docker compose up -d

# 等待服务启动
echo "⏳ 等待服务启动..."
sleep 30

# 检查服务状态
echo "🔍 检查服务状态..."

# 检查数据库
if docker compose exec postgres pg_isready -U postgres; then
    echo "✅ 数据库服务正常"
else
    echo "❌ 数据库服务异常"
    exit 1
fi

# 检查Redis
if docker compose exec redis redis-cli ping | grep -q "PONG"; then
    echo "✅ Redis服务正常"
else
    echo "❌ Redis服务异常"
    exit 1
fi

# 检查API服务
if curl -f http://localhost:8000/health &> /dev/null; then
    echo "✅ API服务正常"
else
    echo "❌ API服务异常"
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
echo "📈 监控服务（如果启用）："
echo "   - Prometheus: http://localhost:9090"
echo "   - Grafana: http://localhost:3000"
echo ""
echo "💡 下一步："
echo "   1. 访问API文档了解接口"
echo "   2. 注册用户并开始使用"
echo "   3. 配置智能体并分配任务"
echo ""
echo "🔧 管理命令："
echo "   - 查看日志: docker compose logs -f"
echo "   - 停止服务: docker compose down"
echo "   - 重启服务: docker compose restart"
echo "   - 查看状态: docker compose ps"