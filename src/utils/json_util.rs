pub fn clean_json_string(raw: &str) -> &str {
    let mut s = raw.trim();
    // 去掉开头的 ```json 或 ```
    if s.starts_with("```json") {
        s = &s[7..];
    } else if s.starts_with("```") {
        s = &s[3..];
    }
    // 去掉结尾的 ```
    if s.ends_with("```") {
        s = &s[..s.len() - 3];
    }
    s.trim()
}
