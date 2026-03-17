use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::timeout;

use crate::mcp::model::McpError;

pub async fn execute_get_weather(arguments: &Value) -> Result<Value, McpError> {
    let location = arguments
        .get("location")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| McpError::Message("get_weather 缺少 location 参数".to_string()))?;
    let unit = arguments
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("celsius");
    let days = arguments.get("days").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3);

    let client = reqwest::Client::new();
    let url = format!("https://wttr.in/{}", location);
    let resp = timeout(
        Duration::from_millis(10_000),
        client.get(url).query(&[("format", "j1")]).send(),
    )
    .await
    .map_err(|_| McpError::Message("get_weather 请求超时".to_string()))?
    .map_err(|e| McpError::Message(format!("get_weather 请求失败: {}", e)))?;

    let text = resp
        .text()
        .await
        .map_err(|e| McpError::Message(format!("读取天气响应失败: {}", e)))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| McpError::Message(format!("解析天气响应 JSON 失败: {}", e)))?;

    let current = v
        .get("current_condition")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);
    let forecast = v
        .get("weather")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().take(days as usize).cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let temp_c = current.get("temp_C").cloned().unwrap_or(Value::Null);
    let temp_f = current.get("temp_F").cloned().unwrap_or(Value::Null);
    let chosen_temp = if unit.eq_ignore_ascii_case("fahrenheit") {
        temp_f
    } else {
        temp_c
    };

    Ok(json!({
        "provider": "wttr.in",
        "location": location,
        "unit": unit,
        "current": {
            "temperature": chosen_temp,
            "weather": current.get("weatherDesc").cloned().unwrap_or(Value::Null),
            "humidity": current.get("humidity").cloned().unwrap_or(Value::Null),
            "wind_kmph": current.get("windspeedKmph").cloned().unwrap_or(Value::Null)
        },
        "forecast": forecast
    }))
}
