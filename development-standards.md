# 基于LLM的多智能体并行协作系统 - 开发规范和标准

## 1. 代码规范

### 1.1 Rust代码规范

#### 1.1.1 代码风格
- **格式化工具**: rustfmt
- **行长度**: 100字符
- **命名约定**: 遵循Rust官方命名规范
- **导入顺序**: std → 外部crate → 本地模块

```rust
// ✅ 正确的导入顺序
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::task::Task;
use crate::services::task_service::TaskService;
```

#### 1.1.2 命名约定
- **结构体/枚举名**: PascalCase (`TaskService`, `AgentManager`)
- **函数/方法名**: snake_case (`create_task`, `get_agent_status`)
- **变量名**: snake_case (`task_id`, `agent_capabilities`)
- **常量**: UPPER_SNAKE_CASE (`MAX_RETRY_COUNT`, `DEFAULT_TIMEOUT`)
- **模块名**: snake_case (`task_service`, `agent_manager`)

#### 1.1.3 类型和错误处理
```rust
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub workspace_id: Uuid,
    pub requirements: HashMap<String, serde_json::Value>,
}

impl CreateTaskRequest {
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            anyhow::bail!("标题不能为空");
        }
        if self.title.len() > 500 {
            anyhow::bail!("标题长度不能超过500字符");
        }
        Ok(())
    }
}

pub async fn create_task(
    request: CreateTaskRequest,
    task_service: &TaskService,
    user_id: Uuid,
) -> Result<Task> {
    request.validate().context("验证任务请求失败")?;
    
    let task = task_service
        .create_task(request, user_id)
        .await
        .context("创建任务失败")?;
    
    Ok(task)
}
```

### 1.2 数据库操作规范

#### 1.2.1 SQLx模型定义
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub workspace_id: Uuid,
    pub assigned_agent_id: Option<Uuid>,
    pub requirements: serde_json::Value,
    pub context: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub progress: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_priority", rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Task {
    pub fn to_response(&self) -> TaskResponse {
        TaskResponse {
            id: self.id,
            title: self.title.clone(),
            description: self.description.clone(),
            status: self.status.clone(),
            priority: self.priority.clone(),
            progress: self.progress,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
```

#### 1.2.2 数据库操作最佳实践
```rust
use sqlx::{PgPool, postgres::PgPoolOptions};
use anyhow::{Result, Context};

// 数据库连接池
pub async fn create_db_pool(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .connect(database_url)
        .await
        .context("创建数据库连接池失败")
}

// 数据库操作示例
pub struct TaskRepository {
    pool: PgPool,
}

impl TaskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn get_task_by_id(&self, task_id: Uuid) -> Result<Option<Task>> {
        sqlx::query_as!(
            Task,
            r#"
            SELECT * FROM tasks
            WHERE id = $1
            "#,
            task_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("查询任务失败")
    }
    
    pub async fn create_task(&self, task: &CreateTaskRequest, user_id: Uuid) -> Result<Task> {
        sqlx::query_as!(
            Task,
            r#"
            INSERT INTO tasks (
                title, description, status, priority,
                workspace_id, created_by, requirements, context
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            task.title,
            task.description,
            TaskStatus::Pending as TaskStatus,
            task.priority,
            task.workspace_id,
            user_id,
            task.requirements,
            task.context
        )
        .fetch_one(&self.pool)
        .await
        .context("创建任务失败")
    }
    
    pub async fn get_tasks_with_agents(&self, workspace_id: Uuid) -> Result<Vec<(Task, Option<Agent>)>> {
        // 使用JOIN避免N+1查询
        let tasks_with_agents = sqlx::query!(
            r#"
            SELECT
                t.*,
                a.id as agent_id,
                a.name as agent_name,
                a.status as agent_status
            FROM tasks t
            LEFT JOIN agents a ON t.assigned_agent_id = a.id
            WHERE t.workspace_id = $1
            ORDER BY t.created_at DESC
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
        .context("查询任务和智能体失败")?;
        
        // 转换为领域对象
        Ok(tasks_with_agents.into_iter().map(|row| {
            let task = Task {
                id: row.id,
                title: row.title,
                description: row.description,
                status: row.status.parse().unwrap_or(TaskStatus::Pending),
                priority: row.priority.parse().unwrap_or(TaskPriority::Medium),
                workspace_id: row.workspace_id,
                assigned_agent_id: row.assigned_agent_id,
                requirements: row.requirements.unwrap_or_default(),
                context: row.context.unwrap_or_default(),
                result: row.result,
                progress: row.progress.unwrap_or(0),
                created_at: row.created_at,
                updated_at: row.updated_at,
            };
            
            let agent = row.agent_id.map(|id| Agent {
                id,
                name: row.agent_name.unwrap_or_default(),
                status: row.agent_status.unwrap_or_default().parse().unwrap_or(AgentStatus::Offline),
                // 其他字段...
            });
            
            (task, agent)
        }).collect())
    }
}
```

### 1.3 API开发规范

#### 1.3.1 Axum路由定义
```rust
use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde_json::json;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTaskRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub workspace_id: Uuid,
    pub requirements: HashMap<String, serde_json::Value>,
}

pub fn task_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_task))
        .route("/{task_id}", get(get_task))
}

