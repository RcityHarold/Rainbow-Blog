-- Rainbow-Blog Database Schema
-- 基于 Medium 风格的博客系统数据库架构
-- 使用 SurrealDB 语法

-- =====================================
-- 用户扩展表（基于 Rainbow-Auth）
-- =====================================

-- 用户资料扩展表
DEFINE TABLE user_profile SCHEMAFULL;
DEFINE FIELD id ON user_profile TYPE record(user_profile);
DEFINE FIELD user_id ON user_profile TYPE string ASSERT $value != NONE; -- Rainbow-Auth 用户ID
DEFINE FIELD username ON user_profile TYPE string ASSERT $value != NONE AND string::len($value) >= 3 AND string::len($value) <= 30;
DEFINE FIELD display_name ON user_profile TYPE string ASSERT $value != NONE AND string::len($value) <= 50;
DEFINE FIELD bio ON user_profile TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 160;
DEFINE FIELD avatar_url ON user_profile TYPE option<string>;
DEFINE FIELD cover_image_url ON user_profile TYPE option<string>;
DEFINE FIELD website ON user_profile TYPE option<string>;
DEFINE FIELD location ON user_profile TYPE option<string>;
DEFINE FIELD twitter_username ON user_profile TYPE option<string>;
DEFINE FIELD github_username ON user_profile TYPE option<string>;
DEFINE FIELD linkedin_url ON user_profile TYPE option<string>;
DEFINE FIELD facebook_url ON user_profile TYPE option<string>;
DEFINE FIELD follower_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD following_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD article_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD total_claps_received ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD is_verified ON user_profile TYPE bool DEFAULT false;
DEFINE FIELD is_suspended ON user_profile TYPE bool DEFAULT false;
DEFINE FIELD created_at ON user_profile TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON user_profile TYPE datetime DEFAULT time::now();

-- 用户资料索引
DEFINE INDEX user_profile_user_id_idx ON user_profile COLUMNS user_id UNIQUE;
DEFINE INDEX user_profile_username_idx ON user_profile COLUMNS username UNIQUE;
DEFINE INDEX user_profile_verified_idx ON user_profile COLUMNS is_verified;

-- =====================================
-- 核心内容表
-- =====================================

-- 文章表
DEFINE TABLE article SCHEMAFULL;
DEFINE FIELD id ON article TYPE record(article);
DEFINE FIELD title ON article TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 150;
DEFINE FIELD subtitle ON article TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 200;
DEFINE FIELD slug ON article TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 200;
DEFINE FIELD content ON article TYPE string DEFAULT "";
DEFINE FIELD content_html ON article TYPE string DEFAULT ""; -- 渲染后的HTML
DEFINE FIELD excerpt ON article TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 300;
DEFINE FIELD cover_image_url ON article TYPE option<string>;
DEFINE FIELD author_id ON article TYPE string ASSERT $value != NONE;
DEFINE FIELD publication_id ON article TYPE option<record(publication)>;
DEFINE FIELD series_id ON article TYPE option<record(series)>;
DEFINE FIELD series_order ON article TYPE option<number>;
DEFINE FIELD status ON article TYPE string DEFAULT "draft" ASSERT $value INSIDE ["draft", "published", "unlisted", "archived"];
DEFINE FIELD is_paid_content ON article TYPE bool DEFAULT false;
DEFINE FIELD is_featured ON article TYPE bool DEFAULT false;
DEFINE FIELD reading_time ON article TYPE number DEFAULT 0; -- 预计阅读时间（分钟）
DEFINE FIELD word_count ON article TYPE number DEFAULT 0;
DEFINE FIELD view_count ON article TYPE number DEFAULT 0;
DEFINE FIELD clap_count ON article TYPE number DEFAULT 0;
DEFINE FIELD comment_count ON article TYPE number DEFAULT 0;
DEFINE FIELD bookmark_count ON article TYPE number DEFAULT 0;
DEFINE FIELD share_count ON article TYPE number DEFAULT 0;
DEFINE FIELD seo_title ON article TYPE option<string>;
DEFINE FIELD seo_description ON article TYPE option<string>;
DEFINE FIELD seo_keywords ON article TYPE array<string> DEFAULT [];
DEFINE FIELD metadata ON article TYPE object DEFAULT {};
DEFINE FIELD created_at ON article TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON article TYPE datetime DEFAULT time::now();
DEFINE FIELD published_at ON article TYPE option<datetime>;
DEFINE FIELD last_edited_at ON article TYPE option<datetime>;
DEFINE FIELD is_deleted ON article TYPE bool DEFAULT false;
DEFINE FIELD deleted_at ON article TYPE option<datetime>;

