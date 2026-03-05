//! 清理工作器
//! 
//! 负责定期清理过期数据、临时文件和无效资源

use tokio::time::{self, Duration};
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use crate::core::errors::AppError;

/// 清理工作器配置
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    /// 清理间隔（秒）
    pub interval_seconds: u64,
    /// 是否启用任务清理
    pub enable_task_cleanup: bool,
    /// 任务保留天数
    pub task_retention_days: i32,
    /// 是否启用消息清理
    pub enable_message_cleanup: bool,
    /// 消息保留天数
    pub message_retention_days: i32,
    /// 是否启用临时文件清理
    pub enable_temp_file_cleanup: bool,
    /// 临时文件保留小时数
    pub temp_file_retention_hours: i32,
    /// 是否启用Redis缓存清理
    pub enable_redis_cleanup: bool,
    /// Redis缓存保留天数
    pub redis_retention_days: i32,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 3600, // 1小时
            enable_task_cleanup: true,
            task_retention_days: 30,
            enable_message_cleanup: true,
            message_retention_days: 7,
            enable_temp_file_cleanup: true,
            temp_file_retention_hours: 24,
            enable_redis_cleanup: true,
            redis_retention_days: 3,
        }
    }
}

/// 清理工作器
pub struct CleanupWorker {
    config: CleanupConfig,
    db_pool: PgPool,
    redis_conn: Option<ConnectionManager>,
}

impl CleanupWorker {
    /// 创建新的清理工作器
    pub fn new(
        config: CleanupConfig,
        db_pool: PgPool,
        redis_conn: Option<ConnectionManager>,
    ) -> Self {
        Self {
            config,
            db_pool,
            redis_conn,
        }
    }

    /// 启动清理工作器
    pub async fn start(self) -> Result<(), AppError> {
        let mut interval = time::interval(Duration::from_secs(self.config.interval_seconds));
        
        log::info!("清理工作器已启动，间隔: {}秒", self.config.interval_seconds);
        
        loop {
            interval.tick().await;
            
            match self.run_cleanup().await {
                Ok(_) => {
                    log::info!("清理任务执行完成");
                }
                Err(e) => {
                    log::error!("清理任务执行失败: {}", e);
                }
            }
        }
    }

    /// 执行清理任务
    async fn run_cleanup(&self) -> Result<(), AppError> {
        log::info!("开始执行清理任务...");
        
        let start_time = std::time::Instant::now();
        let mut cleanup_count = 0;
        
        // 清理过期任务
        if self.config.enable_task_cleanup {
            match self.cleanup_old_tasks().await {
                Ok(count) => {
                    cleanup_count += count;
                    log::info!("清理了 {} 个过期任务", count);
                }
                Err(e) => {
                    log::error!("清理过期任务失败: {}", e);
                }
            }
        }
        
        // 清理过期消息
        if self.config.enable_message_cleanup {
            match self.cleanup_old_messages().await {
                Ok(count) => {
                    cleanup_count += count;
                    log::info!("清理了 {} 个过期消息", count);
                }
                Err(e) => {
                    log::error!("清理过期消息失败: {}", e);
                }
            }
        }
        
        // 清理临时文件
        if self.config.enable_temp_file_cleanup {
            match self.cleanup_temp_files().await {
                Ok(count) => {
                    cleanup_count += count;
                    log::info!("清理了 {} 个临时文件", count);
                }
                Err(e) => {
                    log::error!("清理临时文件失败: {}", e);
                }
            }
        }
        
        // 清理Redis缓存
        if self.config.enable_redis_cleanup && self.redis_conn.is_some() {
            match self.cleanup_redis_cache().await {
                Ok(count) => {
                    cleanup_count += count;
                    log::info!("清理了 {} 个Redis缓存键", count);
                }
                Err(e) => {
                    log::error!("清理Redis缓存失败: {}", e);
                }
            }
        }
        
        let duration = start_time.elapsed();
        log::info!(
            "清理任务完成，总计清理 {} 个项目，耗时: {:?}",
            cleanup_count,
            duration
        );
        
        Ok(())
    }

