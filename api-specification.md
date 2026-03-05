# 基于LLM的多智能体并行协作系统 - API接口规范

## 1. API概述

### 1.1 基础信息
- **API版本**: v1
- **基础URL**: `/api/v1`
- **认证方式**: JWT Bearer Token
- **内容类型**: `application/json`
- **编码**: UTF-8

### 1.2 通用响应格式

#### 成功响应
```json
{
  "success": true,
  "data": {},
  "message": "操作成功",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### 错误响应
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "请求参数验证失败",
    "details": [
      {
        "field": "title",
        "message": "标题不能为空"
      }
    ]
  },
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### 1.3 通用错误码

| 错误码 | HTTP状态码 | 描述 |
|--------|------------|------|
| `VALIDATION_ERROR` | 400 | 请求参数验证失败 |
| `AUTHENTICATION_FAILED` | 401 | 认证失败 |
| `PERMISSION_DENIED` | 403 | 权限不足 |
| `RESOURCE_NOT_FOUND` | 404 | 资源不存在 |
| `RESOURCE_CONFLICT` | 409 | 资源冲突 |
| `RATE_LIMIT_EXCEEDED` | 429 | 请求频率超限 |
| `INTERNAL_SERVER_ERROR` | 500 | 服务器内部错误 |

## 2. 认证API

### 2.1 用户登录

**端点**: `POST /auth/login`

**请求体**:
```json
{
  "username": "user@example.com",
  "password": "password123"
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "token_type": "bearer",
    "expires_in": 3600,
    "refresh_token": "def50200e3b8a...",
    "user": {
      "id": "user_uuid",
      "username": "user@example.com",
      "roles": ["user"]
    }
  },
  "message": "登录成功"
}
```

### 2.2 刷新令牌

**端点**: `POST /auth/refresh`

**请求体**:
```json
{
  "refresh_token": "def50200e3b8a..."
}
```

**响应**: 同登录响应

### 2.3 用户登出

**端点**: `POST /auth/logout`

**认证**: Bearer Token

**响应**:
```json
{
  "success": true,
  "data": null,
  "message": "登出成功"
}
```

## 3. 任务管理API

### 3.1 创建任务

**端点**: `POST /tasks`

**认证**: Bearer Token

**请求体**:
```json
{
  "title": "数据分析报告",
  "description": "分析销售数据并生成报告",
  "priority": "MEDIUM",
  "requirements": {
    "capabilities": ["data_analysis", "report_writing"],
    "timeout": 1800,
    "max_retries": 3
  },
  "context": {
    "workspace_id": "workspace_uuid",
    "input_data": {
      "sales_data_url": "https://example.com/sales.csv"
    }
  },
  "metadata": {
    "customer_id": "cust_123",
    "project": "季度分析"
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "task_uuid",
    "title": "数据分析报告",
    "description": "分析销售数据并生成报告",
    "status": "PENDING",
    "priority": "MEDIUM",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "requirements": {
      "capabilities": ["data_analysis", "report_writing"],
      "timeout": 1800,
      "max_retries": 3
    },
    "context": {
      "workspace_id": "workspace_uuid"
    },
    "metadata": {
      "customer_id": "cust_123",
      "project": "季度分析"
    }
  },
  "message": "任务创建成功"
}
```

### 3.2 获取任务列表

**端点**: `GET /tasks`

**认证**: Bearer Token

**查询参数**:
- `status` (可选): 任务状态过滤
- `priority` (可选): 优先级过滤
- `workspace_id` (可选): 工作空间过滤
- `page` (可选): 页码，默认1
- `page_size` (可选): 每页数量，默认20

**响应**:
```json
{
  "success": true,
  "data": {
    "tasks": [
      {
        "id": "task_uuid",
        "title": "数据分析报告",
        "status": "IN_PROGRESS",
        "priority": "MEDIUM",
        "created_at": "2024-01-01T00:00:00Z",
        "assigned_agent": "agent_uuid"
      }
    ],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total": 150,
      "total_pages": 8
    }
  },
  "message": "获取任务列表成功"
}
```

### 3.3 获取任务详情

**端点**: `GET /tasks/{task_id}`

**认证**: Bearer Token

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "task_uuid",
    "title": "数据分析报告",
    "description": "分析销售数据并生成报告",
    "status": "IN_PROGRESS",
    "priority": "MEDIUM",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:01:00Z",
    "parent_task_id": null,
    "dependencies": [],
    "assigned_agent": "agent_uuid",
    "progress": 50,
    "current_step": "正在分析数据",
    "result": null,
    "requirements": {
      "capabilities": ["data_analysis", "report_writing"],
      "timeout": 1800,
      "max_retries": 3
    },
    "context": {
      "workspace_id": "workspace_uuid",
      "input_data": {
        "sales_data_url": "https://example.com/sales.csv"
      }
    },
    "metadata": {
      "customer_id": "cust_123",
      "project": "季度分析"
    },
    "subtasks": [
      {
        "id": "subtask_uuid",
        "title": "数据清洗",
        "status": "COMPLETED",
        "progress": 100
      }
    ]
  },
  "message": "获取任务详情成功"
}
```

