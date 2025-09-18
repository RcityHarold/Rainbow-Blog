/// 用于处理 SurrealDB Thing ID 的序列化/反序列化辅助模块

use serde::{Deserialize, Deserializer, Serializer};
use chrono::{DateTime, Utc};

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
                    serde_json::Value::Object(map) => {
                        if let Some(value) = map.get("String").and_then(|v| v.as_str()) {
                            return Ok(format!("{}:{}", tb, value));
                        }
                        if let Some(value) = map.get("id") {
                            match value {
                                serde_json::Value::String(s) => {
                                    return Ok(format!("{}:{}", tb, s));
                                }
                                serde_json::Value::Object(inner) => {
                                    if let Some(s) = inner.get("String").and_then(|v| v.as_str()) {
                                        return Ok(format!("{}:{}", tb, s));
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let Some((_, value)) = map.iter().find(|(k, _)| k.eq_ignore_ascii_case("string")) {
                            if let Some(s) = value.as_str() {
                                return Ok(format!("{}:{}", tb, s));
                            }
                        }
                        let fallback = serde_json::to_string(&map).unwrap_or_default();
                        Ok(format!("{}:{}", tb, fallback))
                    }
                    other => Ok(format!("{}:{}", tb, other)),
                }
            }
        }
    }
}

/// 处理 SurrealDB 的 DateTime 格式
pub mod surrealdb_datetime {
    use super::*;
    use chrono::format::Fixed;
    use chrono::TimeZone;
    use serde::de::{self, Unexpected};
    
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 使用 SurrealDB 期望的格式
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("datetime", 1)?;
        state.serialize_field("datetime", &dt.to_rfc3339())?;
        state.end()
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum DateTimeValue {
            String(String),
            Object { datetime: String },
        }
        
        match DateTimeValue::deserialize(deserializer)? {
            DateTimeValue::String(s) => {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(&s), &"RFC3339 datetime"))
            }
            DateTimeValue::Object { datetime } => {
                DateTime::parse_from_rfc3339(&datetime)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(&datetime), &"RFC3339 datetime"))
            }
        }
    }
}

/// 处理可选的 SurrealDB DateTime
pub mod surrealdb_datetime_option {
    use super::*;
    
    pub fn serialize<S>(dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dt {
            Some(dt) => surrealdb_datetime::serialize(dt, serializer),
            None => serializer.serialize_none(),
        }
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum OptionalDateTime {
            Some(#[serde(with = "surrealdb_datetime")] DateTime<Utc>),
            None,
        }
        
        match OptionalDateTime::deserialize(deserializer)? {
            OptionalDateTime::Some(dt) => Ok(Some(dt)),
            OptionalDateTime::None => Ok(None),
        }
    }
}