/// 创建新任务
///
/// - **title**: 任务标题
/// - **description**: 任务描述
/// - **priority**: 任务优先级
/// - **workspace_id**: 工作空间ID
pub async fn create_task(
    State(app_state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<CreateTaskRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 验证请求
    request.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;
    
    // 验证工作空间权限
    let workspace = app_state
        .workspace_service
        .get_workspace(request.workspace_id, user.id)
        .await?;
    
    if workspace.is_none() {
        return Err(AppError::NotFound("工作空间不存在".to_string()));
    }
    
    // 创建任务
    let task = app_state
        .task_service
        .create_task(request, user.id)
        .await?;
    
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": task.to_response(),
            "message": "任务创建成功"
        }))
    ))
}

pub async fn get_task(
    State(app_state): State<AppState>,
    user: AuthenticatedUser,
    Path(task_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let task = app_state
        .task_service
        .get_task_by_id(task_id, user.id)
        .await?;
    
    match task {
        Some(task) => Ok(Json(json!({
            "success": true,
            "data": task.to_response(),
            "message": "获取任务成功"
        }))),
        None => Err(AppError::NotFound("任务不存在".to_string())),
    }
}
```

#### 1.3.2 错误处理规范
```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("验证错误: {0}")]
    ValidationError(String),
    
    #[error("未找到资源: {0}")]
    NotFound(String),
    
    #[error("权限拒绝: {0}")]
    PermissionDenied(String),
    
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("内部服务器错误")]
    InternalServerError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string()),
            AppError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, "内部服务器错误".to_string()),
        };
        
        let body = Json(json!({
            "success": false,
            "error": {
                "code": status.as_str(),
                "message": error_message
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));
        
        (status, body).into_response()
    }
}