### 3.4 更新任务状态

**端点**: `PUT /tasks/{task_id}/status`

**认证**: Bearer Token

**请求体**:
```json
{
  "status": "COMPLETED",
  "progress": 100,
  "result": {
    "report_url": "https://example.com/report.pdf",
    "summary": "数据分析完成，发现增长趋势"
  },
  "metadata": {
    "execution_time": 120,
    "llm_calls": 15
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "task_uuid",
    "status": "COMPLETED",
    "progress": 100,
    "updated_at": "2024-01-01T00:05:00Z"
  },
  "message": "任务状态更新成功"
}
```

### 3.5 任务分解

**端点**: `POST /tasks/{task_id}/decompose`

**认证**: Bearer Token

**请求体**:
```json
{
  "strategy": "HIERARCHICAL",
  "max_depth": 3,
  "constraints": {
    "max_subtasks": 10,
    "min_complexity": 0.1
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "parent_task_id": "task_uuid",
    "subtasks": [
      {
        "id": "subtask_1",
        "title": "数据收集和清洗",
        "description": "收集销售数据并进行数据清洗",
        "dependencies": [],
        "estimated_duration": 300
      },
      {
        "id": "subtask_2",
        "title": "数据分析",
        "description": "分析清洗后的数据",
        "dependencies": ["subtask_1"],
        "estimated_duration": 600
      },
      {
        "id": "subtask_3",
        "title": "报告生成",
        "description": "基于分析结果生成报告",
        "dependencies": ["subtask_2"],
        "estimated_duration": 900
      }
    ]
  },
  "message": "任务分解成功"
}
```

## 4. 智能体管理API

### 4.1 智能体注册

**端点**: `POST /agents/register`

**认证**: API Key

**请求体**:
```json
{
  "name": "数据分析智能体",
  "description": "专门处理数据分析和报告生成的智能体",
  "capabilities": [
    {
      "name": "data_analysis",
      "description": "数据分析能力",
      "version": "1.0",
      "parameters": {
        "supported_formats": ["csv", "json", "excel"],
        "max_data_size": "100MB"
      }
    },
    {
      "name": "report_writing",
      "description": "报告撰写能力",
      "version": "1.0",
      "parameters": {
        "supported_templates": ["business", "technical", "executive"]
      }
    }
  ],
  "endpoints": {
    "task_execution": "https://agent.example.com/api/tasks",
    "health_check": "https://agent.example.com/health"
  },
  "limits": {
    "max_concurrent_tasks": 5,
    "max_execution_time": 3600
  },
  "metadata": {
    "llm_model": "gpt-4",
    "version": "1.0.0"
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "agent_uuid",
    "name": "数据分析智能体",
    "status": "ONLINE",
    "registered_at": "2024-01-01T00:00:00Z"
  },
  "message": "智能体注册成功"
}
```

### 4.2 获取可用智能体

**端点**: `GET /agents`

**认证**: Bearer Token

**查询参数**:
- `capability` (可选): 按能力过滤
- `status` (可选): 按状态过滤
- `available` (可选): 只返回可用智能体

**响应**:
```json
{
  "success": true,
  "data": {
    "agents": [
      {
        "id": "agent_uuid",
        "name": "数据分析智能体",
        "description": "专门处理数据分析和报告生成的智能体",
        "status": "ONLINE",
        "current_load": 2,
        "max_concurrent_tasks": 5,
        "capabilities": ["data_analysis", "report_writing"],
        "last_heartbeat": "2024-01-01T00:00:30Z"
      }
    ]
  },
  "message": "获取智能体列表成功"
}
```

### 4.3 智能体心跳

**端点**: `POST /agents/{agent_id}/heartbeat`

**认证**: API Key

**请求体**:
```json
{
  "current_load": 2,
  "resource_usage": {
    "cpu": 45.5,
    "memory": 67.2,
    "disk": 23.1
  },
  "active_tasks": ["task_1", "task_2"],
  "metadata": {
    "uptime": 3600,
    "version": "1.0.0"
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "status": "HEALTHY",
    "assigned_tasks": [],
    "system_info": {
      "maintenance_window": null,
      "rate_limits": {
        "max_requests_per_minute": 60
      }
    }
  },
  "message": "心跳更新成功"
}
```

