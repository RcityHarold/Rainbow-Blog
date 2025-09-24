use crate::{
    error::{AppError, Result},
    models::{
        article::Article,
        payment::*,
        stripe::{CreateStripeIntentRequest, StripeIntentMode},
        subscription::{SubscriptionCheck, SubscriptionStatus},
    },
    services::{
        stripe::{StripePurchaseUpdate, StripeService, StripeSubscriptionStatusUpdate},
        Database, SubscriptionService,
    },
    utils::markdown::MarkdownProcessor,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone)]
pub struct PaymentService {
    db: Arc<Database>,
    subscription_service: Arc<SubscriptionService>,
    stripe_service: Arc<StripeService>,
}

impl PaymentService {
    pub async fn new(
        db: Arc<Database>,
        subscription_service: Arc<SubscriptionService>,
        stripe_service: Arc<StripeService>,
    ) -> Result<Self> {
        Ok(Self {
            db,
            subscription_service,
            stripe_service,
        })
    }

    /// 检查用户对文章的访问权限
    pub async fn check_content_access(
        &self,
        article_id: &str,
        user_id: Option<&str>,
    ) -> Result<ContentAccess> {
        debug!(
            "Checking content access for article: {}, user: {:?}",
            article_id, user_id
        );

        // 获取文章信息
        let article = self.get_article_info(article_id).await?;

        // 如果不是付费内容，允许访问
        if !article.is_paid_content {
            return Ok(ContentAccess {
                article_id: article_id.to_string(),
                user_id: user_id.unwrap_or("").to_string(),
                has_access: true,
                access_type: AccessType::Free,
                subscription_id: None,
                granted_at: Some(Utc::now()),
                expires_at: None,
            });
        }

        // 如果未登录，只能预览
        let Some(user_id) = user_id else {
            return Ok(ContentAccess {
                article_id: article_id.to_string(),
                user_id: "".to_string(),
                has_access: false,
                access_type: AccessType::Preview,
                subscription_id: None,
                granted_at: None,
                expires_at: None,
            });
        };

        // 检查是否是作者本人
        if article.author_id == user_id {
            return Ok(ContentAccess {
                article_id: article_id.to_string(),
                user_id: user_id.to_string(),
                has_access: true,
                access_type: AccessType::Author,
                subscription_id: None,
                granted_at: Some(Utc::now()),
                expires_at: None,
            });
        }

        // 检查订阅状态
        if let Ok(subscription_check) = self
            .subscription_service
            .check_subscription(user_id, &article.author_id)
            .await
        {
            if subscription_check.can_access_paid_content {
                let subscription_id = subscription_check
                    .subscription
                    .as_ref()
                    .map(|s| s.id.clone());
                let expires_at = subscription_check
                    .subscription
                    .as_ref()
                    .map(|s| s.current_period_end);

                return Ok(ContentAccess {
                    article_id: article_id.to_string(),
                    user_id: user_id.to_string(),
                    has_access: true,
                    access_type: AccessType::Subscription,
                    subscription_id,
                    granted_at: Some(Utc::now()),
                    expires_at,
                });
            }
        }

        // 检查单次购买
        if let Ok(purchase) = self.check_article_purchase(article_id, user_id).await {
            if purchase.status == PurchaseStatus::Completed {
                return Ok(ContentAccess {
                    article_id: article_id.to_string(),
                    user_id: user_id.to_string(),
                    has_access: true,
                    access_type: AccessType::OneTime,
                    subscription_id: None,
                    granted_at: Some(purchase.created_at),
                    expires_at: None, // 单次购买永久有效
                });
            }
        }

        // 默认只能预览
        Ok(ContentAccess {
            article_id: article_id.to_string(),
            user_id: user_id.to_string(),
            has_access: false,
            access_type: AccessType::Preview,
            subscription_id: None,
            granted_at: None,
            expires_at: None,
        })
    }

