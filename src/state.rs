use crate::{
    config::Config,
    services::{
        database::Database,
        auth::AuthService,
        article::ArticleService,
        user::UserService,
        comment::CommentService,
        notification::NotificationService,
        search::SearchService,
        media::MediaService,
        recommendation::RecommendationService,
    },
};

/// 应用程序的共享状态
/// 包含所有服务和配置的引用
#[derive(Clone)]
pub struct AppState {
    /// 应用配置
    pub config: Config,
    
    /// 数据库连接
    pub db: Database,
    
    /// 认证服务
    pub auth_service: AuthService,
    
    /// 文章服务
    pub article_service: ArticleService,
    
    /// 用户服务
    pub user_service: UserService,
    
    /// 评论服务
    pub comment_service: CommentService,
    
    /// 通知服务
    pub notification_service: NotificationService,
    
    /// 搜索服务
    pub search_service: SearchService,
    
    /// 媒体服务
    pub media_service: MediaService,
    
    /// 推荐服务
    pub recommendation_service: RecommendationService,
}

impl AppState {
    /// 检查功能是否启用
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "registrations" => self.config.enable_registrations,
            "comments" => self.config.enable_comments,
            "subscriptions" => self.config.enable_subscriptions,
            "publications" => self.config.enable_publications,
            "email_notifications" => self.config.enable_email_notifications,
            _ => false,
        }
    }
    
    /// 获取分页配置
    pub fn get_page_size(&self, resource_type: &str) -> usize {
        match resource_type {
            "articles" => self.config.default_articles_per_page,
            "comments" => self.config.default_comments_per_page,
            _ => 20, // 默认每页20条
        }
    }
    
    /// 检查是否为生产环境
    pub fn is_production(&self) -> bool {
        self.config.is_production()
    }
    
    /// 检查是否为开发环境
    pub fn is_development(&self) -> bool {
        self.config.is_development()
    }
}