-- 文章索引
DEFINE INDEX article_slug_idx ON article COLUMNS slug UNIQUE;
DEFINE INDEX article_author_idx ON article COLUMNS author_id;
DEFINE INDEX article_publication_idx ON article COLUMNS publication_id;
DEFINE INDEX article_series_idx ON article COLUMNS series_id;
DEFINE INDEX article_status_idx ON article COLUMNS status;
DEFINE INDEX article_published_idx ON article COLUMNS published_at;
DEFINE INDEX article_featured_idx ON article COLUMNS is_featured;
DEFINE INDEX article_deleted_idx ON article COLUMNS is_deleted;

-- 文章版本历史表
DEFINE TABLE article_version SCHEMAFULL;
DEFINE FIELD id ON article_version TYPE record(article_version);
DEFINE FIELD article_id ON article_version TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD version_number ON article_version TYPE number ASSERT $value != NONE AND $value > 0;
DEFINE FIELD title ON article_version TYPE string ASSERT $value != NONE;
DEFINE FIELD subtitle ON article_version TYPE option<string>;
DEFINE FIELD content ON article_version TYPE string DEFAULT "";
DEFINE FIELD content_html ON article_version TYPE string DEFAULT "";
DEFINE FIELD change_summary ON article_version TYPE option<string>;
DEFINE FIELD author_id ON article_version TYPE string ASSERT $value != NONE;
DEFINE FIELD created_at ON article_version TYPE datetime DEFAULT time::now();

-- 版本索引
DEFINE INDEX article_version_article_idx ON article_version COLUMNS article_id;
DEFINE INDEX article_version_number_idx ON article_version COLUMNS article_id, version_number UNIQUE;

-- 文章系列表
DEFINE TABLE series SCHEMAFULL;
DEFINE FIELD id ON series TYPE record(series);
DEFINE FIELD title ON series TYPE string ASSERT $value != NONE AND string::len($value) <= 100;
DEFINE FIELD description ON series TYPE option<string>;
DEFINE FIELD slug ON series TYPE string ASSERT $value != NONE;
DEFINE FIELD author_id ON series TYPE string ASSERT $value != NONE;
DEFINE FIELD cover_image_url ON series TYPE option<string>;
DEFINE FIELD article_count ON series TYPE number DEFAULT 0;
DEFINE FIELD is_completed ON series TYPE bool DEFAULT false;
DEFINE FIELD created_at ON series TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON series TYPE datetime DEFAULT time::now();

-- 系列索引
DEFINE INDEX series_slug_idx ON series COLUMNS author_id, slug UNIQUE;
DEFINE INDEX series_author_idx ON series COLUMNS author_id;

-- =====================================
-- 标签系统
-- =====================================

-- 标签表
DEFINE TABLE tag SCHEMAFULL;
DEFINE FIELD id ON tag TYPE record(tag);
DEFINE FIELD name ON tag TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 50;
DEFINE FIELD slug ON tag TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON tag TYPE option<string>;
DEFINE FIELD follower_count ON tag TYPE number DEFAULT 0;
DEFINE FIELD article_count ON tag TYPE number DEFAULT 0;
DEFINE FIELD is_featured ON tag TYPE bool DEFAULT false;
DEFINE FIELD created_at ON tag TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON tag TYPE datetime DEFAULT time::now();

-- 标签索引
DEFINE INDEX tag_slug_idx ON tag COLUMNS slug UNIQUE;
DEFINE INDEX tag_name_idx ON tag COLUMNS name UNIQUE;
DEFINE INDEX tag_featured_idx ON tag COLUMNS is_featured;