    /// 获取内容预览（用于付费内容）
    pub async fn get_content_preview(
        &self,
        article_id: &str,
        user_id: Option<&str>,
    ) -> Result<ContentPreview> {
        debug!("Getting content preview for article: {}", article_id);

        let article = self.get_article_info(article_id).await?;
        let pricing = self
            .get_article_pricing(article_id)
            .await
            .unwrap_or_else(|_| ArticlePricing {
                article_id: article_id.to_string(),
                is_paid_content: article.is_paid_content,
                price: None,
                subscription_required: true,
                preview_percentage: 30,
                paywall_message: "订阅以继续阅读完整内容".to_string(),
                creator_id: article.author_id.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });

        // 检查访问权限
        let access = self.check_content_access(article_id, user_id).await?;

        if access.has_access {
            // 有访问权限，返回完整内容
            return Ok(ContentPreview {
                article_id: article_id.to_string(),
                preview_content: article.content,
                preview_html: article.content_html,
                is_complete: true,
                paywall_message: String::new(),
                subscription_required: false,
                creator_id: article.author_id,
            });
        }

        // 只能预览，提取预览内容
        let markdown_processor = MarkdownProcessor::new();
        let (preview_content, preview_html) = markdown_processor.extract_preview(
            &article.content,
            &article.content_html,
            pricing.preview_percentage,
        );

        Ok(ContentPreview {
            article_id: article_id.to_string(),
            preview_content,
            preview_html,
            is_complete: false,
            paywall_message: pricing.paywall_message,
            subscription_required: pricing.subscription_required,
            creator_id: article.author_id,
        })
    }