// 从anyhow::Error转换
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // 根据错误类型进行更精确的转换
        if let Some(db_err) = err.downcast_ref::<sqlx::Error>() {
            match db_err {
                sqlx::Error::RowNotFound => AppError::NotFound("资源不存在".to_string()),
                _ => AppError::DatabaseError(db_err.clone()),
            }
        } else {
            AppError::InternalServerError
        }
    }
}
```

## 2. 项目结构规范

### 2.1 目录结构
```
agent-parallel-system/
├── app/
│   ├── __init__.py
│   ├── main.py              # FastAPI应用入口
│   ├── core/                # 核心配置和工具
│   │   ├── __init__.py
│   │   ├── config.py        # 应用配置
│   │   ├── database.py      # 数据库配置
│   │   ├── security.py      # 安全相关
│   │   └── exceptions.py    # 自定义异常
│   ├── models/              # 数据模型
│   │   ├── __init__.py
│   │   ├── user.py
│   │   ├── task.py
│   │   ├── agent.py
│   │   └── workspace.py
│   ├── schemas/             # Pydantic模型
│   │   ├── __init__.py
│   │   ├── user.py
│   │   ├── task.py
│   │   └── common.py
│   ├── services/            # 业务逻辑层
│   │   ├── __init__.py
│   │   ├── task_service.py
│   │   ├── agent_service.py
│   │   ├── workspace_service.py
│   │   └── orchestrator_service.py
│   ├── api/                 # API路由
│   │   ├── __init__.py
│   │   ├── dependencies.py  # 依赖注入
│   │   ├── routes/
│   │   │   ├── __init__.py
│   │   │   ├── auth.py
│   │   │   ├── tasks.py
│   │   │   ├── agents.py
│   │   │   └── workspaces.py
│   ├── workers/             # 后台任务
│   │   ├── __init__.py
│   │   ├── task_worker.py
│   │   └── notification_worker.py
│   └── utils/               # 工具函数
│       ├── __init__.py
│       ├── validators.py
│       ├── formatters.py
│       └── helpers.py
├── tests/                   # 测试代码
│   ├── __init__.py
│   ├── conftest.py
│   ├── unit/
│   ├── integration/
│   └── fixtures/
├── migrations/              # 数据库迁移
├── docs/                    # 文档
├── scripts/                 # 部署脚本
├── requirements.txt
├── requirements-dev.txt
├── pyproject.toml
├── Dockerfile
└── docker-compose.yml
```

### 2.2 模块导入规范
```python
# ✅ 正确的导入方式
from app.models.task import Task
from app.schemas.task import TaskCreate, TaskResponse
from app.services.task_service import TaskService
from app.core.config import settings

# ❌ 避免的导入方式
from app.models import *  # 避免通配符导入
from ..services.task_service import TaskService  # 避免相对导入
```

## 3. 测试规范

### 3.1 测试结构
```python
import pytest
from fastapi.testclient import TestClient
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from app.main import app
from app.core.database import Base, get_db

# 测试数据库配置
SQLALCHEMY_DATABASE_URL = "sqlite:///./test.db"
engine = create_engine(SQLALCHEMY_DATABASE_URL, connect_args={"check_same_thread": False})
TestingSessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

@pytest.fixture(scope="function")
def test_db():
    """测试数据库会话"""
    Base.metadata.create_all(bind=engine)
    db = TestingSessionLocal()
    try:
        yield db
    finally:
        db.close()
        Base.metadata.drop_all(bind=engine)

@pytest.fixture(scope="function")
def client(test_db):
    """测试客户端"""
    def override_get_db():
        try:
            yield test_db
        finally:
            pass
    
    app.dependency_overrides[get_db] = override_get_db
    with TestClient(app) as test_client:
        yield test_client
    app.dependency_overrides.clear()
```

### 3.2 单元测试示例
```python
class TestTaskService:
    """任务服务单元测试"""
    
    @pytest.mark.asyncio
    async def test_create_task_success(self, test_db):
        """测试成功创建任务"""
        # 准备
        task_service = TaskService(test_db)
        task_data = {
            "title": "测试任务",
            "description": "测试任务描述",
            "priority": "MEDIUM",
            "workspace_id": "test_workspace",
            "created_by": "test_user"
        }
        
        # 执行
        task = await task_service.create_task(**task_data)
        
        # 断言
        assert task.id is not None
        assert task.title == "测试任务"
        assert task.status == "PENDING"
        assert task.priority == "MEDIUM"
    
    @pytest.mark.asyncio
    async def test_create_task_invalid_priority(self, test_db):
        """测试创建任务时无效的优先级"""
        task_service = TaskService(test_db)
        task_data = {
            "title": "测试任务",
            "description": "测试任务描述", 
            "priority": "INVALID",
            "workspace_id": "test_workspace",
            "created_by": "test_user"
        }
        
        # 执行和断言
        with pytest.raises(ValueError, match="无效的优先级"):
            await task_service.create_task(**task_data)