-- 文章标签关联表
DEFINE TABLE article_tag SCHEMAFULL;
DEFINE FIELD id ON article_tag TYPE record(article_tag);
DEFINE FIELD article_id ON article_tag TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD tag_id ON article_tag TYPE record(tag) ASSERT $value != NONE;
DEFINE FIELD created_at ON article_tag TYPE datetime DEFAULT time::now();

-- 文章标签索引
DEFINE INDEX article_tag_unique_idx ON article_tag COLUMNS article_id, tag_id UNIQUE;
DEFINE INDEX article_tag_article_idx ON article_tag COLUMNS article_id;
DEFINE INDEX article_tag_tag_idx ON article_tag COLUMNS tag_id;

-- 用户关注标签表
DEFINE TABLE user_tag_follow SCHEMAFULL;
DEFINE FIELD id ON user_tag_follow TYPE record(user_tag_follow);
DEFINE FIELD user_id ON user_tag_follow TYPE string ASSERT $value != NONE;
DEFINE FIELD tag_id ON user_tag_follow TYPE record(tag) ASSERT $value != NONE;
DEFINE FIELD created_at ON user_tag_follow TYPE datetime DEFAULT time::now();

-- 用户标签关注索引
DEFINE INDEX user_tag_follow_unique_idx ON user_tag_follow COLUMNS user_id, tag_id UNIQUE;
DEFINE INDEX user_tag_follow_user_idx ON user_tag_follow COLUMNS user_id;
DEFINE INDEX user_tag_follow_tag_idx ON user_tag_follow COLUMNS tag_id;

-- =====================================
-- 互动系统
-- =====================================

-- 点赞表（Claps）
DEFINE TABLE clap SCHEMAFULL;
DEFINE FIELD id ON clap TYPE record(clap);
DEFINE FIELD user_id ON clap TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON clap TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD count ON clap TYPE number DEFAULT 1 ASSERT $value >= 1 AND $value <= 50; -- Medium限制最多50次
DEFINE FIELD created_at ON clap TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON clap TYPE datetime DEFAULT time::now();

-- 点赞索引
DEFINE INDEX clap_unique_idx ON clap COLUMNS user_id, article_id UNIQUE;
DEFINE INDEX clap_article_idx ON clap COLUMNS article_id;
DEFINE INDEX clap_user_idx ON clap COLUMNS user_id;

-- 评论表
DEFINE TABLE comment SCHEMAFULL;
DEFINE FIELD id ON comment TYPE record(comment);
DEFINE FIELD article_id ON comment TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD author_id ON comment TYPE string ASSERT $value != NONE;
DEFINE FIELD parent_id ON comment TYPE option<record(comment)>; -- 支持回复
DEFINE FIELD content ON comment TYPE string ASSERT $value != NONE AND string::len($value) > 0;
DEFINE FIELD is_author_response ON comment TYPE bool DEFAULT false; -- 作者回复标记
DEFINE FIELD clap_count ON comment TYPE number DEFAULT 0;
DEFINE FIELD is_edited ON comment TYPE bool DEFAULT false;
DEFINE FIELD is_deleted ON comment TYPE bool DEFAULT false;
DEFINE FIELD created_at ON comment TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON comment TYPE datetime DEFAULT time::now();
DEFINE FIELD deleted_at ON comment TYPE option<datetime>;

-- 评论索引
DEFINE INDEX comment_article_idx ON comment COLUMNS article_id;
DEFINE INDEX comment_author_idx ON comment COLUMNS author_id;
DEFINE INDEX comment_parent_idx ON comment COLUMNS parent_id;
DEFINE INDEX comment_deleted_idx ON comment COLUMNS is_deleted;

-- 评论点赞表
DEFINE TABLE comment_clap SCHEMAFULL;
DEFINE FIELD id ON comment_clap TYPE record(comment_clap);
DEFINE FIELD user_id ON comment_clap TYPE string ASSERT $value != NONE;
DEFINE FIELD comment_id ON comment_clap TYPE record(comment) ASSERT $value != NONE;
DEFINE FIELD created_at ON comment_clap TYPE datetime DEFAULT time::now();

-- 评论点赞索引
DEFINE INDEX comment_clap_unique_idx ON comment_clap COLUMNS user_id, comment_id UNIQUE;
DEFINE INDEX comment_clap_comment_idx ON comment_clap COLUMNS comment_id;