    /// 设置文章定价
    pub async fn set_article_pricing(
        &self,
        article_id: &str,
        creator_id: &str,
        request: ArticlePricingRequest,
    ) -> Result<ArticlePricing> {
        debug!("Setting pricing for article: {}", article_id);

        // 验证请求
        request
            .validate()
            .map_err(|e| AppError::Validation(format!("文章定价设置验证失败: {}", e)))?;

        // 验证作者权限
        self.verify_article_ownership(article_id, creator_id)
            .await?;

        let pricing_id = format!("article_pricing:{}", uuid::Uuid::new_v4());
        let is_paid = request.subscription_required || request.price.is_some();
        let preview_percentage = request.preview_percentage.unwrap_or(30);
        let paywall_message = request
            .paywall_message
            .unwrap_or_else(|| "订阅以继续阅读完整内容".to_string());

        // 更新文章的付费状态
        self.update_article_paid_status(article_id, is_paid).await?;

        // 创建或更新定价信息
        let query = r#"
            UPSERT article_pricing:[$article_id] CONTENT {
                id: $pricing_id,
                article_id: $article_id,
                is_paid_content: $is_paid,
                price: $price,
                subscription_required: $subscription_required,
                preview_percentage: $preview_percentage,
                paywall_message: $paywall_message,
                creator_id: $creator_id,
                created_at: time::now(),
                updated_at: time::now()
            }
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "pricing_id": pricing_id,
                    "article_id": article_id,
                    "is_paid": is_paid,
                    "price": request.price,
                    "subscription_required": request.subscription_required,
                    "preview_percentage": preview_percentage,
                    "paywall_message": paywall_message,
                    "creator_id": creator_id
                }),
            )
            .await?;

        let pricings: Vec<Value> = response.take(0)?;
        let pricing = pricings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to set article pricing".to_string()))?;

        info!("Article pricing set: {}", article_id);
        Ok(self.parse_article_pricing(pricing)?)
    }

    /// 购买单篇文章
    pub async fn purchase_article(
        &self,
        buyer_id: &str,
        buyer_email: &str,
        buyer_display_name: Option<&str>,
        request: ArticlePurchaseRequest,
    ) -> Result<ArticlePurchaseResponse> {
        debug!("Processing article purchase for user: {}", buyer_id);

        // 验证请求
        request
            .validate()
            .map_err(|e| AppError::Validation(format!("文章购买请求验证失败: {}", e)))?;

        // 获取文章和定价信息
        let article = self.get_article_info(&request.article_id).await?;
        let pricing = self.get_article_pricing(&request.article_id).await?;

        if !pricing.is_paid_content {
            return Err(AppError::BadRequest("文章不是付费内容".to_string()));
        }

        let Some(price) = pricing.price else {
            return Err(AppError::BadRequest("文章不支持单次购买".to_string()));
        };

        // 检查是否已经购买
        if let Ok(existing_purchase) = self
            .check_article_purchase(&request.article_id, buyer_id)
            .await
        {
            if existing_purchase.status == PurchaseStatus::Completed {
                return Err(AppError::BadRequest("您已经购买了这篇文章".to_string()));
            }
        }

        // 检查是否已经有订阅访问权限
        if let Ok(subscription_check) = self
            .subscription_service
            .check_subscription(buyer_id, &article.author_id)
            .await
        {
            if subscription_check.can_access_paid_content {
                return Err(AppError::BadRequest(
                    "您已经通过订阅获得访问权限".to_string(),
                ));
            }
        }

        let purchase_id = format!("article_purchase:{}", Uuid::new_v4());
        let currency = "USD".to_string();

        let payment_method_id = if let Some(pm) =
            request.payment_method_id.as_ref().and_then(|pm| {
                let trimmed = pm.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }) {
            Some(pm)
        } else {
            let methods = self.stripe_service.list_payment_methods(buyer_id).await?;
            methods
                .into_iter()
                .find(|m| m.is_default)
                .map(|m| m.stripe_payment_method_id)
        };

        let payment_method_id = payment_method_id
            .ok_or_else(|| AppError::BadRequest("请先添加并设置默认支付方式".to_string()))?;

        let metadata = json!({
            "purchase_id": purchase_id,
            "article_id": request.article_id,
            "creator_id": article.author_id,
            "buyer_id": buyer_id,
        });

        let intent_request = CreateStripeIntentRequest {
            mode: StripeIntentMode::Payment,
            amount: Some(price),
            currency: Some(currency.clone()),
            payment_method_id: Some(payment_method_id.clone()),
            article_id: Some(request.article_id.clone()),
            confirm: Some(false),
            metadata: Some(metadata),
        };

        let payment_intent = self
            .stripe_service
            .create_payment_intent(buyer_id, buyer_email, buyer_display_name, intent_request)
            .await?;

        let stripe_payment_intent_id = payment_intent
            .payment_intent
            .as_ref()
            .map(|intent| intent.stripe_payment_intent_id.clone())
            .ok_or_else(|| AppError::Internal("Stripe 未返回 payment_intent".to_string()))?;

        let query = r#"
            CREATE article_purchase CONTENT {
                id: $purchase_id,
                article_id: $article_id,
                buyer_id: $buyer_id,
                creator_id: $creator_id,
                amount: $amount,
                currency: $currency,
                stripe_payment_intent_id: $stripe_payment_intent_id,
                status: "pending",
                created_at: time::now(),
                updated_at: time::now()
            }
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "purchase_id": &purchase_id,
                    "article_id": &request.article_id,
                    "buyer_id": buyer_id,
                    "creator_id": article.author_id,
                    "amount": price,
                    "currency": currency,
                    "stripe_payment_intent_id": stripe_payment_intent_id,
                }),
            )
            .await?;

        let purchases: Vec<Value> = response.take(0)?;
        let purchase = purchases
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to create article purchase".to_string()))?;

        let purchase = self.parse_article_purchase(purchase)?;

        info!(
            "Article purchase initiated: {} by user: {}",
            request.article_id, buyer_id
        );

        Ok(ArticlePurchaseResponse {
            purchase,
            payment: payment_intent,
        })
    }

    /// 获取付费内容仪表板数据
    pub async fn get_payment_dashboard(&self, creator_id: &str) -> Result<PaymentDashboard> {
        debug!("Getting payment dashboard for creator: {}", creator_id);

        // 获取基本统计
        let stats = self.get_creator_payment_stats(creator_id).await?;

        // 获取收益最高的文章
        let top_earning_articles = self.get_top_earning_articles(creator_id, 10).await?;

        // 获取最近购买记录
        let recent_purchases = self.get_recent_purchases(creator_id, 10).await?;

        // 获取访问统计
        let access_stats = self.get_content_access_stats(creator_id).await?;

        Ok(PaymentDashboard {
            creator_id: creator_id.to_string(),
            total_paid_articles: stats.0,
            total_subscribers: stats.1,
            total_purchases: stats.2,
            monthly_revenue: stats.3,
            top_earning_articles,
            recent_purchases,
            access_stats,
        })
    }

    /// 记录内容访问
    pub async fn record_content_access(
        &self,
        user_id: &str,
        article_id: &str,
        access_type: AccessType,
        reading_time: Option<i64>,
    ) -> Result<()> {
        debug!(
            "Recording content access: user:{}, article:{}",
            user_id, article_id
        );

        let query = r#"
            CREATE user_content_access CONTENT {
                user_id: $user_id,
                article_id: $article_id,
                access_type: $access_type,
                accessed_at: time::now(),
                reading_time: $reading_time,
                completed: $completed
            }
        "#;

        let completed = reading_time.map_or(false, |t| t > 60); // 超过1分钟算完整阅读

        self.db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id,
                    "article_id": article_id,
                    "access_type": match access_type {
                        AccessType::Free => "free",
                        AccessType::Subscription => "subscription",
                        AccessType::OneTime => "one_time",
                        AccessType::Author => "author",
                        AccessType::Preview => "preview",
                    },
                    "reading_time": reading_time,
                    "completed": completed
                }),
            )
            .await?;

        Ok(())
    }

    // 私有辅助方法
    async fn get_article_info(&self, article_id: &str) -> Result<Article> {
        let query = "SELECT * FROM article WHERE id = $article_id";
        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "article_id": article_id
                }),
            )
            .await?;

        let articles: Vec<Value> = response.take(0)?;
        let article = articles
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("文章不存在".to_string()))?;

        Ok(Article {
            id: article["id"].as_str().unwrap().to_string(),
            title: article["title"].as_str().unwrap().to_string(),
            slug: article["slug"].as_str().unwrap().to_string(),
            content: article["content"].as_str().unwrap_or("").to_string(),
            content_html: article["content_html"].as_str().unwrap_or("").to_string(),
            author_id: article["author_id"].as_str().unwrap().to_string(),
            is_paid_content: article["is_paid_content"].as_bool().unwrap_or(false),
            status: match article["status"].as_str().unwrap_or("draft") {
                "published" => crate::models::article::ArticleStatus::Published,
                "unlisted" => crate::models::article::ArticleStatus::Unlisted,
                "archived" => crate::models::article::ArticleStatus::Archived,
                _ => crate::models::article::ArticleStatus::Draft,
            },
            created_at: chrono::DateTime::parse_from_rfc3339(
                article["created_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                article["updated_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
            published_at: article["published_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            // 其他字段使用默认值
            subtitle: None,
            excerpt: None,
            cover_image_url: None,
            publication_id: None,
            series_id: None,
            series_order: None,
            is_featured: false,
            reading_time: 0,
            word_count: 0,
            view_count: 0,
            clap_count: 0,
            comment_count: 0,
            bookmark_count: 0,
            share_count: 0,
            seo_title: None,
            seo_description: None,
            seo_keywords: vec![],
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            last_edited_at: None,
            is_deleted: false,
            deleted_at: None,
        })
    }

    pub async fn get_article_pricing(&self, article_id: &str) -> Result<ArticlePricing> {
        let query = "SELECT * FROM article_pricing WHERE article_id = $article_id";
        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "article_id": article_id
                }),
            )
            .await?;

        let pricings: Vec<Value> = response.take(0)?;
        let pricing = pricings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("文章定价信息不存在".to_string()))?;

        self.parse_article_pricing(pricing)
    }

    async fn check_article_purchase(
        &self,
        article_id: &str,
        buyer_id: &str,
    ) -> Result<ArticlePurchase> {
        let query = r#"
            SELECT * FROM article_purchase 
            WHERE article_id = $article_id AND buyer_id = $buyer_id
            ORDER BY created_at DESC LIMIT 1
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "article_id": article_id,
                    "buyer_id": buyer_id
                }),
            )
            .await?;

        let purchases: Vec<Value> = response.take(0)?;
        let purchase = purchases
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("购买记录不存在".to_string()))?;

        self.parse_article_purchase(purchase)
    }

    async fn verify_article_ownership(&self, article_id: &str, creator_id: &str) -> Result<()> {
        let query = "SELECT id FROM article WHERE id = $article_id AND author_id = $creator_id";
        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "article_id": article_id,
                    "creator_id": creator_id
                }),
            )
            .await?;

        let articles: Vec<Value> = response.take(0)?;
        if articles.is_empty() {
            return Err(AppError::Authorization("您无权限修改此文章".to_string()));
        }
        Ok(())
    }

    async fn update_article_paid_status(&self, article_id: &str, is_paid: bool) -> Result<()> {
        let query = "UPDATE article SET is_paid_content = $is_paid, updated_at = time::now() WHERE id = $article_id";
        self.db
            .query_with_params(
                query,
                json!({
                    "article_id": article_id,
                    "is_paid": is_paid
                }),
            )
            .await?;
        Ok(())
    }

    async fn complete_purchase(&self, purchase_id: &str) -> Result<ArticlePurchase> {
        let query = r#"
            UPDATE article_purchase SET 
                status = "completed",
                updated_at = time::now()
            WHERE id = $purchase_id
            RETURN AFTER
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "purchase_id": purchase_id
                }),
            )
            .await?;

        let purchases: Vec<Value> = response.take(0)?;
        let purchase = purchases
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to complete purchase".to_string()))?;

        self.parse_article_purchase(purchase)
    }

    pub async fn handle_stripe_purchase_success(
        &self,
        update: &StripePurchaseUpdate,
    ) -> Result<()> {
        debug!(
            "Reconciling Stripe purchase intent: {}",
            update.stripe_payment_intent_id
        );

        let mut purchase_id = if let Some(id) = &update.purchase_id {
            self.ensure_purchase_exists(id).await?;
            Some(id.clone())
        } else {
            None
        };

        if purchase_id.is_none() {
            if let Some(existing) = self
                .find_purchase_by_intent(&update.stripe_payment_intent_id)
                .await?
            {
                purchase_id = Some(existing.id);
            }
        }

        if purchase_id.is_none() {
            let new_id = self.create_purchase_from_stripe(update).await?;
            purchase_id = Some(new_id);
        }

        let purchase_id = purchase_id.expect("purchase id must be resolved");

        let _ = self.mark_purchase_completed(&purchase_id, update).await?;

        self.grant_paid_access(
            &update.buyer_id,
            &update.article_id,
            AccessType::OneTime,
            Some(&purchase_id),
            None,
        )
        .await?;

        Ok(())
    }

    pub async fn handle_subscription_status_update(
        &self,
        update: &StripeSubscriptionStatusUpdate,
    ) -> Result<()> {
        debug!(
            "Syncing subscription access for user: {} -> creator: {} ({:?})",
            update.subscriber_id, update.creator_id, update.status
        );

        match update.status {
            SubscriptionStatus::Active | SubscriptionStatus::PastDue => {
                self.grant_subscription_access_for_creator(
                    &update.subscriber_id,
                    &update.creator_id,
                    &update.subscription_id,
                    update.current_period_end,
                )
                .await?;
            }
            SubscriptionStatus::Canceled | SubscriptionStatus::Expired => {
                self.revoke_subscription_access_for_creator(
                    &update.subscriber_id,
                    &update.creator_id,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn ensure_purchase_exists(&self, purchase_id: &str) -> Result<()> {
        let mut response = self
            .db
            .query_with_params(
                "SELECT id FROM article_purchase WHERE id = $purchase_id LIMIT 1",
                json!({ "purchase_id": purchase_id }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        if records.is_empty() {
            return Err(AppError::NotFound("购买记录不存在".to_string()));
        }
        Ok(())
    }

    async fn mark_purchase_completed(
        &self,
        purchase_id: &str,
        update: &StripePurchaseUpdate,
    ) -> Result<ArticlePurchase> {
        self
            .db
            .query_with_params(
                "UPDATE article_purchase SET stripe_payment_intent_id = $intent_id, amount = $amount, currency = $currency, updated_at = time::now() WHERE id = $purchase_id",
                json!({
                    "purchase_id": purchase_id,
                    "intent_id": update.stripe_payment_intent_id,
                    "amount": update.amount,
                    "currency": update.currency,
                }),
            )
            .await?;

        self.complete_purchase(purchase_id).await
    }

    async fn find_purchase_by_intent(
        &self,
        stripe_payment_intent_id: &str,
    ) -> Result<Option<ArticlePurchase>> {
        let mut response = self
            .db
            .query_with_params(
                "SELECT * FROM article_purchase WHERE stripe_payment_intent_id = $intent_id LIMIT 1",
                json!({ "intent_id": stripe_payment_intent_id }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        if let Some(record) = records.into_iter().next() {
            return self.parse_article_purchase(record).map(Some);
        }

        Ok(None)
    }

    async fn create_purchase_from_stripe(&self, update: &StripePurchaseUpdate) -> Result<String> {
        let purchase_id = format!("article_purchase:{}", Uuid::new_v4());

        self.db
            .query_with_params(
                "CREATE article_purchase CONTENT {
                    id: $purchase_id,
                    article_id: $article_id,
                    buyer_id: $buyer_id,
                    creator_id: $creator_id,
                    amount: $amount,
                    currency: $currency,
                    stripe_payment_intent_id: $intent_id,
                    status: 'pending',
                    created_at: time::now(),
                    updated_at: time::now()
                }",
                json!({
                    "purchase_id": &purchase_id,
                    "article_id": update.article_id,
                    "buyer_id": update.buyer_id,
                    "creator_id": update.creator_id,
                    "amount": update.amount,
                    "currency": update.currency,
                    "intent_id": update.stripe_payment_intent_id,
                }),
            )
            .await?;

        Ok(purchase_id)
    }

    pub async fn grant_paid_access(
        &self,
        user_id: &str,
        article_id: &str,
        access_type: AccessType,
        payment_id: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let access_type_str = match access_type {
            AccessType::Subscription => "subscription",
            _ => "one_time_purchase",
        };

        self.db
            .query_with_params(
                "DELETE paid_content_access WHERE user_id = $user_id AND article_id = $article_id",
                json!({
                    "user_id": user_id,
                    "article_id": article_id,
                }),
            )
            .await?;

        self.db
            .query_with_params(
                "CREATE paid_content_access CONTENT {
                    id: $access_id,
                    user_id: $user_id,
                    article_id: $article_id,
                    access_type: $access_type,
                    payment_id: $payment_id,
                    expires_at: $expires_at,
                    created_at: time::now()
                }",
                json!({
                    "access_id": format!("paid_content_access:{}", Uuid::new_v4()),
                    "user_id": user_id,
                    "article_id": article_id,
                    "access_type": access_type_str,
                    "payment_id": payment_id,
                    "expires_at": expires_at,
                }),
            )
            .await?;

        Ok(())
    }

    async fn grant_subscription_access_for_creator(
        &self,
        subscriber_id: &str,
        creator_id: &str,
        subscription_id: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let articles = self.fetch_paid_articles_by_creator(creator_id).await?;
        for article_id in articles {
            self.grant_paid_access(
                subscriber_id,
                &article_id,
                AccessType::Subscription,
                Some(subscription_id),
                expires_at,
            )
            .await?;
        }
        Ok(())
    }

    async fn revoke_subscription_access_for_creator(
        &self,
        subscriber_id: &str,
        creator_id: &str,
    ) -> Result<()> {
        let articles = self.fetch_paid_articles_by_creator(creator_id).await?;
        for article_id in articles {
            self
                .db
                .query_with_params(
                    "DELETE paid_content_access WHERE user_id = $user_id AND article_id = $article_id AND access_type = 'subscription'",
                    json!({
                        "user_id": subscriber_id,
                        "article_id": article_id,
                    }),
                )
                .await?;
        }
        Ok(())
    }

    async fn fetch_paid_articles_by_creator(&self, creator_id: &str) -> Result<Vec<String>> {
        let mut response = self
            .db
            .query_with_params(
                "SELECT id FROM article WHERE author_id = $creator_id AND is_paid_content = true",
                json!({ "creator_id": creator_id }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        let articles = records
            .into_iter()
            .filter_map(|record| {
                record
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(articles)
    }

    async fn get_creator_payment_stats(&self, creator_id: &str) -> Result<(i64, i64, i64, i64)> {
        let query = r#"
            SELECT 
                count(DISTINCT ap.article_id) as total_paid_articles,
                count(DISTINCT s.subscriber_id) as total_subscribers,
                count(DISTINCT pur.id) as total_purchases,
                sum(pur.amount) as total_revenue
            FROM article a
            LEFT JOIN article_pricing ap ON a.id = ap.article_id
            LEFT JOIN subscription s ON a.author_id = s.creator_id AND s.status = 'active'
            LEFT JOIN article_purchase pur ON a.id = pur.article_id AND pur.status = 'completed'
            WHERE a.author_id = $creator_id AND ap.is_paid_content = true
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "creator_id": creator_id
                }),
            )
            .await?;

        let stats: Vec<Value> = response.take(0)?;
        let stat = stats
            .first()
            .ok_or_else(|| AppError::Internal("Failed to get payment stats".to_string()))?;

        Ok((
            stat["total_paid_articles"].as_i64().unwrap_or(0),
            stat["total_subscribers"].as_i64().unwrap_or(0),
            stat["total_purchases"].as_i64().unwrap_or(0),
            stat["total_revenue"].as_i64().unwrap_or(0),
        ))
    }

    async fn get_top_earning_articles(
        &self,
        creator_id: &str,
        limit: usize,
    ) -> Result<Vec<ArticleEarnings>> {
        let query = format!(
            r#"
            SELECT 
                a.id as article_id,
                a.title,
                a.slug,
                sum(pur.amount) as total_revenue,
                count(pur.id) as purchase_count,
                a.view_count
            FROM article a
            LEFT JOIN article_purchase pur ON a.id = pur.article_id AND pur.status = 'completed'
            WHERE a.author_id = $creator_id AND a.is_paid_content = true
            GROUP BY a.id, a.title, a.slug, a.view_count
            ORDER BY total_revenue DESC
            LIMIT {}
        "#,
            limit
        );

        let mut response = self
            .db
            .query_with_params(
                &query,
                json!({
                    "creator_id": creator_id
                }),
            )
            .await?;

        let results: Vec<Value> = response.take(0)?;

        let mut earnings = Vec::new();
        for result in results {
            earnings.push(ArticleEarnings {
                article_id: result["article_id"].as_str().unwrap().to_string(),
                title: result["title"].as_str().unwrap().to_string(),
                slug: result["slug"].as_str().unwrap().to_string(),
                total_revenue: result["total_revenue"].as_i64().unwrap_or(0),
                subscription_revenue: 0, // TODO: 计算订阅收益
                purchase_revenue: result["total_revenue"].as_i64().unwrap_or(0),
                view_count: result["view_count"].as_i64().unwrap_or(0),
                purchase_count: result["purchase_count"].as_i64().unwrap_or(0),
            });
        }

        Ok(earnings)
    }

    async fn get_recent_purchases(
        &self,
        creator_id: &str,
        limit: usize,
    ) -> Result<Vec<ArticlePurchase>> {
        let query = format!(
            r#"
            SELECT * FROM article_purchase 
            WHERE creator_id = $creator_id 
            ORDER BY created_at DESC 
            LIMIT {}
        "#,
            limit
        );

        let mut response = self
            .db
            .query_with_params(
                &query,
                json!({
                    "creator_id": creator_id
                }),
            )
            .await?;

        let results: Vec<Value> = response.take(0)?;

        let mut purchases = Vec::new();
        for result in results {
            purchases.push(self.parse_article_purchase(result)?);
        }

        Ok(purchases)
    }

    async fn get_content_access_stats(&self, creator_id: &str) -> Result<Vec<ContentAccessStats>> {
        let query = r#"
            SELECT 
                a.id as article_id,
                count(uca.user_id) as total_views,
                count(uca.user_id WHERE uca.access_type = 'free') as free_views,
                count(uca.user_id WHERE uca.access_type = 'subscription') as subscription_views,
                count(uca.user_id WHERE uca.access_type = 'one_time') as purchase_views,
                count(uca.user_id WHERE uca.access_type = 'preview') as preview_views
            FROM article a
            LEFT JOIN user_content_access uca ON a.id = uca.article_id
            WHERE a.author_id = $creator_id AND a.is_paid_content = true
            GROUP BY a.id
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "creator_id": creator_id
                }),
            )
            .await?;

        let results: Vec<Value> = response.take(0)?;

        let mut stats = Vec::new();
        for result in results {
            let total_views = result["total_views"].as_i64().unwrap_or(0);
            let preview_views = result["preview_views"].as_i64().unwrap_or(0);
            let paid_views = result["subscription_views"].as_i64().unwrap_or(0)
                + result["purchase_views"].as_i64().unwrap_or(0);

            let conversion_rate = if preview_views > 0 {
                (paid_views as f64 / preview_views as f64) * 100.0
            } else {
                0.0
            };

            stats.push(ContentAccessStats {
                article_id: result["article_id"].as_str().unwrap().to_string(),
                total_views,
                free_views: result["free_views"].as_i64().unwrap_or(0),
                subscription_views: result["subscription_views"].as_i64().unwrap_or(0),
                purchase_views: result["purchase_views"].as_i64().unwrap_or(0),
                preview_views,
                conversion_rate,
                total_revenue: 0, // TODO: 计算收益
            });
        }

        Ok(stats)
    }

    fn parse_article_pricing(&self, pricing_data: Value) -> Result<ArticlePricing> {
        Ok(ArticlePricing {
            article_id: pricing_data["article_id"].as_str().unwrap().to_string(),
            is_paid_content: pricing_data["is_paid_content"].as_bool().unwrap_or(false),
            price: pricing_data["price"].as_i64(),
            subscription_required: pricing_data["subscription_required"]
                .as_bool()
                .unwrap_or(false),
            preview_percentage: pricing_data["preview_percentage"].as_u64().unwrap_or(30) as u8,
            paywall_message: pricing_data["paywall_message"]
                .as_str()
                .unwrap_or("订阅以继续阅读")
                .to_string(),
            creator_id: pricing_data["creator_id"].as_str().unwrap().to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339(
                pricing_data["created_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                pricing_data["updated_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
        })
    }

    fn parse_article_purchase(&self, purchase_data: Value) -> Result<ArticlePurchase> {
        let status = match purchase_data["status"].as_str().unwrap_or("pending") {
            "pending" => PurchaseStatus::Pending,
            "completed" => PurchaseStatus::Completed,
            "failed" => PurchaseStatus::Failed,
            "refunded" => PurchaseStatus::Refunded,
            _ => PurchaseStatus::Pending,
        };

        Ok(ArticlePurchase {
            id: purchase_data["id"].as_str().unwrap().to_string(),
            article_id: purchase_data["article_id"].as_str().unwrap().to_string(),
            buyer_id: purchase_data["buyer_id"].as_str().unwrap().to_string(),
            creator_id: purchase_data["creator_id"].as_str().unwrap().to_string(),
            amount: purchase_data["amount"].as_i64().unwrap(),
            currency: purchase_data["currency"]
                .as_str()
                .unwrap_or("USD")
                .to_string(),
            stripe_payment_intent_id: purchase_data["stripe_payment_intent_id"]
                .as_str()
                .map(|s| s.to_string()),
            status,
            created_at: chrono::DateTime::parse_from_rfc3339(
                purchase_data["created_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                purchase_data["updated_at"].as_str().unwrap(),
            )
            .unwrap()
            .with_timezone(&Utc),
        })
    }
}