```

### 3.3 集成测试示例
```python
class TestTaskAPI:
    """任务API集成测试"""
    
    def test_create_task_endpoint(self, client, test_user_token):
        """测试创建任务端点"""
        # 准备
        headers = {"Authorization": f"Bearer {test_user_token}"}
        task_data = {
            "title": "API测试任务",
            "description": "通过API创建的任务",
            "priority": "HIGH",
            "workspace_id": "test_workspace"
        }
        
        # 执行
        response = client.post("/api/v1/tasks/", json=task_data, headers=headers)
        
        # 断言
        assert response.status_code == 200
        data = response.json()
        assert data["success"] is True
        assert data["data"]["title"] == "API测试任务"
        assert data["data"]["status"] == "PENDING"
```

## 4. 文档规范

### 4.1 API文档
```python
@router.get("/{task_id}", response_model=TaskResponse)
async def get_task(
    task_id: str,
    current_user: User = Depends(get_current_user),
    db: Session = Depends(get_db)
):
    """根据ID获取任务详情
    
    ## 参数
    - **task_id**: 任务唯一标识符
    
    ## 响应
    - **200**: 成功返回任务详情
    - **404**: 任务不存在
    - **403**: 没有访问权限
    
    ## 示例
    ```json
    {
        "id": "task_uuid",
        "title": "数据分析报告",
        "status": "IN_PROGRESS",
        "progress": 50
    }
    ```
    """
    task = await task_service.get_task_by_id(task_id, current_user.id)
    if not task:
        raise TaskNotFoundError(task_id)
    
    return TaskResponse.from_orm(task)
```

### 4.2 代码文档
```python
def assign_task_to_agent(task_id: str, agent_id: str) -> bool:
    """将任务分配给智能体
    
    此函数负责将指定任务分配给合适的智能体，考虑以下因素：
    - 智能体的当前负载
    - 智能体的能力匹配度
    - 任务的优先级
    
    Args:
        task_id: 要分配的任务ID
        agent_id: 目标智能体ID
        
    Returns:
        bool: 分配是否成功
        
    Raises:
        TaskNotFoundError: 当任务不存在时
        AgentNotFoundError: 当智能体不存在时
        AgentBusyError: 当智能体负载过高时
    """
    # 实现逻辑
    pass
```

## 5. 部署和运维规范

### 5.1 Docker配置
```dockerfile
# 使用官方Python运行时作为父镜像
FROM python:3.11-slim

# 设置工作目录
WORKDIR /app

# 复制依赖文件
COPY requirements.txt .

# 安装依赖
RUN pip install --no-cache-dir -r requirements.txt

# 复制应用代码
COPY . .

# 创建非root用户
RUN useradd -m -u 1000 appuser && chown -R appuser:appuser /app
USER appuser

# 暴露端口
EXPOSE 8000

# 定义健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# 启动命令
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
```

### 5.2 环境配置
```python
# app/core/config.py
from pydantic import BaseSettings

class Settings(BaseSettings):
    """应用配置"""
    
    # 应用配置
    app_name: str = "Agent Parallel System"
    environment: str = "development"
    debug: bool = False
    
    # 数据库配置
    database_url: str
    redis_url: str
    
    # 安全配置
    secret_key: str
    algorithm: str = "HS256"
    access_token_expire_minutes: int = 30
    
    # LLM配置
    openai_api_key: str = ""
    openai_base_url: str = "https://api.openai.com/v1"
    
    class Config:
        env_file = ".env"
        case_sensitive = False

settings = Settings()
```

### 5.3 日志规范
```python
import logging
import json
from pythonjsonlogger import jsonlogger

# 配置JSON格式日志
class JsonFormatter(jsonlogger.JsonFormatter):
    def add_fields(self, log_record, record, message_dict):
        super().add_fields(log_record, record, message_dict)
        log_record['timestamp'] = record.created
        log_record['level'] = record.levelname
        log_record['logger'] = record.name

# 日志配置
def setup_logging():
    """配置结构化日志"""
    logger = logging.getLogger()
    logger.setLevel(logging.INFO)
    
    # 控制台处理器
    console_handler = logging.StreamHandler()
    formatter = JsonFormatter('%(timestamp)s %(level)s %(name)s %(message)s')
    console_handler.setFormatter(formatter)
    logger.addHandler(console_handler)