-- 书签表
DEFINE TABLE bookmark SCHEMAFULL;
DEFINE FIELD id ON bookmark TYPE record(bookmark);
DEFINE FIELD user_id ON bookmark TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON bookmark TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD note ON bookmark TYPE option<string>; -- 私人笔记
DEFINE FIELD created_at ON bookmark TYPE datetime DEFAULT time::now();

-- 书签索引
DEFINE INDEX bookmark_unique_idx ON bookmark COLUMNS user_id, article_id UNIQUE;
DEFINE INDEX bookmark_user_idx ON bookmark COLUMNS user_id;
DEFINE INDEX bookmark_article_idx ON bookmark COLUMNS article_id;

-- 高亮表
DEFINE TABLE highlight SCHEMAFULL;
DEFINE FIELD id ON highlight TYPE record(highlight);
DEFINE FIELD user_id ON highlight TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON highlight TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD text ON highlight TYPE string ASSERT $value != NONE;
DEFINE FIELD start_offset ON highlight TYPE number ASSERT $value >= 0;
DEFINE FIELD end_offset ON highlight TYPE number ASSERT $value > 0;
DEFINE FIELD note ON highlight TYPE option<string>;
DEFINE FIELD is_private ON highlight TYPE bool DEFAULT true;
DEFINE FIELD created_at ON highlight TYPE datetime DEFAULT time::now();

-- 高亮索引
DEFINE INDEX highlight_user_article_idx ON highlight COLUMNS user_id, article_id;
DEFINE INDEX highlight_article_idx ON highlight COLUMNS article_id;
DEFINE INDEX highlight_private_idx ON highlight COLUMNS is_private;

-- =====================================
-- 社交系统
-- =====================================

-- 关注表
DEFINE TABLE follow SCHEMAFULL;
DEFINE FIELD id ON follow TYPE record(follow);
DEFINE FIELD follower_id ON follow TYPE string ASSERT $value != NONE;
DEFINE FIELD following_id ON follow TYPE string ASSERT $value != NONE;
DEFINE FIELD created_at ON follow TYPE datetime DEFAULT time::now();

-- 关注索引
DEFINE INDEX follow_unique_idx ON follow COLUMNS follower_id, following_id UNIQUE;
DEFINE INDEX follow_follower_idx ON follow COLUMNS follower_id;
DEFINE INDEX follow_following_idx ON follow COLUMNS following_id;

-- =====================================
-- 出版物系统
-- =====================================

-- 出版物表
DEFINE TABLE publication SCHEMAFULL;
DEFINE FIELD id ON publication TYPE record(publication);
DEFINE FIELD name ON publication TYPE string ASSERT $value != NONE AND string::len($value) <= 100;
DEFINE FIELD slug ON publication TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON publication TYPE option<string>;
DEFINE FIELD tagline ON publication TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 100;
DEFINE FIELD logo_url ON publication TYPE option<string>;
DEFINE FIELD cover_image_url ON publication TYPE option<string>;
DEFINE FIELD owner_id ON publication TYPE string ASSERT $value != NONE;
DEFINE FIELD homepage_layout ON publication TYPE string DEFAULT "grid" ASSERT $value INSIDE ["grid", "list", "magazine"];
DEFINE FIELD theme_color ON publication TYPE string DEFAULT "#000000";
DEFINE FIELD custom_domain ON publication TYPE option<string>;
DEFINE FIELD google_analytics_id ON publication TYPE option<string>;
DEFINE FIELD twitter_username ON publication TYPE option<string>;
DEFINE FIELD facebook_page_url ON publication TYPE option<string>;
DEFINE FIELD instagram_username ON publication TYPE option<string>;
DEFINE FIELD member_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD article_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD follower_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD is_verified ON publication TYPE bool DEFAULT false;
DEFINE FIELD is_suspended ON publication TYPE bool DEFAULT false;
DEFINE FIELD created_at ON publication TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON publication TYPE datetime DEFAULT time::now();

