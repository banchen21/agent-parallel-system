use actix::Addr;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use uuid::Uuid;

use crate::agsnets::actor_agents_manage::{
    AgentManagerActor, CreateAgent, DeleteAgent, GetAgentInfo, ListAgents, StartAgent,
    StopAgent,
};

#[derive(Debug, Serialize)]
pub struct ProviderModelOption {
    pub provider: String,
    pub default_model: String,
    pub base_url: String,
    pub has_token: bool,
    pub recommended_models: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderModelOptionsResponse {
    pub default_provider: String,
    pub providers: Vec<ProviderModelOption>,
}

#[derive(Debug, Deserialize)]
pub struct SaveProviderConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub base_url: String,
}

// 创建 agent
#[post("/agent")]
pub async fn create_agent_handler(
    create_agent: web::Json<CreateAgent>,
    agsnets: web::Data<Addr<AgentManagerActor>>,
) -> impl Responder {
    let create_agent = create_agent.into_inner();
    match agsnets.send(create_agent.clone()).await {
        Ok(s) => match s {
            Ok(_info) => HttpResponse::Ok().json(_info),
            Err(e) => HttpResponse::BadRequest().body(e.to_string()),
        },
        Err(_e) => HttpResponse::InternalServerError().body(_e.to_string()),
    }
}

