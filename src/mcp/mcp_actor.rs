use actix::{Actor, ActorFutureExt, Context, Handler, Message, ResponseActFuture};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

use crate::mcp::model::{McpConfig, McpConfigList, McpError};

/// MCP 配置管理 Actor
pub struct McpManagerActor {
    /// .mcps 目录路径
    mcps_dir: PathBuf,
    /// 内存中的 MCP 配置列表
    mcp_list: McpConfigList,
}

impl McpManagerActor {
    pub fn new() -> Self {
        let mcps_dir = PathBuf::from(".mcps");
        
        // 确保 .mcps 目录存在
        if let Err(e) = fs::create_dir_all(&mcps_dir) {
            error!("创建 .mcps 目录失败: {}", e);
        } else {
            info!("已确保 .mcps 目录存在");
        }

        // 从 .mcps 目录加载所有配置
        let mcp_list = Self::load_all_configs(&mcps_dir).unwrap_or_else(|e| {
            error!("加载 MCP 配置失败: {}", e);
            McpConfigList::new()
        });

        Self {
            mcps_dir,
            mcp_list,
        }
    }

    /// 从 .mcps 目录加载所有配置
    fn load_all_configs(mcps_dir: &PathBuf) -> Result<McpConfigList, McpError> {
        let mut config_list = McpConfigList::new();
        
        if !mcps_dir.exists() {
            return Ok(config_list);
        }

        let entries = fs::read_dir(mcps_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && path.file_name().and_then(|s| s.to_str()) != Some("README.md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = serde_json::from_str::<McpConfig>(&content) {
                        let _ = config_list.add(config);
                    }
                }
            }
        }

        Ok(config_list)
    }


    /// 读取单个 MCP 配置文件
    fn read_mcp_file(&self, name: &str) -> Result<McpConfig, McpError> {
        let file_path = self.mcps_dir.join(format!("{}.json", name));
        if !file_path.exists() {
            return Err(McpError::NotFound(name.to_string()));
        }
        let content = fs::read_to_string(&file_path)?;
        let config: McpConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 写入单个 MCP 配置文件
    fn write_mcp_file(&self, config: &McpConfig) -> Result<(), McpError> {
        // 确保 .mcps 目录存在
        fs::create_dir_all(&self.mcps_dir)?;
        
        let file_path = self.mcps_dir.join(format!("{}.json", config.name));
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&file_path, content)?;
        Ok(())
    }

    /// 删除单个 MCP 配置文件
    fn delete_mcp_file(&self, name: &str) -> Result<(), McpError> {
        let file_path = self.mcps_dir.join(format!("{}.json", name));
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }
        Ok(())
    }
}

impl Actor for McpManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("McpManagerActor 已启动");
        
        // 确保目录存在
        if let Err(e) = fs::create_dir_all(&self.mcps_dir) {
            error!("创建 .mcps 目录失败: {}", e);
        }
    }
}

// ==================== 消息定义 ====================

/// 添加 MCP 配置
#[derive(Message, Clone, Deserialize, Serialize)]
#[rtype(result = "Result<McpConfig, McpError>")]
pub struct AddMcpConfig {
    pub config: McpConfig,
}

impl Handler<AddMcpConfig> for McpManagerActor {
    type Result = ResponseActFuture<Self, Result<McpConfig, McpError>>;

    fn handle(&mut self, msg: AddMcpConfig, _ctx: &mut Self::Context) -> Self::Result {
        let config = msg.config.clone();
        let mcps_dir = self.mcps_dir.clone();

        let fut = async move {
            // 验证配置
            config.validate()?;

            // 检查是否已存在
            let file_path = mcps_dir.join(format!("{}.json", config.name));
            if file_path.exists() {
                return Err(McpError::AlreadyExists(config.name.clone()));
            }

            // 写入 .mcps/{name}.json
            fs::create_dir_all(&mcps_dir)?;
            let content = serde_json::to_string_pretty(&config)?;
            fs::write(&file_path, content)?;

            info!("成功添加 MCP 配置: {}", config.name);
            Ok(config)
        };

        Box::pin(actix::fut::wrap_future(fut).map(
            |res: Result<McpConfig, McpError>, actor: &mut Self, _ctx| {
                if let Ok(ref config) = res {
                    // 更新内存中的配置列表
                    let _ = actor.mcp_list.add(config.clone());
                }
                res
            },
        ))
    }
}

/// 删除 MCP 配置
#[derive(Message, Clone, Deserialize, Serialize)]
#[rtype(result = "Result<(), McpError>")]
pub struct DeleteMcpConfig {
    pub name: String,
}

impl Handler<DeleteMcpConfig> for McpManagerActor {
    type Result = ResponseActFuture<Self, Result<(), McpError>>;

    fn handle(&mut self, msg: DeleteMcpConfig, _ctx: &mut Self::Context) -> Self::Result {
        let name = msg.name.clone();
        let mcps_dir = self.mcps_dir.clone();

        let fut = async move {
            // 删除 .mcps/{name}.json
            let file_path = mcps_dir.join(format!("{}.json", name));
            if !file_path.exists() {
                return Err(McpError::NotFound(name.clone()));
            }
            fs::remove_file(&file_path)?;

            info!("成功删除 MCP 配置: {}", name);
            Ok(name)
        };

        Box::pin(actix::fut::wrap_future(fut).map(
            |res: Result<String, McpError>, actor: &mut Self, _ctx| {
                if let Ok(ref name) = res {
                    // 从内存中移除配置
                    let _ = actor.mcp_list.remove(name);
                }
                res.map(|_| ())
            },
        ))
    }
}

/// 查询单个 MCP 配置
#[derive(Message, Clone, Deserialize, Serialize)]
#[rtype(result = "Result<McpConfig, McpError>")]
pub struct GetMcpConfig {
    pub name: String,
}
impl Handler<GetMcpConfig> for McpManagerActor {
    type Result = ResponseActFuture<Self, Result<McpConfig, McpError>>;

    fn handle(&mut self, msg: GetMcpConfig, _ctx: &mut Self::Context) -> Self::Result {
        // 直接从内存中获取
        if let Some(config) = self.mcp_list.get(&msg.name) {
            let config = config.clone();
            Box::pin(actix::fut::wrap_future(async move { Ok(config) }).map(
                |res: Result<McpConfig, McpError>, _actor: &mut Self, _ctx| res,
            ))
        } else {
            let error = McpError::NotFound(msg.name.clone());
            Box::pin(actix::fut::wrap_future(async move { Err(error) }).map(
                |res: Result<McpConfig, McpError>, _actor: &mut Self, _ctx| res,
            ))
        }
    }
}

/// 查询所有 MCP 配置
#[derive(Message, Clone, Deserialize, Serialize)]
#[rtype(result = "Result<Vec<McpConfig>, McpError>")]
pub struct ListMcpConfigs;

impl Handler<ListMcpConfigs> for McpManagerActor {
    type Result = ResponseActFuture<Self, Result<Vec<McpConfig>, McpError>>;

    fn handle(&mut self, _msg: ListMcpConfigs, _ctx: &mut Self::Context) -> Self::Result {
        // 直接从内存中获取所有配置
        let configs = self.mcp_list.list().into_iter().cloned().collect();
        
        Box::pin(actix::fut::wrap_future(async move { Ok(configs) }).map(
            |res: Result<Vec<McpConfig>, McpError>, _actor: &mut Self, _ctx| res,
        ))
    }
}
