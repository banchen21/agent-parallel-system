use chrono::{Duration, Local};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use log::info;
use serde::{Deserialize, Serialize};

const SECRET: &[u8] = b"your_super_secret_key"; // TODO: 生产环境请使用环境变量

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // username
    pub exp: usize,         // expiration time
    pub iat: usize,         // issued at
    pub token_type: String, // "access" 或 "refresh"
}

pub fn generate_tokens(username: &str) -> (String, String) {
    let now = Local::now();

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