// 获取 agent 列表
#[get("/agent")]
pub async fn list_agents_handler(
    agsnets: web::Data<Addr<AgentManagerActor>>,
    req: HttpRequest,
) -> impl Responder {
    // 从请求上下文中获取当前用户名（由 Auth middleware 放入 extensions）
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match agsnets.send(ListAgents { user_name }).await {
        Ok(Ok(list)) => HttpResponse::Ok().json(list),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/agent/{agent_id}")]
pub async fn get_agent_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManagerActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match agsnets
        .send(GetAgentInfo {
            agent_id: path.into_inner(),
            user_name,
        })
        .await
    {
        Ok(Ok(agent)) => HttpResponse::Ok().json(agent),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/agents/statuses")]
pub async fn list_agent_statuses_handler(
    agsnets: web::Data<Addr<AgentManagerActor>>,
    req: HttpRequest,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match agsnets.send(ListAgents { user_name }).await {
        Ok(Ok(list)) => {
            let statuses = list
                .into_iter()
                .map(|agent| {
                    let status_label = match agent.status.as_str() {
                        "starting" => "启动中",
                        "working" => "执行中",
                        "idle" => "空闲中",
                        "running" => "运行中",
                        "stopping" => "停止中",
                        "stopped" => "已停止",
                        _ => "未知",
                    };
                    serde_json::json!({
                        "agent_id": agent.id,
                        "name": agent.name,
                        "status": agent.status,
                        "status_label": status_label,
                    })
                })
                .collect::<Vec<_>>();
            HttpResponse::Ok().json(statuses)
        }
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/agent/provider-options")]
pub async fn get_agent_provider_options_handler(
    config: web::Data<crate::core::config::Settings>,
) -> impl Responder {
    let providers = config
        .providers
        .iter()
        .map(|p| ProviderModelOption {
            provider: p.name.clone(),
            default_model: p.default_model.clone(),
            base_url: p.base_url.clone(),
            has_token: !p.api_key.trim().is_empty(),
            recommended_models: vec![p.default_model.clone()],
        })
        .collect::<Vec<_>>();

    HttpResponse::Ok().json(ProviderModelOptionsResponse {
        default_provider: config.llm.default_provider.clone(),
        providers,
    })
}

#[post("/agent/provider-options")]
pub async fn save_agent_provider_options_handler(
    req: web::Json<SaveProviderConfigRequest>,
) -> impl Responder {
    let provider = req.provider.trim();
    let model = req.model.trim();
    if provider.is_empty() || model.is_empty() {
        return HttpResponse::BadRequest().body("provider 和 model 不能为空");
    }

    let config_path = "config/default.toml";
    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("读取配置文件失败: {}", e));
        }
    };

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // 更新 [llm] 下的 default_provider
    let mut llm_start: Option<usize> = None;
    for (idx, line) in lines.iter().enumerate() {
        if line.trim() == "[llm]" {
            llm_start = Some(idx);
            break;
        }
    }
    if let Some(start) = llm_start {
        let mut replaced = false;
        let mut i = start + 1;
        while i < lines.len() {
            let t = lines[i].trim();
            if t.starts_with('[') && t.ends_with(']') {
                break;
            }
            if t.starts_with("default_provider") {
                lines[i] = format!("default_provider = \"{}\"", provider);
                replaced = true;
                break;
            }
            i += 1;
        }
        if !replaced {
            lines.insert(start + 1, format!("default_provider = \"{}\"", provider));
        }
    }

    // 更新或新增 [[providers]] 块
    let mut found_block_start: Option<usize> = None;
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i].trim() == "[[providers]]" {
            let block_start = i;
            let mut j = i + 1;
            while j < lines.len() {
                let t = lines[j].trim();
                if t == "[[providers]]" || (t.starts_with('[') && t.ends_with(']') && t != "[[providers]]") {
                    break;
                }
                j += 1;
            }
            let mut name_match = false;
            for k in (block_start + 1)..j {
                let t = lines[k].trim();
                if t.starts_with("name") {
                    let expect = format!("name = \"{}\"", provider);
                    let expect2 = format!("name         = \"{}\"", provider);
                    if t == expect || t == expect2 {
                        name_match = true;
                    }
                    break;
                }
            }
            if name_match {
                found_block_start = Some(block_start);

                let mut has_name = false;
                let mut has_base_url = false;
                let mut has_default_model = false;
                let mut has_api_key = false;

                for k in (block_start + 1)..j {
                    let t = lines[k].trim();
                    if t.starts_with("name") {
                        lines[k] = format!("name         = \"{}\"", provider);
                        has_name = true;
                    } else if t.starts_with("base_url") {
                        let base_url = if req.base_url.trim().is_empty() {
                            String::from("https://api.openai.com/v1")
                        } else {
                            req.base_url.trim().to_string()
                        };
                        lines[k] = format!("base_url     = \"{}\"", base_url);
                        has_base_url = true;
                    } else if t.starts_with("default_model") {
                        lines[k] = format!("default_model = \"{}\"", model);
                        has_default_model = true;
                    } else if t.starts_with("api_key") {
                        lines[k] = format!("api_key      = \"{}\"", req.token.trim());
                        has_api_key = true;
                    }
                }

                let mut insert_pos = j;
                if !has_name {
                    lines.insert(insert_pos, format!("name         = \"{}\"", provider));
                    insert_pos += 1;
                }
                if !has_base_url {
                    let base_url = if req.base_url.trim().is_empty() {
                        String::from("https://api.openai.com/v1")
                    } else {
                        req.base_url.trim().to_string()
                    };
                    lines.insert(insert_pos, format!("base_url     = \"{}\"", base_url));
                    insert_pos += 1;
                }
                if !has_default_model {
                    lines.insert(insert_pos, format!("default_model = \"{}\"", model));
                    insert_pos += 1;
                }
                if !has_api_key {
                    lines.insert(insert_pos, format!("api_key      = \"{}\"", req.token.trim()));
                }
                break;
            }

            i = j;
            continue;
        }
        i += 1;
    }

    if found_block_start.is_none() {
        if !lines.is_empty() && !lines.last().unwrap_or(&String::new()).is_empty() {
            lines.push(String::new());
        }
        lines.push("[[providers]]".to_string());
        lines.push(format!("name         = \"{}\"", provider));
        lines.push(format!(
            "base_url     = \"{}\"",
            if req.base_url.trim().is_empty() {
                "https://api.openai.com/v1"
            } else {
                req.base_url.trim()
            }
        ));
        lines.push(format!("default_model = \"{}\"", model));
        lines.push(format!("api_key      = \"{}\"", req.token.trim()));
    }

    let new_content = lines.join("\n") + "\n";
    if let Err(e) = fs::write(config_path, new_content) {
        return HttpResponse::InternalServerError().body(format!("写入配置文件失败: {}", e));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "配置已写入 config/default.toml，重启服务后生效"
    }))
}

#[post("/agent/{agent_id}/start")]
pub async fn start_agent_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManagerActor>>,
) -> impl Responder {
    let agent_id = path.into_inner();
    match agsnets.send(StartAgent { agent_id }).await {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[post("/agent/{agent_id}/stop")]
pub async fn stop_agent_handler(
    path: web::Path<Uuid>,
    agsnets: web::Data<Addr<AgentManagerActor>>,
) -> impl Responder {
    let agent_id = path.into_inner();
    match agsnets.send(StopAgent { agent_id }).await {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/agent/{agent_id}")]
pub async fn delete_agent_handler(
    path: web::Path<Uuid>,
    req: HttpRequest,
    agsnets: web::Data<Addr<AgentManagerActor>>,
) -> impl Responder {
    let user_name = match crate::utils::handler_util::get_user_name(&req) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    let agent_id = path.into_inner();
    match agsnets.send(DeleteAgent { agent_id, user_name }).await {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Ok(Err(e)) => HttpResponse::BadRequest().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
