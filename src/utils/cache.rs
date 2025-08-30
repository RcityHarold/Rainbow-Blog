use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

/// 缓存项
#[derive(Debug, Clone)]
struct CacheItem<T> {
    value: T,
    expires_at: u64,
}

/// 简单的内存缓存实现
#[derive(Debug, Clone)]
pub struct Cache<T: Clone + Send + Sync> {
    data: Arc<RwLock<HashMap<String, CacheItem<T>>>>,
    default_ttl: Duration,
}

impl<T: Clone + Send + Sync + 'static> Cache<T> {
    /// 创建新的缓存实例
    pub fn new(default_ttl: Duration) -> Self {
        let cache = Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        };
        
        // 启动后台清理任务
        let data_ref = cache.data.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(300)).await; // 每5分钟清理一次
                Self::cleanup_expired(&data_ref);
            }
        });
        
        cache
    }
    
    /// 设置缓存项
    pub fn set(&self, key: String, value: T) -> Result<(), String> {
        self.set_with_ttl(key, value, self.default_ttl)
    }
    
    /// 设置带有自定义TTL的缓存项
    pub fn set_with_ttl(&self, key: String, value: T, ttl: Duration) -> Result<(), String> {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() + ttl.as_secs();
        
        let item = CacheItem {
            value,
            expires_at,
        };
        
        let mut data = self.data.write().map_err(|e| e.to_string())?;
        data.insert(key, item);
        Ok(())
    }
    
    /// 获取缓存项
    pub fn get(&self, key: &str) -> Result<Option<T>, String> {
        let data = self.data.read().map_err(|e| e.to_string())?;
        
        if let Some(item) = data.get(key) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs();
            
            if item.expires_at > current_time {
                Ok(Some(item.value.clone()))
            } else {
                // 过期了，需要删除（在读锁下不能删除，所以先返回None）
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    /// 删除缓存项
    pub fn delete(&self, key: &str) -> Result<bool, String> {
        let mut data = self.data.write().map_err(|e| e.to_string())?;
        Ok(data.remove(key).is_some())
    }
    
    /// 清空所有缓存
    pub fn clear(&self) -> Result<(), String> {
        let mut data = self.data.write().map_err(|e| e.to_string())?;
        data.clear();
        Ok(())
    }
    
    /// 获取缓存大小
    pub fn size(&self) -> Result<usize, String> {
        let data = self.data.read().map_err(|e| e.to_string())?;
        Ok(data.len())
    }
    
    /// 检查键是否存在且未过期
    pub fn exists(&self, key: &str) -> Result<bool, String> {
        Ok(self.get(key)?.is_some())
    }
    
    /// 清理过期项
    fn cleanup_expired(data: &Arc<RwLock<HashMap<String, CacheItem<T>>>>) {
        if let Ok(mut data) = data.write() {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            
            data.retain(|_, item| item.expires_at > current_time);
        }
    }
}

/// 全局缓存管理器
pub struct CacheManager {
    /// 用户信息缓存
    pub user_cache: Cache<serde_json::Value>,
    /// 文章缓存
    pub article_cache: Cache<serde_json::Value>,
    /// 推荐结果缓存
    pub recommendation_cache: Cache<serde_json::Value>,
    /// 搜索结果缓存
    pub search_cache: Cache<serde_json::Value>,
    /// 分析数据缓存
    pub analytics_cache: Cache<serde_json::Value>,
}

impl CacheManager {
    /// 创建新的缓存管理器
    pub fn new() -> Self {
        Self {
            user_cache: Cache::new(Duration::from_secs(300)), // 5分钟
            article_cache: Cache::new(Duration::from_secs(600)), // 10分钟
            recommendation_cache: Cache::new(Duration::from_secs(900)), // 15分钟
            search_cache: Cache::new(Duration::from_secs(300)), // 5分钟
            analytics_cache: Cache::new(Duration::from_secs(600)), // 10分钟
        }
    }
    
    /// 生成推荐缓存键
    pub fn recommendation_key(user_id: &str, algorithm: &str, limit: usize) -> String {
        format!("rec:{}:{}:{}", user_id, algorithm, limit)
    }
    
    /// 生成搜索缓存键
    pub fn search_key(query: &str, search_type: &str, page: usize) -> String {
        format!("search:{}:{}:{}", 
            Self::hash_string(query), search_type, page)
    }
    
    /// 生成分析缓存键
    pub fn analytics_key(user_id: &str, metric: &str, period: &str) -> String {
        format!("analytics:{}:{}:{}", user_id, metric, period)
    }
    
    /// 生成用户缓存键
    pub fn user_key(user_id: &str) -> String {
        format!("user:{}", user_id)
    }
    
    /// 生成文章缓存键
    pub fn article_key(article_id: &str) -> String {
        format!("article:{}", article_id)
    }
    
    /// 简单字符串哈希（用于缩短缓存键）
    fn hash_string(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 缓存辅助宏
#[macro_export]
macro_rules! cache_get_or_set {
    ($cache:expr, $key:expr, $compute:expr) => {{
        match $cache.get($key) {
            Ok(Some(value)) => Ok(value),
            _ => {
                let computed_value = $compute?;
                let _ = $cache.set($key.to_string(), computed_value.clone());
                Ok(computed_value)
            }
        }
    }};
}

/// 异步缓存辅助宏
#[macro_export]
macro_rules! cache_get_or_set_async {
    ($cache:expr, $key:expr, $compute:expr) => {{
        match $cache.get($key) {
            Ok(Some(value)) => Ok(value),
            _ => {
                let computed_value = $compute.await?;
                let _ = $cache.set($key.to_string(), computed_value.clone());
                Ok(computed_value)
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = Cache::new(Duration::from_secs(1));
        
        // 测试设置和获取
        cache.set("key1".to_string(), "value1".to_string()).unwrap();
        assert_eq!(cache.get("key1").unwrap(), Some("value1".to_string()));
        
        // 测试不存在的键
        assert_eq!(cache.get("nonexistent").unwrap(), None);
        
        // 测试删除
        assert!(cache.delete("key1").unwrap());
        assert_eq!(cache.get("key1").unwrap(), None);
    }
    
    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = Cache::new(Duration::from_millis(100));
        
        cache.set("temp_key".to_string(), "temp_value".to_string()).unwrap();
        assert_eq!(cache.get("temp_key").unwrap(), Some("temp_value".to_string()));
        
        sleep(Duration::from_millis(150)).await;
        assert_eq!(cache.get("temp_key").unwrap(), None);
    }
    
    #[test]
    fn test_cache_manager() {
        let manager = CacheManager::new();
        
        // 测试键生成
        let rec_key = CacheManager::recommendation_key("user123", "hybrid", 10);
        assert_eq!(rec_key, "rec:user123:hybrid:10");
        
        let search_key = CacheManager::search_key("rust programming", "articles", 1);
        assert!(search_key.starts_with("search:"));
        
        let analytics_key = CacheManager::analytics_key("user456", "dashboard", "30d");
        assert_eq!(analytics_key, "analytics:user456:dashboard:30d");
    }
}