    /// 清理过期任务
    async fn cleanup_old_tasks(&self) -> Result<u64, AppError> {
        let retention_days = self.config.task_retention_days;
        let cutoff_time = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let result = sqlx::query!(
            r#"
            DELETE FROM tasks
            WHERE 
                (status = 'completed' OR status = 'failed' OR status = 'cancelled')
                AND updated_at < $1
            "#,
            cutoff_time
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(result.rows_affected())
    }

    /// 清理过期消息
    async fn cleanup_old_messages(&self) -> Result<u64, AppError> {
        let retention_days = self.config.message_retention_days;
        let cutoff_time = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let result = sqlx::query!(
            r#"
            DELETE FROM messages
            WHERE created_at < $1
            "#,
            cutoff_time
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(result.rows_affected())
    }

    /// 清理临时文件
    async fn cleanup_temp_files(&self) -> Result<u64, AppError> {
        let retention_hours = self.config.temp_file_retention_hours;
        let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(retention_hours as i64);
        
        // 清理存储目录中的临时文件
        let storage_dir = std::path::Path::new("storage/temp");
        if !storage_dir.exists() {
            return Ok(0);
        }
        
        let mut cleanup_count = 0;
        
        match std::fs::read_dir(storage_dir) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_file() {
                            // 检查文件修改时间
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                if let Ok(modified) = metadata.modified() {
                                    let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                                    if modified_time < cutoff_time {
                                        match std::fs::remove_file(&path) {
                                            Ok(_) => {
                                                cleanup_count += 1;
                                                log::debug!("删除临时文件: {:?}", path);
                                            }
                                            Err(e) => {
                                                log::warn!("无法删除临时文件 {:?}: {}", path, e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("无法读取临时文件目录: {}", e);
            }
        }
        
        Ok(cleanup_count)
    }

    /// 清理Redis缓存
    async fn cleanup_redis_cache(&self) -> Result<u64, AppError> {
        let redis_conn = self.redis_conn.as_ref()
            .ok_or_else(|| AppError::InternalError("Redis连接不可用".to_string()))?;
        
        let retention_days = self.config.redis_retention_days;
        let cutoff_timestamp = chrono::Utc::now().timestamp() - (retention_days as i64 * 86400);
        
        // 获取所有缓存键
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("cache:*")
            .query_async(&mut redis_conn.clone())
            .await
            .map_err(|e| AppError::RedisError(e.to_string()))?;
        
        let mut cleanup_count = 0;
        
        for key in keys {
            // 检查键的过期时间或最后访问时间
            let ttl: i64 = redis::cmd("TTL")
                .arg(&key)
                .query_async(&mut redis_conn.clone())
                .await
                .map_err(|e| AppError::RedisError(e.to_string()))?;
            
            // 如果TTL为-1（永不过期）或-2（键不存在），检查最后访问时间
            if ttl == -1 {
                // 获取键的最后访问时间（使用OBJECT IDLETIME）
                let idle_time: i64 = redis::cmd("OBJECT")
                    .arg("IDLETIME")
                    .arg(&key)
                    .query_async(&mut redis_conn.clone())
                    .await
                    .map_err(|e| AppError::RedisError(e.to_string()))?;
                
                // 如果空闲时间超过保留期限，删除键
                if idle_time > cutoff_timestamp {
                    let _: () = redis::cmd("DEL")
                        .arg(&key)
                        .query_async(&mut redis_conn.clone())
                        .await
                        .map_err(|e| AppError::RedisError(e.to_string()))?;
                    
                    cleanup_count += 1;
                }
            } else if ttl == -2 {
                // 键不存在，跳过
                continue;
            }
        }
        
        // 清理过期的会话数据
        let session_keys: Vec<String> = redis::cmd("KEYS")
            .arg("session:*")
            .query_async(&mut redis_conn.clone())
            .await
            .map_err(|e| AppError::RedisError(e.to_string()))?;
        
        for key in session_keys {
            let ttl: i64 = redis::cmd("TTL")
                .arg(&key)
                .query_async(&mut redis_conn.clone())
                .await
                .map_err(|e| AppError::RedisError(e.to_string()))?;
            
            if ttl == -2 {
                // 键不存在，删除
                let _: () = redis::cmd("DEL")
                    .arg(&key)
                    .query_async(&mut redis_conn.clone())
                    .await
                    .map_err(|e| AppError::RedisError(e.to_string()))?;
                
                cleanup_count += 1;
            }
        }
        
        Ok(cleanup_count)
    }

    /// 执行一次性清理（用于手动触发）
    pub async fn run_once(&self) -> Result<u64, AppError> {
        let mut total_cleaned = 0;
        
        if self.config.enable_task_cleanup {
            total_cleaned += self.cleanup_old_tasks().await?;
        }
        
        if self.config.enable_message_cleanup {
            total_cleaned += self.cleanup_old_messages().await?;
        }
        
        if self.config.enable_temp_file_cleanup {
            total_cleaned += self.cleanup_temp_files().await?;
        }
        
        if self.config.enable_redis_cleanup && self.redis_conn.is_some() {
            total_cleaned += self.cleanup_redis_cache().await?;
        }
        
        Ok(total_cleaned)
    }
}

/// 启动清理工作器
pub async fn start_cleanup_worker(
    config: CleanupConfig,
    db_pool: PgPool,
    redis_conn: Option<ConnectionManager>,
) -> Result<(), AppError> {
    let worker = CleanupWorker::new(config, db_pool, redis_conn);
    worker.start().await
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cleanup_config_default() {
        let config = CleanupConfig::default();
        assert_eq!(config.interval_seconds, 3600);
        assert_eq!(config.task_retention_days, 30);
        assert_eq!(config.message_retention_days, 7);
        assert_eq!(config.temp_file_retention_hours, 24);
        assert_eq!(config.redis_retention_days, 3);
    }
    
    #[test]
    fn test_cleanup_worker_creation() {
        // 这个测试主要是验证结构体可以正确创建
        let config = CleanupConfig::default();
        // 由于需要数据库连接，这里只测试编译通过
        assert!(config.enable_task_cleanup);
    }
}
