pub mod database;
pub mod auth;
pub mod user;
pub mod article;
pub mod comment;
pub mod notification;
pub mod search;
pub mod media;
pub mod recommendation;
pub mod publication;
pub mod tag;

// 重新导出常用类型
pub use database::Database;
pub use auth::AuthService;
pub use user::UserService;
pub use article::ArticleService;
pub use comment::CommentService;
pub use notification::NotificationService;
pub use search::SearchService;
pub use media::MediaService;
pub use recommendation::RecommendationService;
pub use publication::PublicationService;
pub use tag::TagService;