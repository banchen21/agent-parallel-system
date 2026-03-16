use crate::agsnets::actor_agents::{AgentManagerActor, CreateAgent};
use crate::api::auth_utils::{generate_tokens, validate_token};
use crate::api::redis_actor::{RedisActor, SaveRefreshToken, VerifyAndConsumeToken};
use crate::api::user::actor_user::{CreateUser, GetUserByUsername, UserManagerActor};
use crate::api::user::model::{AuthResponse, LoginRequest, RegisterRequest};
use crate::workspace::model::AgentKind;
use actix::Addr;
use actix_web::{HttpMessage as _, HttpRequest, HttpResponse, Responder, post, web};
use bcrypt::{DEFAULT_COST, hash, verify};
use reqwest::header::AUTHORIZATION;
use tracing::{debug, error, info};

/// 1. 注册接口
#[post("/register")]
pub async fn register(
    user_manager: web::Data<Addr<UserManagerActor>>,
    workspace_manage: web::Data<Addr<crate::workspace::workspace_actor::WorkspaceManageActor>>,
    agent_manager: web::Data<Addr<AgentManagerActor>>,
    req: web::Json<RegisterRequest>,
) -> impl Responder {
    // 密码加密
    let hashed_password = match hash(&req.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("密码加密失败"),
    };

    // 发送消息给 Actor 执行数据库插入
    let res = user_manager
        .send(CreateUser {
            username: req.username.clone(),
            password_hash: hashed_password,
            email: req.email.clone(),
        })
        .await;

    match res {
        Ok(Ok(user)) => {
            // 尝试为新用户创建一个默认工作区（不阻塞注册流程）
            let owner = user.username.clone();
            let mut name: String = owner.chars().filter(|c| c.is_alphabetic()).collect();
            if name.is_empty() {
                name = format!("workspace{}", user.id);
            }
            let ws_actor = workspace_manage.get_ref().clone();
            let workspace_name = name.clone() + "_default";
            let create = crate::workspace::workspace_actor::CreateWorkspace {
                name: workspace_name.clone(),
                description: Some("默认工作区".to_string()),
                owner_username: owner.clone(),
            };
            // spawn 不阻塞主流程：先创建工作区，成功后尝试创建一个默认执行型 Agent
            let agent_addr = agent_manager.get_ref().clone();
            tokio::spawn(async move {
                match ws_actor.send(create).await {
                    Ok(Ok(_)) => {
                        tracing::info!("Created default workspace for {}: {}", owner, workspace_name);
                        // 创建默认 Agent（executor）
                        let agent_req = CreateAgent {
                            user_name: owner.clone(),
                            name: format!("executor-{}", owner.clone()),
                            kind: AgentKind::General,
                            workspace_name: workspace_name.clone(),
                            mcp_list: vec![],
                        };

                        match agent_addr.send(agent_req).await {
                            Ok(Ok(agent_info)) => tracing::info!(
                                "Created default agent for {}: {}",
                                owner,
                                agent_info.name
                            ),
                            Ok(Err(e)) => tracing::warn!("Create default agent failed: {:?}", e),
                            Err(e) => tracing::warn!("Agent manager mailbox error: {:?}", e),
                        }
                    }
                    Ok(Err(e)) => tracing::warn!("Create default workspace failed: {:?}", e),
                    Err(e) => tracing::warn!("Workspace actor mailbox error: {:?}", e),
                }
            });

            HttpResponse::Ok().json(user)
        }
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[post("/login")]
pub async fn login(
    user_manager: web::Data<Addr<UserManagerActor>>,
    redis_actor: web::Data<Addr<RedisActor>>,
    req: web::Json<LoginRequest>,
) -> impl Responder {
    // 1. 从数据库校验用户是否存在
    let user_res = user_manager
        .send(GetUserByUsername {
            username: req.username.clone(),
        })
        .await;

    match user_res {
        Ok(Ok(Some(user))) => {
            // 2. 校验密码
            if verify(&req.password, &user.password_hash).unwrap_or(false) {
                // 3. 生成双 Token
                let (at, rt) = generate_tokens(&user.username);

                // 4. ✅ 将 Refresh Token 存入 Redis (不再存入数据库)
                let save_res = redis_actor
                    .send(SaveRefreshToken {
                        token: rt.clone(),
                        username: user.username.clone(),
                        expires_in_seconds: 7 * 24 * 3600, // 7天
                    })
                    .await;

                // 修改这一段
                match save_res {
                    Ok(Ok(_)) => {
                        info!("用户 {} 登录成功", user.username);
                        HttpResponse::Ok().json(AuthResponse {
                            access_token: at,
                            refresh_token: rt,
                        })
                    }
                    Ok(Err(e)) => {
                        error!("Redis 写入业务失败: {:?}", e);
                        HttpResponse::InternalServerError().body(format!("Redis 存储失败: {}", e))
                    }
                    Err(e) => {
                        error!("Redis Actor 通信失败 (Mailbox Error): {:?}", e);
                        HttpResponse::InternalServerError().body("内部服务通信失败")
                    }
                }
            } else {
                HttpResponse::Unauthorized().body("密码错误")
            }
        }
        Ok(Ok(None)) => HttpResponse::Unauthorized().body("用户不存在"),
        _ => HttpResponse::InternalServerError().body("服务器内部错误"),
    }
}

#[post("/refresh")]
pub async fn refresh(redis_actor: web::Data<Addr<RedisActor>>, req: HttpRequest) -> impl Responder {
    // 1. 从 Header 获取 Refresh Token
    let refresh_token = match req.headers().get(AUTHORIZATION) {
        Some(val) => match val.to_str() {
            Ok(s) => s.replace("Bearer ", ""),
            Err(_) => return HttpResponse::BadRequest().body("Invalid Token encoding"),
        },
        None => return HttpResponse::BadRequest().body("Missing Authorization header"),
    };

    // 2. 本地验证 JWT 签名
    let _claims = match validate_token(&refresh_token) {
        Ok(c) => {
            if c.token_type != "refresh" {
                return HttpResponse::BadRequest().body("Token 不是 Refresh Token");
            }
        }
        Err(e) => return HttpResponse::Unauthorized().body(format!("Invalid Token: {}", e)),
    };

    // 3. ✅ 在 Redis 中验证并消耗旧 Token (实现一次性旋转)
    let redis_res = match redis_actor
        .send(VerifyAndConsumeToken {
            token: refresh_token.clone(),
        })
        .await
    {
        Ok(res) => res,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match redis_res {
        Ok(Some(username)) => {
            // 4. 安全校验：确保当前 AT 用户与 RT 匹配
            if let Some(current_user) = req.extensions().get::<String>() {
                if current_user != &username {
                    return HttpResponse::Forbidden().body("User mismatch");
                }
            }

            // 5. 生成新的一对 Token
            let (access_token, refresh_token) = generate_tokens(&username);
            // 6. ✅ 将新 RT 存入 Redis
            let _ = redis_actor
                .send(SaveRefreshToken {
                    token: refresh_token.clone(),
                    username: username.clone(),
                    expires_in_seconds: 7 * 24 * 3600,
                })
                .await;

            HttpResponse::Ok().json(AuthResponse {
                access_token,
                refresh_token,
            })
        }
        Ok(None) => HttpResponse::Unauthorized().body("Token 已失效或已被使用"),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