## 5. 工作空间API

### 5.1 创建工作空间

**端点**: `POST /workspaces`

**认证**: Bearer Token

**请求体**:
```json
{
  "name": "销售分析项目",
  "description": "用于季度销售数据分析的工作空间",
  "permissions": {
    "users": [
      {
        "user_id": "user_uuid",
        "role": "admin"
      }
    ],
    "agents": [
      {
        "agent_id": "agent_uuid",
        "access_level": "read_write"
      }
    ]
  },
  "tools": ["data_analyzer", "report_generator"],
  "metadata": {
    "project": "季度分析",
    "department": "销售部"
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "workspace_uuid",
    "name": "销售分析项目",
    "description": "用于季度销售数据分析的工作空间",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "permissions": {
      "users": [
        {
          "user_id": "user_uuid",
          "role": "admin"
        }
      ],
      "agents": [
        {
          "agent_id": "agent_uuid",
          "access_level": "read_write"
        }
      ]
    },
    "tools": ["data_analyzer", "report_generator"]
  },
  "message": "工作空间创建成功"
}
```

### 5.2 获取工作空间上下文

**端点**: `GET /workspaces/{workspace_id}/context`

**认证**: Bearer Token

**响应**:
```json
{
  "success": true,
  "data": {
    "workspace_id": "workspace_uuid",
    "context": {
      "project_info": {
        "name": "销售分析项目",
        "description": "季度销售数据分析",
        "timeline": {
          "start_date": "2024-01-01",
          "end_date": "2024-03-31"
        }
      },
      "shared_knowledge": [
        {
          "id": "knowledge_1",
          "type": "document",
          "title": "销售数据规范",
          "content": "销售数据应包含日期、产品、数量、金额等字段...",
          "created_at": "2024-01-01T00:00:00Z"
        }
      ],
      "recent_activities": [
        {
          "timestamp": "2024-01-01T00:00:00Z",
          "agent": "数据分析智能体",
          "action": "开始分析销售数据",
          "details": "处理了1000条销售记录"
        }
      ]
    },
    "documents": [
      {
        "id": "doc_uuid",
        "name": "sales_data.csv",
        "type": "csv",
        "size": 102400,
        "uploaded_at": "2024-01-01T00:00:00Z"
      }
    ],
    "tools": [
      {
        "id": "tool_uuid",
        "name": "data_analyzer",
        "description": "数据分析工具",
        "endpoint": "https://tools.example.com/analyze",
        "parameters": {
          "supported_formats": ["csv", "json"]
        }
      }
    ]
  },
  "message": "获取工作空间上下文成功"
}
```

### 5.3 上传文档到工作空间

**端点**: `POST /workspaces/{workspace_id}/documents`

**认证**: Bearer Token

