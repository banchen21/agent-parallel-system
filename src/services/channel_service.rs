use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::channel::{ChannelConfig, ChannelUser, CreateChannelConfigRequest, UpdateChannelConfigRequest},
};

/// 通道服务
#[derive(Clone)]
pub struct ChannelService {
    db_pool: PgPool,
}

impl ChannelService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// 创建通道配置
    pub async fn create_channel_config(
        &self,
        request: CreateChannelConfigRequest,
    ) -> Result<ChannelConfig, AppError> {
        let config = sqlx::query_as!(
            ChannelConfig,
            r#"
            INSERT INTO channel_configs (channel_type, name, description, config)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            request.channel_type,
            request.name,
            request.description,
            request.config
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 获取通道配置
    pub async fn get_channel_config(&self, config_id: Uuid) -> Result<Option<ChannelConfig>, AppError> {
        let config = sqlx::query_as!(
            ChannelConfig,
            "SELECT * FROM channel_configs WHERE id = $1",
            config_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 获取所有活跃通道配置
    pub async fn get_active_channels(&self) -> Result<Vec<ChannelConfig>, AppError> {
        let configs = sqlx::query_as!(
            ChannelConfig,
            "SELECT * FROM channel_configs WHERE is_active = true ORDER BY created_at DESC"
        )
        .fetch_all(&self.db_pool)
        .await?;

        Ok(configs)
    }

    /// 获取指定类型的通道配置
    pub async fn get_channel_by_type(&self, channel_type: &str) -> Result<Option<ChannelConfig>, AppError> {
        let config = sqlx::query_as!(
            ChannelConfig,
            "SELECT * FROM channel_configs WHERE channel_type = $1 AND is_active = true LIMIT 1",
            channel_type
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 更新通道配置
    pub async fn update_channel_config(
        &self,
        config_id: Uuid,
        request: UpdateChannelConfigRequest,
    ) -> Result<ChannelConfig, AppError> {
        let config = sqlx::query_as!(
            ChannelConfig,
            r#"
            UPDATE channel_configs 
            SET 
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                config = COALESCE($4, config),
                is_active = COALESCE($5, is_active),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
            config_id,
            request.name,
            request.description,
            request.config,
            request.is_active
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(config)
    }

    /// 创建或获取通道用户
    pub async fn get_or_create_channel_user(
        &self,
        channel_config_id: Uuid,
        channel_user_id: &str,
        channel_username: Option<&str>,
    ) -> Result<ChannelUser, AppError> {
        // 尝试获取现有用户
        if let Ok(Some(user)) = sqlx::query_as!(
            ChannelUser,
            "SELECT * FROM channel_users WHERE channel_config_id = $1 AND channel_user_id = $2",
            channel_config_id,
            channel_user_id
        )
        .fetch_optional(&self.db_pool)
        .await
        {
            return Ok(user);
        }

        // 创建新用户
        let user = sqlx::query_as!(
            ChannelUser,
            r#"
            INSERT INTO channel_users (channel_config_id, channel_user_id, channel_username, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            channel_config_id,
            channel_user_id,
            channel_username,
            serde_json::json!({})
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(user)
    }

    /// 获取通道用户
    pub async fn get_channel_user(&self, user_id: Uuid) -> Result<Option<ChannelUser>, AppError> {
        let user = sqlx::query_as!(
            ChannelUser,
            "SELECT * FROM channel_users WHERE id = $1",
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(user)
    }

    /// 绑定通道用户到系统用户
    pub async fn bind_channel_user(
        &self,
        channel_user_id: Uuid,
        system_user_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE channel_users SET user_id = $1, updated_at = NOW() WHERE id = $2",
            system_user_id,
            channel_user_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}
