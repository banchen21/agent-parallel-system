use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};


/// 从数据库URL中提取数据库名称
fn extract_database_name(database_url: &str) -> Result<String> {
    let url_parts: Vec<&str> = database_url.split('/').collect();
    if url_parts.len() >= 4 {
        Ok(url_parts[3].to_string())
    } else {
        anyhow::bail!("无效的数据库URL格式: {}", database_url)
    }
}

/// 构建postgres数据库URL（连接到默认postgres数据库）
fn build_postgres_url(database_url: &str) -> Result<String> {
    let url_parts: Vec<&str> = database_url.split('/').collect();
    if url_parts.len() >= 3 {
        // 替换数据库名称为postgres
        let mut new_parts = url_parts.clone();
        if new_parts.len() >= 4 {
            new_parts[3] = "postgres";
        }
        Ok(new_parts.join("/"))
    } else {
        anyhow::bail!("无效的数据库URL格式: {}", database_url)
    }
}

/// 确保数据库存在，如果不存在则创建
pub async fn ensure_database_exists(database_url: &str) -> Result<()> {
    // 解析数据库URL获取数据库名称
    let db_name = extract_database_name(database_url).context("无法从数据库URL中提取数据库名称")?;

    // 尝试连接到目标数据库
    match PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
    {
        Ok(_) => {
            info!("数据库 '{}' 已存在", db_name);
            return Ok(());
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains(&format!("database \"{}\" does not exist", db_name)) {
                info!("数据库 '{}' 不存在，正在创建...", db_name);

                // 从原始URL构建postgres数据库URL
                let postgres_url =
                    build_postgres_url(database_url).context("无法构建postgres数据库URL")?;

                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&postgres_url)
                    .await
                    .context("无法连接到postgres数据库")?;

                // 创建数据库
                sqlx::query(&format!("CREATE DATABASE {}", db_name))
                    .execute(&pool)
                    .await
                    .context(format!("无法创建数据库 '{}'", db_name))?;

                info!("数据库 '{}' 创建成功", db_name);
            } else {
                return Err(e).context("数据库连接失败");
            }
        }
    }

    Ok(())
}
