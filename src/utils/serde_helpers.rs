/// 用于处理 SurrealDB Thing ID 的序列化/反序列化辅助模块

use serde::{Deserialize, Deserializer, Serializer};

/// 处理 SurrealDB 的 Thing ID 格式 (例如: "tag:xxxxx")
pub mod thing_id {
    use super::*;
    
    pub fn serialize<S>(id: &str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(id)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum IdValue {
            String(String),
            Thing { 
                tb: String, 
                id: serde_json::Value 
            },
        }
        
        match IdValue::deserialize(deserializer)? {
            IdValue::String(s) => Ok(s),
            IdValue::Thing { tb, id } => {
                match id {
                    serde_json::Value::String(s) => Ok(format!("{}:{}", tb, s)),
                    serde_json::Value::Number(n) => Ok(format!("{}:{}", tb, n)),
                    _ => Ok(format!("{}:{}", tb, id)),
                }
            }
        }
    }
}