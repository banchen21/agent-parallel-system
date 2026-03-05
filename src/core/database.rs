use anyhow::{Context, Result};
use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::info;

use super::config;

/// 创建PostgreSQL数据库连接池
pub async fn create_db_pool() -> Result<PgPool> {
    let database_url = config::CONFIG.database_url();
    
    info!("Connecting to database at: {}", database_url);
    
    let pool = PgPoolOptions::new()
        .max_connections(config::CONFIG.database.max_connections)
        .min_connections(config::CONFIG.database.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(
            config::CONFIG.database.connect_timeout,
        ))
        .idle_timeout(std::time::Duration::from_secs(
            config::CONFIG.database.idle_timeout,
        ))
        .connect(database_url)
        .await
        .context("Failed to create database connection pool")?;
    
    // 测试连接
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("Failed to test database connection")?;
    
    info!("Database connection pool created successfully");
    Ok(pool)
}

/// 创建Redis连接池
pub async fn create_redis_pool() -> Result<Pool<RedisConnectionManager>> {
    let redis_url = config::CONFIG.redis_url();
    
    info!("Connecting to Redis at: {}", redis_url);
    
    let manager = RedisConnectionManager::new(redis_url)
        .context("Failed to create Redis connection manager")?;
    
    let pool = Pool::builder()
        .max_size(config::CONFIG.redis.pool_size)
        .build(manager)
        .await
        .context("Failed to create Redis connection pool")?;
    
    // 测试连接
    let mut conn = pool.get().await?;
    redis::cmd("PING")
        .query_async::<String>(&mut *conn)
        .await
        .context("Failed to test Redis connection")?;
    drop(conn);
    
    info!("Redis connection pool created successfully");
    Ok(pool)
}

/// 获取数据库连接
pub async fn get_db_conn(pool: &PgPool) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>> {
    pool.acquire()
        .await
        .context("Failed to acquire database connection")
}

/// 获取Redis连接
pub async fn get_redis_conn(
    pool: &Pool<RedisConnectionManager>,
) -> Result<PooledConnection<'_, RedisConnectionManager>> {
    pool.get()
        .await
        .context("Failed to acquire Redis connection")
}

/// 数据库迁移
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    info!("Running database migrations...");
    
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("Failed to run database migrations")?;
    
    info!("Database migrations completed successfully");
    Ok(())
}

/// 数据库健康检查
pub async fn check_database_health(pool: &PgPool) -> Result<bool> {
    match sqlx::query("SELECT 1").execute(pool).await {
        Ok(_) => Ok(true),
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            Ok(false)
        }
    }
}

/// Redis健康检查
pub async fn check_redis_health(pool: &Pool<RedisConnectionManager>) -> Result<bool> {
    match pool.get().await {
        Ok(mut conn) => match redis::cmd("PING").query_async::<String>(&mut *conn).await {
            Ok(pong) => Ok(pong == "PONG"),
            Err(e) => {
                tracing::error!("Redis health check failed: {}", e);
                Ok(false)
            }
        },
        Err(e) => {
            tracing::error!("Failed to get Redis connection for health check: {}", e);
            Ok(false)
        }
    }
}