-- 出版物索引
DEFINE INDEX publication_slug_idx ON publication COLUMNS slug UNIQUE;
DEFINE INDEX publication_owner_idx ON publication COLUMNS owner_id;
DEFINE INDEX publication_domain_idx ON publication COLUMNS custom_domain;
DEFINE INDEX publication_verified_idx ON publication COLUMNS is_verified;

-- 出版物成员表
DEFINE TABLE publication_member SCHEMAFULL;
DEFINE FIELD id ON publication_member TYPE record(publication_member);
DEFINE FIELD publication_id ON publication_member TYPE record(publication) ASSERT $value != NONE;
DEFINE FIELD user_id ON publication_member TYPE string ASSERT $value != NONE;
DEFINE FIELD role ON publication_member TYPE string DEFAULT "writer" ASSERT $value INSIDE ["owner", "editor", "writer"];
DEFINE FIELD permissions ON publication_member TYPE array<string> DEFAULT ["article.write"];
DEFINE FIELD invited_by ON publication_member TYPE string ASSERT $value != NONE;
DEFINE FIELD accepted_at ON publication_member TYPE option<datetime>;
DEFINE FIELD created_at ON publication_member TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON publication_member TYPE datetime DEFAULT time::now();

-- 出版物成员索引
DEFINE INDEX publication_member_unique_idx ON publication_member COLUMNS publication_id, user_id UNIQUE;
DEFINE INDEX publication_member_publication_idx ON publication_member COLUMNS publication_id;
DEFINE INDEX publication_member_user_idx ON publication_member COLUMNS user_id;

-- 出版物关注表
DEFINE TABLE publication_follow SCHEMAFULL;
DEFINE FIELD id ON publication_follow TYPE record(publication_follow);
DEFINE FIELD user_id ON publication_follow TYPE string ASSERT $value != NONE;
DEFINE FIELD publication_id ON publication_follow TYPE record(publication) ASSERT $value != NONE;
DEFINE FIELD created_at ON publication_follow TYPE datetime DEFAULT time::now();

-- 出版物关注索引
DEFINE INDEX publication_follow_unique_idx ON publication_follow COLUMNS user_id, publication_id UNIQUE;
DEFINE INDEX publication_follow_user_idx ON publication_follow COLUMNS user_id;
DEFINE INDEX publication_follow_publication_idx ON publication_follow COLUMNS publication_id;

-- =====================================
-- 订阅和付费系统
-- =====================================

-- 订阅计划表
DEFINE TABLE subscription_plan SCHEMAFULL;
DEFINE FIELD id ON subscription_plan TYPE record(subscription_plan);
DEFINE FIELD creator_id ON subscription_plan TYPE string ASSERT $value != NONE; -- 创作者ID
DEFINE FIELD name ON subscription_plan TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON subscription_plan TYPE option<string>;
DEFINE FIELD price ON subscription_plan TYPE number ASSERT $value >= 0; -- 月费（美分）
DEFINE FIELD currency ON subscription_plan TYPE string DEFAULT "USD";
DEFINE FIELD benefits ON subscription_plan TYPE array<string> DEFAULT [];
DEFINE FIELD is_active ON subscription_plan TYPE bool DEFAULT true;
DEFINE FIELD created_at ON subscription_plan TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON subscription_plan TYPE datetime DEFAULT time::now();

-- 订阅计划索引
DEFINE INDEX subscription_plan_creator_idx ON subscription_plan COLUMNS creator_id;
DEFINE INDEX subscription_plan_active_idx ON subscription_plan COLUMNS is_active;

-- 用户订阅表
DEFINE TABLE subscription SCHEMAFULL;
DEFINE FIELD id ON subscription TYPE record(subscription);
DEFINE FIELD subscriber_id ON subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD plan_id ON subscription TYPE record(subscription_plan) ASSERT $value != NONE;
DEFINE FIELD creator_id ON subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD status ON subscription TYPE string DEFAULT "active" ASSERT $value INSIDE ["active", "canceled", "expired", "past_due"];
DEFINE FIELD started_at ON subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD current_period_end ON subscription TYPE datetime ASSERT $value != NONE;
DEFINE FIELD canceled_at ON subscription TYPE option<datetime>;
DEFINE FIELD stripe_subscription_id ON subscription TYPE option<string>; -- 支付平台ID
DEFINE FIELD created_at ON subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON subscription TYPE datetime DEFAULT time::now();