**请求体** (multipart/form-data):
- `file`: 文件内容
- `metadata` (可选): JSON字符串

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "doc_uuid",
    "name": "sales_data.csv",
    "type": "csv",
    "size": 102400,
    "url": "https://storage.example.com/documents/doc_uuid",
    "uploaded_at": "2024-01-01T00:00:00Z",
    "metadata": {
      "description": "季度销售数据",
      "source": "CRM系统"
    }
  },
  "message": "文档上传成功"
}
```

## 6. 编排器API

### 6.1 创建工作流

**端点**: `POST /workflows`

**认证**: Bearer Token

**请求体**:
```json
{
  "name": "销售报告生成流程",
  "description": "自动化销售数据分析和报告生成流程",
  "definition": {
    "version": "1.0",
    "steps": [
      {
        "id": "step_1",
        "name": "数据收集",
        "type": "TASK",
        "agent_capabilities": ["data_collection"],
        "parameters": {
          "data_sources": ["crm", "erp"]
        },
        "next_steps": ["step_2"]
      },
      {
        "id": "step_2",
        "name": "数据分析",
        "type": "TASK",
        "agent_capabilities": ["data_analysis"],
        "parameters": {
          "analysis_type": "trend_analysis"
        },
        "next_steps": ["step_3"]
      },
      {
        "id": "step_3",
        "name": "报告生成",
        "type": "TASK",
        "agent_capabilities": ["report_writing"],
        "parameters": {
          "template": "executive_summary"
        },
        "next_steps": []
      }
    ],
    "error_handling": {
      "retry_policy": {
        "max_attempts": 3,
        "backoff_multiplier": 2
      },
      "fallback_actions": [
        {
          "condition": "step_failed",
          "action": "notify_admin"
        }
      ]
    }
  },
  "metadata": {
    "category": "business_intelligence",
    "version": "1.0.0"
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "workflow_uuid",
    "name": "销售报告生成流程",
    "status": "DRAFT",
    "created_at": "2024-01-01T00:00:00Z",
    "definition": {
      "version": "1.0",
      "steps": 3
    }
  },
  "message": "工作流创建成功"
}
```

### 6.2 执行工作流

**端点**: `POST /workflows/{workflow_id}/execute`

**认证**: Bearer Token

**请求体**:
```json
{
  "input": {
    "data_sources": ["crm", "erp"],
    "time_range": {
      "start": "2024-01-01",
      "end": "2024-03-31"
    }
  },
  "workspace_id": "workspace_uuid",
  "priority": "MEDIUM",
  "metadata": {
    "execution_context": "季度报告",
    "notify_on_completion": true
  }
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "execution_id": "execution_uuid",
    "workflow_id": "workflow_uuid",
    "status": "RUNNING",
    "started_at": "2024-01-01T00:00:00Z",
    "current_step": "step_1",
    "progress": 0
  },
  "message": "工作流执行开始"
}
```

### 6.3 获取工作流执行状态

**端点**: `GET /workflows/{workflow_id}/executions/{execution_id}`

**认证**: Bearer Token

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "execution_uuid",
    "workflow_id": "workflow_uuid",
    "status": "RUNNING",
    "started_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:01:00Z",
    "current_step": "step_2",
    "progress": 33,
    "steps": [
      {
        "id": "step_1",
        "name": "数据收集",
        "status": "COMPLETED",
        "started_at": "2024-01-01T00:00:00Z",
        "completed_at": "2024-01-01T00:00:30Z",
        "assigned_agent": "agent_1",
        "result": {
          "collected_records": 1500
        }
      },
      {
        "id": "step_2",
        "name": "数据分析",
        "status": "IN_PROGRESS",
        "started_at": "2024-01-01T00:00:35Z",
        "assigned_agent": "agent_2",
        "progress": 50
      },
      {
        "id": "step_3",
        "name": "报告生成",
        "status": "PENDING"
      }
    ],
    "metrics": {
      "total_steps": 3,
      "completed_steps": 1,
      "failed_steps": 0,
      "estimated_remaining_time": 120
    }
  },
  "message": "获取执行状态成功"
}
```

## 7. 监控和统计API

### 7.1 系统状态

**端点**: `GET /monitoring/system`

**认证**: Bearer Token (需要管理员权限)

**响应**:
```json
{
  "success": true,
  "data": {
    "system": {
      "status": "HEALTHY",
      "uptime": 86400,
      "version": "1.0.0"
    },
    "services": {
      "api_gateway": {
        "status": "HEALTHY",
        "response_time": 45,
        "throughput": 120
      },
      "task_service": {
        "status": "HEALTHY",
        "active_tasks": 25,
        "queue_size": 8
      },
      "agent_service": {
        "status": "HEALTHY",
        "registered_agents": 15,
        "online_agents": 12
      }
    },
    "resources": {
      "cpu_usage": 45.2,
      "memory_usage": 67.8,
      "disk_usage": 32.1
    }
  },
  "message": "获取系统状态成功"
}
```

### 7.2 任务统计

**端点**: `GET /monitoring/tasks/stats`

**认证**: Bearer Token

**查询参数**:
- `time_range` (可选): 时间范围，如"7d", "30d"
- `workspace_id` (可选): 工作空间ID

**响应**:
```json
{
  "success": true,
  "data": {
    "time_range": {
      "start": "2024-01-01T00:00:00Z",
      "end": "2024-01-08T00:00:00Z"
    },
    "overview": {
      "total_tasks": 150,
      "completed_tasks": 120,
      "failed_tasks": 5,
      "in_progress_tasks": 25,
      "success_rate": 80.0
    },
    "by_status": {
      "PENDING": 10,
      "IN_PROGRESS": 25,
      "COMPLETED": 120,
      "FAILED": 5
    },
    "by_priority": {
      "LOW": 30,
      "MEDIUM": 80,
      "HIGH": 35,
      "URGENT": 5
    },
    "performance": {
      "average_execution_time": 180,
      "average_queue_time": 15,
      "peak_concurrent_tasks": 42
    }
  },
  "message": "获取任务统计成功"
}
```

---

*此API规范文档定义了系统的所有接口，为前后端开发提供统一的标准。所有API都遵循RESTful设计原则，使用一致的错误处理和响应格式。*