# 使用示例
logger = logging.getLogger(__name__)

def process_task(task_id: str):
    """处理任务"""
    logger.info("开始处理任务", extra={
        "task_id": task_id,
        "action": "task_processing_start"
    })
    
    try:
        # 处理逻辑
        logger.info("任务处理完成", extra={
            "task_id": task_id,
            "action": "task_processing_complete",
            "duration": 120
        })
    except Exception as e:
        logger.error("任务处理失败", extra={
            "task_id": task_id,
            "action": "task_processing_failed",
            "error": str(e)
        })
        raise
```

## 6. 安全规范

### 6.1 输入验证
```python
from pydantic import BaseModel, validator, constr
import re

class UserCreate(BaseModel):
    username: constr(min_length=3, max_length=50, strip_whitespace=True)
    email: str
    password: constr(min_length=8)
    
    @validator('username')
    def validate_username(cls, v):
        if not re.match(r'^[a-zA-Z0-9_]+$', v):
            raise ValueError('用户名只能包含字母、数字和下划线')
        return v
    
    @validator('email')
    def validate_email(cls, v):
        if not re.match(r'^[^@]+@[^@]+\.[^@]+$', v):
            raise ValueError('邮箱格式不正确')
        return v.lower()
    
    @validator('password')
    def validate_password(cls, v):
        if not any(c.isupper() for c in v):
            raise ValueError('密码必须包含至少一个大写字母')
        if not any(c.islower() for c in v):
            raise ValueError('密码必须包含至少一个小写字母')
        if not any(c.isdigit() for c in v):
            raise ValueError('密码必须包含至少一个数字')
        return v
```

### 6.2 SQL注入防护
```python
# ✅ 使用参数化查询
from sqlalchemy import text

def get_user_by_username(username: str) -> Optional[User]:
    """根据用户名获取用户（安全版本）"""
    with get_db_session() as session:
        # 使用参数化查询防止SQL注入
        query = text("SELECT * FROM users WHERE username = :username")
        result = session.execute(query, {"username": username})
        return result.fetchone()

# ❌ 避免字符串拼接
# query = f"SELECT * FROM users WHERE username = '{username}'"  # 危险！
```

## 7. 性能优化规范

### 7.1 数据库优化
```python
# ✅ 使用索引优化查询
def get_recent_tasks(workspace_id: str, limit: int = 50) -> List[Task]:
    """获取工作空间最近的任务"""
    with get_db_session() as session:
        return session.query(Task)\
            .filter(Task.workspace_id == workspace_id)\
            .order_by(Task.created_at.desc())\
            .limit(limit)\
            .all()

# ✅ 使用批量操作
def create_multiple_tasks(tasks_data: List[Dict]) -> List[Task]:
    """批量创建任务"""
    with get_db_session() as session:
        tasks = [Task(**data) for data in tasks_data]
        session.bulk_save_objects(tasks)
        session.commit()
        return tasks
```

### 7.2 缓存策略
```python
import redis
from functools import wraps

redis_client = redis.Redis.from_url(settings.redis_url)

def cache_result(ttl: int = 300):
    """缓存装饰器"""
    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            # 生成缓存键
            cache_key = f"{func.__name__}:{str(args)}:{str(kwargs)}"
            
            # 尝试从缓存获取
            cached_result = redis_client.get(cache_key)
            if cached_result:
                return json.loads(cached_result)
            
            # 执行函数并缓存结果
            result = await func(*args, **kwargs)
            redis_client.setex(cache_key, ttl, json.dumps(result))
            
            return result
        return wrapper
    return decorator

@cache_result(ttl=600)
async def get_agent_capabilities(agent_id: str) -> List[str]:
    """获取智能体能力（带缓存）"""
    # 数据库查询逻辑
    pass
```

---

*此开发规范和标准文档为团队开发提供统一的指导，确保代码质量、可维护性和安全性。所有开发人员都应严格遵守这些规范。*