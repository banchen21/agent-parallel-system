use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};

const SECRET: &[u8] = b"your_super_secret_key"; // 生产环境请使用环境变量

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,     // username
    pub exp: usize,      // expiration time
    pub iat: usize,      // issued at
}

pub fn generate_tokens(username: &str) -> (String, String) {
    let now = Utc::now();
    
    // Access Token: 15分钟过期
    let access_exp = (now + Duration::minutes(15)).timestamp() as usize;
    let access_claims = Claims { sub: username.to_string(), exp: access_exp, iat: now.timestamp() as usize };
    let access_token = encode(&Header::default(), &access_claims, &EncodingKey::from_secret(SECRET)).unwrap();

    // Refresh Token: 7天过期
    let refresh_exp = (now + Duration::days(7)).timestamp() as usize;
    let refresh_claims = Claims { sub: username.to_string(), exp: refresh_exp, iat: now.timestamp() as usize };
    let refresh_token = encode(&Header::default(), &refresh_claims, &EncodingKey::from_secret(SECRET)).unwrap();

    (access_token, refresh_token)
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(SECRET), &Validation::default()).map(|data| data.claims)
}