-- 订阅索引
DEFINE INDEX subscription_subscriber_idx ON subscription COLUMNS subscriber_id;
DEFINE INDEX subscription_creator_idx ON subscription COLUMNS creator_id;
DEFINE INDEX subscription_status_idx ON subscription COLUMNS status;
DEFINE INDEX subscription_stripe_idx ON subscription COLUMNS stripe_subscription_id;

-- =====================================
-- 通知系统
-- =====================================

-- 通知表
DEFINE TABLE notification SCHEMAFULL;
DEFINE FIELD id ON notification TYPE record(notification);
DEFINE FIELD recipient_id ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD type ON notification TYPE string ASSERT $value INSIDE ["follow", "clap", "comment", "mention", "article_published", "subscription"];
DEFINE FIELD title ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD message ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD data ON notification TYPE object DEFAULT {}; -- 相关数据（文章ID、用户ID等）
DEFINE FIELD is_read ON notification TYPE bool DEFAULT false;
DEFINE FIELD read_at ON notification TYPE option<datetime>;
DEFINE FIELD created_at ON notification TYPE datetime DEFAULT time::now();

-- 通知索引
DEFINE INDEX notification_recipient_idx ON notification COLUMNS recipient_id;
DEFINE INDEX notification_recipient_unread_idx ON notification COLUMNS recipient_id, is_read;
DEFINE INDEX notification_created_idx ON notification COLUMNS created_at;

-- =====================================
-- 统计和分析
-- =====================================

-- 文章统计表（按天汇总）
DEFINE TABLE article_stats_daily SCHEMAFULL;
DEFINE FIELD id ON article_stats_daily TYPE record(article_stats_daily);
DEFINE FIELD article_id ON article_stats_daily TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD date ON article_stats_daily TYPE datetime ASSERT $value != NONE;
DEFINE FIELD views ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD reads ON article_stats_daily TYPE number DEFAULT 0; -- 完整阅读
DEFINE FIELD claps ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD comments ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD bookmarks ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD shares ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD reading_time_total ON article_stats_daily TYPE number DEFAULT 0; -- 总阅读时间（秒）
DEFINE FIELD created_at ON article_stats_daily TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON article_stats_daily TYPE datetime DEFAULT time::now();

-- 文章统计索引
DEFINE INDEX article_stats_daily_unique_idx ON article_stats_daily COLUMNS article_id, date UNIQUE;
DEFINE INDEX article_stats_daily_article_idx ON article_stats_daily COLUMNS article_id;
DEFINE INDEX article_stats_daily_date_idx ON article_stats_daily COLUMNS date;

-- 用户活动日志表
DEFINE TABLE activity_log SCHEMAFULL;
DEFINE FIELD id ON activity_log TYPE record(activity_log);
DEFINE FIELD user_id ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD action ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD resource_type ON activity_log TYPE string ASSERT $value INSIDE ["article", "comment", "user", "publication", "tag"];
DEFINE FIELD resource_id ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD ip_address ON activity_log TYPE option<string>;
DEFINE FIELD user_agent ON activity_log TYPE option<string>;
DEFINE FIELD details ON activity_log TYPE object DEFAULT {};
DEFINE FIELD created_at ON activity_log TYPE datetime DEFAULT time::now();

-- 活动日志索引
DEFINE INDEX activity_log_user_idx ON activity_log COLUMNS user_id;
DEFINE INDEX activity_log_resource_idx ON activity_log COLUMNS resource_type, resource_id;
DEFINE INDEX activity_log_created_idx ON activity_log COLUMNS created_at;
DEFINE INDEX activity_log_action_idx ON activity_log COLUMNS action;

-- =====================================
-- 搜索优化
-- =====================================

