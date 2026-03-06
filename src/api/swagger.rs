use axum::{
    response::Html,
    routing::get,
    Router,
};
use crate::AppState;

/// Swagger UI 路由
pub fn swagger_routes() -> Router<AppState> {
    Router::new()
        .route("/swagger-ui", get(swagger_ui))
        .route("/swagger-ui/", get(swagger_ui))
        .route("/openapi.json", get(openapi_spec))
}

/// Swagger UI HTML
async fn swagger_ui() -> Html<&'static str> {
    Html(include_str!("swagger-ui.html"))
}

/// OpenAPI 规范 JSON
async fn openapi_spec() -> axum::Json<serde_json::Value> {
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Agent Parallel System API",
            "description": "智能体并行系统 API 文档",
            "version": "1.0.0",
            "contact": {
                "name": "Agent Parallel System",
                "url": "https://github.com/agent-parallel-system"
            }
        },
        "servers": [
            {
                "url": "/api/v1",
                "description": "API v1"
            }
        ],
        "components": {
            "securitySchemes": {
                "BearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT"
                }
            }
        },
        "security": [
            {
                "BearerAuth": []
            }
        ],
        "paths": {
            "/health": {
                "get": {
                    "tags": ["Health"],
                    "summary": "健康检查",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "服务器正常"
                        }
                    }
                }
            },
            "/ready": {
                "get": {
                    "tags": ["Health"],
                    "summary": "就绪检查",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "服务器已就绪"
                        }
                    }
                }
            },
            "/auth/register": {
                "post": {
                    "tags": ["Authentication"],
                    "summary": "用户注册",
                    "security": [],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["username", "email", "password"],
                                    "properties": {
                                        "username": {"type": "string"},
                                        "email": {"type": "string", "format": "email"},
                                        "password": {"type": "string"},
                                        "first_name": {"type": "string"},
                                        "last_name": {"type": "string"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "注册成功"},
                        "400": {"description": "请求参数错误"}
                    }
                }
            },
            "/auth/login": {
                "post": {
                    "tags": ["Authentication"],
                    "summary": "用户登录",
                    "security": [],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["username", "password"],
                                    "properties": {
                                        "username": {"type": "string"},
                                        "password": {"type": "string"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "登录成功"},
                        "401": {"description": "认证失败"}
                    }
                }
            },
            "/auth/refresh": {
                "post": {
                    "tags": ["Authentication"],
                    "summary": "刷新令牌",
                    "security": [],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["refresh_token"],
                                    "properties": {
                                        "refresh_token": {"type": "string"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "令牌刷新成功"},
                        "401": {"description": "刷新令牌无效"}
                    }
                }
            },
            "/auth/logout": {
                "post": {
                    "tags": ["Authentication"],
                    "summary": "用户登出",
                    "responses": {
                        "200": {"description": "登出成功"}
                    }
                }
            },
            "/auth/me": {
                "get": {
                    "tags": ["Authentication"],
                    "summary": "获取当前用户信息",
                    "responses": {
                        "200": {"description": "获取成功"},
                        "401": {"description": "未认证"}
                    }
                }
            },
            "/tasks": {
                "get": {
                    "tags": ["Tasks"],
                    "summary": "获取任务列表",
                    "parameters": [
                        {"name": "page", "in": "query", "schema": {"type": "integer"}},
                        {"name": "page_size", "in": "query", "schema": {"type": "integer"}}
                    ],
                    "responses": {
                        "200": {"description": "获取成功"}
                    }
                },
                "post": {
                    "tags": ["Tasks"],
                    "summary": "创建任务",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["title"],
                                    "properties": {
                                        "title": {"type": "string"},
                                        "description": {"type": "string"},
                                        "priority": {"type": "string", "enum": ["low", "medium", "high", "critical"]},
                                        "workspace_id": {"type": "string", "format": "uuid"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {"description": "创建成功"}
                    }
                }
            },
            "/agents": {
                "get": {
                    "tags": ["Agents"],
                    "summary": "获取可用智能体列表",
                    "parameters": [
                        {"name": "capabilities", "in": "query", "schema": {"type": "string"}}
                    ],
                    "responses": {
                        "200": {"description": "获取成功"}
                    }
                },
                "post": {
                    "tags": ["Agents"],
                    "summary": "注册智能体",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["name"],
                                    "properties": {
                                        "name": {"type": "string"},
                                        "description": {"type": "string"},
                                        "capabilities": {"type": "object"},
                                        "endpoints": {"type": "object"},
                                        "limits": {"type": "object"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {"description": "注册成功"}
                    }
                }
            },
            "/workspaces": {
                "get": {
                    "tags": ["Workspaces"],
                    "summary": "获取工作空间列表",
                    "responses": {
                        "200": {"description": "获取成功"}
                    }
                },
                "post": {
                    "tags": ["Workspaces"],
                    "summary": "创建工作空间",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["name"],
                                    "properties": {
                                        "name": {"type": "string"},
                                        "description": {"type": "string"},
                                        "is_public": {"type": "boolean"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {"description": "创建成功"}
                    }
                }
            },
            "/workflows": {
                "get": {
                    "tags": ["Workflows"],
                    "summary": "获取工作流列表",
                    "responses": {
                        "200": {"description": "获取成功"}
                    }
                },
                "post": {
                    "tags": ["Workflows"],
                    "summary": "创建工作流",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["name", "dag"],
                                    "properties": {
                                        "name": {"type": "string"},
                                        "description": {"type": "string"},
                                        "dag": {"type": "object"},
                                        "workspace_id": {"type": "string", "format": "uuid"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {"description": "创建成功"}
                    }
                }
            },
            "/messages": {
                "get": {
                    "tags": ["Messages"],
                    "summary": "获取消息列表",
                    "responses": {
                        "200": {"description": "获取成功"}
                    }
                },
                "post": {
                    "tags": ["Messages"],
                    "summary": "发送消息",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["message_type", "content"],
                                    "properties": {
                                        "message_type": {"type": "string"},
                                        "content": {"type": "string"},
                                        "recipient_id": {"type": "string", "format": "uuid"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {"description": "发送成功"}
                    }
                }
            }
        }
    });
    axum::Json(spec)
}
