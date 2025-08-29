use std::sync::Arc;
use axum::{
    routing::{Router, get, post},
    Extension,
    http::{Method, HeaderValue},
    middleware,
};
use tower_http::{
    cors::{CorsLayer, Any},
    compression::CompressionLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing::{info, warn, error};
use tokio::time::{interval, Duration};

mod routes;
mod models;
mod services;
mod config;
mod error;
mod utils;
mod state;

#[cfg(feature = "metrics")]
mod metrics;

use crate::{
    config::Config,
    state::AppState,
    services::{
        Database,
        AuthService,
        ArticleService,
        UserService,
        CommentService,
        NotificationService,
        SearchService,
        MediaService,
        RecommendationService,
        PublicationService,
        BookmarkService,
        FollowService,
        SeriesService,
        AnalyticsService,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("LOG_LEVEL").unwrap_or_else(|_| "rainbow_blog=debug,tower_http=debug".into())
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Rainbow-Blog service...");

    // 加载配置
    dotenv::dotenv().ok();
    let config = Config::from_env()?;
    
    // 初始化数据库连接
    let db = Arc::new(match Database::new(&config).await {
        Ok(db) => {
            match db.verify_connection().await {
                Ok(_) => {
                    info!("Database connection established successfully");
                    db
                }
                Err(e) => {
                    warn!("Database connection failed: {}", e);
                    info!("Attempting to auto-start database...");
                    
                    // 尝试自动启动数据库
                    if let Err(start_err) = auto_start_database(&config).await {
                        error!("Failed to auto-start database: {}. Original error: {}", start_err, e);
                        return Err(anyhow::anyhow!("Database connection failed"));
                    }
                    
                    // 重新尝试连接
                    let db = Database::new(&config).await?;
                    db.verify_connection().await?;
                    info!("Database auto-started and connected successfully");
                    db
                }
            }
        }
        Err(e) => {
            error!("Failed to create database connection: {}", e);
            return Err(anyhow::anyhow!("Database initialization failed"));
        }
    });

    // 初始化所有服务
    let auth_service = AuthService::new(&config).await?;
    let article_service = ArticleService::new(db.clone()).await?;
    let user_service = UserService::new(db.clone()).await?;
    let comment_service = CommentService::new(db.clone()).await?;
    let notification_service = NotificationService::new(db.clone(), &config).await?;
    let search_service = SearchService::new(db.clone()).await?;
    let media_service = MediaService::new(&config).await?;
    let recommendation_service = RecommendationService::new(db.clone()).await?;
    let publication_service = PublicationService::new(db.clone()).await?;
    let bookmark_service = BookmarkService::new(db.clone()).await?;
    let follow_service = FollowService::new(db.clone(), notification_service.clone()).await?;
    let tag_service = crate::services::tag::TagService::new(db.clone()).await?;
    let series_service = SeriesService::new(db.clone()).await?;
    let analytics_service = AnalyticsService::new(db.clone()).await?;

    // 创建应用状态
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db: (*db).clone(),
        auth_service,
        article_service,
        user_service,
        comment_service,
        notification_service,
        search_service,
        media_service,
        recommendation_service,
        publication_service,
        bookmark_service,
        follow_service,
        tag_service,
        series_service,
        analytics_service,
    });

    // 启动后台任务
    start_background_tasks(app_state.clone()).await;

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(
            config.cors_allowed_origins
                .split(',')
                .map(|origin| origin.parse::<HeaderValue>().unwrap())
                .collect::<Vec<_>>(),
        );

    // 构建应用路由 - 使用/api/blog/前缀避免网关路由冲突
    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .nest("/api/blog/auth", routes::auth::router())
        .nest("/api/blog/users", routes::users::router())
        .nest("/api/blog/articles", routes::articles::router())
        .nest("/api/blog/comments", routes::comments::router())
        .nest("/api/blog/tags", routes::tags::router())
        .nest("/api/blog/publications", routes::publications::router())
        .nest("/api/blog/search", routes::search::router())
        .nest("/api/blog/media", routes::media::router())
        .nest("/api/blog/stats", routes::stats::router())
        .nest("/api/blog/bookmarks", routes::bookmarks::router())
        .nest("/api/blog/follows", routes::follows::router())
        .nest("/api/blog/recommendations", routes::recommendations::router())
        .nest("/api/blog/series", routes::series::router())
        .nest("/api/blog/analytics", routes::analytics::router())
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // 启动指标服务器（如果启用）
    #[cfg(feature = "metrics")]
    if config.metrics_enabled {
        let metrics_app = metrics::setup_metrics().await?;
        let metrics_addr = format!("0.0.0.0:{}", config.metrics_port);
        info!("Starting metrics server on {}", metrics_addr);
        
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(&metrics_addr).await.unwrap();
            axum::serve(listener, metrics_app).await.unwrap();
        });
    }

    // 启动主服务器
    let addr = format!("{}:{}", config.server_host, config.server_port);
    info!("Starting server on http://{}", addr);

    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "Rainbow-Blog is running!"
}

async fn auto_start_database(config: &Config) -> anyhow::Result<()> {
    info!("Attempting to start SurrealDB...");
    
    // 尝试启动 SurrealDB 进程
    let output = tokio::process::Command::new("surreal")
        .args(&[
            "start",
            "--user", &config.database_username,
            "--pass", &config.database_password,
            "memory",
        ])
        .spawn();

    match output {
        Ok(_) => {
            info!("SurrealDB started successfully");
            // 等待数据库启动
            tokio::time::sleep(Duration::from_secs(3)).await;
            Ok(())
        }
        Err(e) => {
            error!("Failed to start SurrealDB: {}", e);
            Err(anyhow::anyhow!("Failed to start database"))
        }
    }
}

async fn start_background_tasks(app_state: Arc<AppState>) {
    info!("Starting background tasks...");

    // 推荐系统更新任务
    let recommendation_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(
            recommendation_state.config.recommendation_update_interval
        ));
        
        loop {
            interval.tick().await;
            if let Err(e) = recommendation_state.recommendation_service.update_recommendations().await {
                error!("Failed to update recommendations: {}", e);
            }
        }
    });

    // 统计数据聚合任务
    let stats_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600)); // 每小时执行一次
        
        loop {
            interval.tick().await;
            if let Err(e) = stats_state.article_service.aggregate_daily_stats().await {
                error!("Failed to aggregate daily stats: {}", e);
            }
        }
    });

    // 清理过期会话任务
    let auth_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600)); // 每小时执行一次
        
        loop {
            interval.tick().await;
            if let Err(e) = auth_state.auth_service.cleanup_expired_sessions().await {
                error!("Failed to cleanup expired sessions: {}", e);
            }
        }
    });

    info!("Background tasks started successfully");
}