-- 搜索索引表
DEFINE TABLE search_index SCHEMAFULL;
DEFINE FIELD id ON search_index TYPE record(search_index);
DEFINE FIELD article_id ON search_index TYPE record(article) ASSERT $value != NONE;
DEFINE FIELD title ON search_index TYPE string;
DEFINE FIELD content ON search_index TYPE string; -- 纯文本内容，用于全文搜索
DEFINE FIELD author_name ON search_index TYPE string;
DEFINE FIELD tags ON search_index TYPE array<string> DEFAULT [];
DEFINE FIELD publication_name ON search_index TYPE option<string>;
DEFINE FIELD is_published ON search_index TYPE bool DEFAULT false;
DEFINE FIELD published_at ON search_index TYPE option<datetime>;
DEFINE FIELD popularity_score ON search_index TYPE number DEFAULT 0; -- 基于互动的流行度分数
DEFINE FIELD updated_at ON search_index TYPE datetime DEFAULT time::now();

-- 搜索索引
DEFINE INDEX search_index_article_idx ON search_index COLUMNS article_id UNIQUE;
DEFINE INDEX search_index_published_idx ON search_index COLUMNS is_published;
DEFINE INDEX search_index_popularity_idx ON search_index COLUMNS popularity_score;

-- =====================================
-- 初始数据
-- =====================================

-- 插入默认标签（热门标签）
INSERT INTO tag (name, slug, description, is_featured) VALUES
    ("Technology", "technology", "Latest in tech and programming", true),
    ("Business", "business", "Business insights and strategies", true),
    ("Health", "health", "Health and wellness topics", true),
    ("Science", "science", "Scientific discoveries and research", true),
    ("Design", "design", "Design trends and inspiration", true),
    ("Politics", "politics", "Political analysis and news", false),
    ("Culture", "culture", "Cultural commentary and arts", false),
    ("Programming", "programming", "Programming tutorials and tips", true),
    ("AI", "ai", "Artificial Intelligence and ML", true),
    ("Startup", "startup", "Startup ecosystem and advice", true);

-- =====================================
-- 存储过程和触发器（SurrealDB特有）
-- =====================================

-- 更新用户统计的函数
DEFINE FUNCTION fn::update_user_stats($user_id: string) {
    -- 更新文章数
    LET $article_count = (SELECT count() FROM article WHERE author_id = $user_id AND status = 'published' AND is_deleted = false);
    
    -- 更新关注者数
    LET $follower_count = (SELECT count() FROM follow WHERE following_id = $user_id);
    
    -- 更新关注数
    LET $following_count = (SELECT count() FROM follow WHERE follower_id = $user_id);
    
    -- 更新总获赞数
    LET $total_claps = (SELECT math::sum(clap_count) FROM article WHERE author_id = $user_id);
    
    -- 更新用户资料
    UPDATE user_profile SET 
        article_count = $article_count,
        follower_count = $follower_count,
        following_count = $following_count,
        total_claps_received = $total_claps,
        updated_at = time::now()
    WHERE user_id = $user_id;
};

-- 更新文章统计的函数
DEFINE FUNCTION fn::update_article_stats($article_id: record(article)) {
    -- 更新评论数
    LET $comment_count = (SELECT count() FROM comment WHERE article_id = $article_id AND is_deleted = false);
    
    -- 更新书签数
    LET $bookmark_count = (SELECT count() FROM bookmark WHERE article_id = $article_id);
    
    -- 更新点赞总数
    LET $clap_count = (SELECT math::sum(count) FROM clap WHERE article_id = $article_id);
    
    -- 更新文章
    UPDATE article SET 
        comment_count = $comment_count,
        bookmark_count = $bookmark_count,
        clap_count = $clap_count,
        updated_at = time::now()
    WHERE id = $article_id;
};

-- 计算文章流行度分数的函数
DEFINE FUNCTION fn::calculate_popularity_score($article_id: record(article)) {
    LET $article = (SELECT * FROM article WHERE id = $article_id);
    
    -- 基于各种互动指标计算分数
    LET $score = (
        $article.view_count * 0.1 +
        $article.clap_count * 1 +
        $article.comment_count * 2 +
        $article.bookmark_count * 3 +
        $article.share_count * 5
    );
    
    -- 时间衰减因子（新文章获得加成）
    LET $age_days = duration::days(time::now() - $article.published_at);
    LET $time_factor = math::max(0.5, 1 - ($age_days / 30));
    
    RETURN $score * $time_factor;
};