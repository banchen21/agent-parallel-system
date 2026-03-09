use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use tracing::debug;

const SECRET: &[u8] = b"your_super_secret_key"; // TODO: 生产环境请使用环境变量

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // username
    pub exp: usize,         // expiration time
    pub iat: usize,         // issued at
    pub token_type: String, // "access" 或 "refresh"
}

pub fn generate_tokens(username: &str) -> (String, String) {
    let now = Utc::now();

    // Access Token
    let access_claims = Claims {
        sub: username.to_string(),
        exp: (now + Duration::minutes(15)).timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: "access".to_string(),
    };

    // Refresh Token
    let refresh_claims = Claims {
        sub: username.to_string(),
        exp: (now + Duration::days(7)).timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: "refresh".to_string(),
    };
    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(SECRET),
    )
    .unwrap();

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(SECRET),
    )
    .unwrap();

    (access_token, refresh_token)
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

// 测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_cycle() {
        let username = "test_user";
        // 1. 生成
        let (at, rt) = generate_tokens(username);
        println!("Access Token: {}", at);

        // 2. 立即验证
        let result = validate_token(&at);

        // 如果这里报错，说明你的 SECRET 或算法在同一个文件里都不匹配
        assert!(result.is_ok(), "验证失败: {:?}", result.err());

        let claims = result.unwrap();
        assert_eq!(claims.sub, username);
        assert_eq!(claims.token_type, "access");
        println!("✅ 内部验证成功！");
    }

    #[test]
    fn test_middleware_logic_simulation() {
        let (at, _) = generate_tokens("user123");

        // 模拟中间件收到的 Header 字符串
        let auth_header_val = format!("Bearer {}", at);

        // 模拟中间件截取逻辑
        let token_part = &auth_header_val[7..].trim();

        let result = validate_token(token_part);
        assert!(result.is_ok(), "模拟中间件截取验证失败: {:?}", result.err());
    }
}
