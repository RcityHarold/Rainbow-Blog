use crate::{
    error::{AppError, Result},
    models::{
        subscription::*,
        user::UserProfile,
    },
    services::Database,
    config::Config,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn, error};
use validator::Validate;

#[cfg(feature = "payments")]
use stripe::{
    Client as StripeClient,
    CreateSubscription,
    CreatePrice,
    CreateProduct,
    Subscription as StripeSubscription,
    Price as StripePrice,
    Product as StripeProduct,
    EventObject,
    Event as StripeEvent,
};

#[derive(Clone)]
pub struct SubscriptionService {
    db: Arc<Database>,
    #[cfg(feature = "payments")]
    stripe_client: Option<StripeClient>,
}

impl SubscriptionService {
    pub async fn new(db: Arc<Database>, config: &Config) -> Result<Self> {
        #[cfg(feature = "payments")]
        let stripe_client = if config.stripe_secret_key.is_some() {
            Some(StripeClient::new(config.stripe_secret_key.as_ref().unwrap()))
        } else {
            None
        };

        Ok(Self {
            db,
            #[cfg(feature = "payments")]
            stripe_client,
        })
    }

    /// 创建订阅计划
    pub async fn create_subscription_plan(
        &self,
        creator_id: &str,
        request: CreateSubscriptionPlanRequest,
    ) -> Result<SubscriptionPlan> {
        debug!("Creating subscription plan for creator: {}", creator_id);

        // 验证请求
        request.validate().map_err(|e| {
            AppError::Validation(format!("订阅计划数据验证失败: {}", e))
        })?;

        // 检查创作者是否存在
        self.verify_creator_exists(creator_id).await?;

        let plan_id = format!("subscription_plan:{}", uuid::Uuid::new_v4());
        let currency = request.currency.unwrap_or_else(|| "USD".to_string());

        // 创建 Stripe 产品和价格（如果启用支付功能）
        #[cfg(feature = "payments")]
        let stripe_price_id = if let Some(ref stripe_client) = self.stripe_client {
            self.create_stripe_product_and_price(
                &plan_id,
                &request.name,
                request.description.as_deref(),
                request.price,
                &currency,
                stripe_client,
            ).await?
        } else {
            None
        };
        
        #[cfg(not(feature = "payments"))]
        let stripe_price_id: Option<String> = None;

        let query = r#"
            CREATE subscription_plan CONTENT {
                id: $plan_id,
                creator_id: $creator_id,
                name: $name,
                description: $description,
                price: $price,
                currency: $currency,
                benefits: $benefits,
                is_active: true,
                stripe_price_id: $stripe_price_id,
                created_at: time::now(),
                updated_at: time::now()
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "plan_id": plan_id,
            "creator_id": creator_id,
            "name": request.name,
            "description": request.description,
            "price": request.price,
            "currency": currency,
            "benefits": request.benefits,
            "stripe_price_id": stripe_price_id
        })).await?;

        let plans: Vec<Value> = response.take(0)?;
        let plan = plans.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create subscription plan".to_string()))?;

        let subscription_plan = SubscriptionPlan {
            id: plan["id"].as_str().unwrap().to_string(),
            creator_id: plan["creator_id"].as_str().unwrap().to_string(),
            name: plan["name"].as_str().unwrap().to_string(),
            description: plan["description"].as_str().map(|s| s.to_string()),
            price: plan["price"].as_i64().unwrap(),
            currency: plan["currency"].as_str().unwrap().to_string(),
            benefits: plan["benefits"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            is_active: plan["is_active"].as_bool().unwrap_or(true),
            created_at: chrono::DateTime::parse_from_rfc3339(plan["created_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(plan["updated_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
        };

        info!("Subscription plan created: {}", subscription_plan.id);
        Ok(subscription_plan)
    }

    /// 更新订阅计划
    pub async fn update_subscription_plan(
        &self,
        plan_id: &str,
        creator_id: &str,
        request: UpdateSubscriptionPlanRequest,
    ) -> Result<SubscriptionPlan> {
        debug!("Updating subscription plan: {} for creator: {}", plan_id, creator_id);

        // 验证请求
        request.validate().map_err(|e| {
            AppError::Validation(format!("订阅计划更新数据验证失败: {}", e))
        })?;

        // 检查计划是否存在且属于该创作者
        self.verify_plan_ownership(plan_id, creator_id).await?;

        let mut updates = Vec::new();
        if let Some(name) = &request.name {
            updates.push(("name", json!(name)));
        }
        if let Some(description) = &request.description {
            updates.push(("description", json!(description)));
        }
        if let Some(price) = request.price {
            updates.push(("price", json!(price)));
        }
        if let Some(benefits) = &request.benefits {
            updates.push(("benefits", json!(benefits)));
        }
        if let Some(is_active) = request.is_active {
            updates.push(("is_active", json!(is_active)));
        }

        if updates.is_empty() {
            return Err(AppError::Validation("没有提供更新字段".to_string()));
        }

        let set_clause = updates.iter()
            .map(|(field, _)| format!("{} = ${}", field, field))
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(r#"
            UPDATE subscription_plan SET {}, updated_at = time::now()
            WHERE id = $plan_id AND creator_id = $creator_id
            RETURN AFTER
        "#, set_clause);

        let mut params = json!({
            "plan_id": plan_id,
            "creator_id": creator_id
        });

        for (field, value) in updates {
            params[field] = value;
        }

        let mut response = self.db.query_with_params(&query, params).await?;
        let plans: Vec<Value> = response.take(0)?;
        let plan = plans.into_iter().next()
            .ok_or_else(|| AppError::NotFound("订阅计划未找到".to_string()))?;

        Ok(self.parse_subscription_plan(plan)?)
    }

    /// 获取创作者的订阅计划列表
    pub async fn get_creator_plans(
        &self,
        creator_id: &str,
        query: SubscriptionPlanQuery,
    ) -> Result<SubscriptionPlanListResponse> {
        debug!("Getting subscription plans for creator: {}", creator_id);

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let mut where_conditions = vec!["p.creator_id = $creator_id".to_string()];
        let mut query_params = json!({
            "creator_id": creator_id,
            "limit": limit,
            "offset": offset
        });

        if let Some(is_active) = query.is_active {
            where_conditions.push("p.is_active = $is_active".to_string());
            query_params["is_active"] = json!(is_active);
        }

        let where_clause = where_conditions.join(" AND ");

        let query_str = format!(r#"
            SELECT * FROM subscription_plan p
            WHERE {}
            ORDER BY p.created_at DESC
            LIMIT $limit START $offset
        "#, where_clause);

        let count_query = format!(r#"
            SELECT count() as total FROM subscription_plan p WHERE {}
        "#, where_clause);

        let mut response = self.db.query_with_params(&query_str, query_params.clone()).await?;
        let plans: Vec<Value> = response.take(0)?;

        let mut count_response = self.db.query_with_params(&count_query, query_params).await?;
        let count_result: Vec<Value> = count_response.take(0)?;
        let total = count_result.first()
            .and_then(|v| v["total"].as_i64())
            .unwrap_or(0);

        let subscription_plans: Result<Vec<SubscriptionPlan>> = plans.into_iter()
            .map(|plan| self.parse_subscription_plan(plan))
            .collect();

        let subscription_plans = subscription_plans?;
        let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;

        Ok(SubscriptionPlanListResponse {
            plans: subscription_plans,
            total,
            page,
            limit,
            total_pages,
        })
    }

    /// 创建订阅
    pub async fn create_subscription(
        &self,
        subscriber_id: &str,
        request: CreateSubscriptionRequest,
    ) -> Result<SubscriptionDetails> {
        debug!("Creating subscription for user: {}", subscriber_id);

        // 验证请求
        request.validate().map_err(|e| {
            AppError::Validation(format!("订阅数据验证失败: {}", e))
        })?;

        // 获取订阅计划
        let plan = self.get_subscription_plan(&request.plan_id).await?;
        if !plan.is_active {
            return Err(AppError::BadRequest("订阅计划已停用".to_string()));
        }

        // 检查用户是否已经订阅了该创作者
        if self.check_existing_subscription(subscriber_id, &plan.creator_id).await? {
            return Err(AppError::BadRequest("您已经订阅了该创作者".to_string()));
        }

        // 创建 Stripe 订阅（如果启用支付功能）
        #[cfg(feature = "payments")]
        let stripe_subscription_id = if let Some(ref stripe_client) = self.stripe_client {
            if let Some(payment_method_id) = &request.payment_method_id {
                self.create_stripe_subscription(
                    subscriber_id,
                    &plan,
                    payment_method_id,
                    stripe_client,
                ).await?
            } else {
                return Err(AppError::BadRequest("需要提供支付方式".to_string()));
            }
        } else {
            None
        };
        
        #[cfg(not(feature = "payments"))]
        let stripe_subscription_id: Option<String> = None;

        let subscription_id = format!("subscription:{}", uuid::Uuid::new_v4());
        let now = Utc::now();
        let current_period_end = now + Duration::days(30);

        let query = r#"
            CREATE subscription CONTENT {
                id: $subscription_id,
                subscriber_id: $subscriber_id,
                plan_id: $plan_id,
                creator_id: $creator_id,
                status: "active",
                started_at: $started_at,
                current_period_end: $current_period_end,
                stripe_subscription_id: $stripe_subscription_id,
                created_at: time::now(),
                updated_at: time::now()
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscription_id": subscription_id,
            "subscriber_id": subscriber_id,
            "plan_id": request.plan_id,
            "creator_id": plan.creator_id,
            "started_at": now.to_rfc3339(),
            "current_period_end": current_period_end.to_rfc3339(),
            "stripe_subscription_id": stripe_subscription_id
        })).await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        let subscription = subscriptions.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create subscription".to_string()))?;

        let subscription_details = self.build_subscription_details_sync(subscription, plan)?;

        info!("Subscription created: {}", subscription_details.id);
        Ok(subscription_details)
    }

    /// 取消订阅
    pub async fn cancel_subscription(
        &self,
        subscription_id: &str,
        user_id: &str,
    ) -> Result<SubscriptionDetails> {
        debug!("Canceling subscription: {} for user: {}", subscription_id, user_id);

        // 获取订阅信息
        let subscription = self.get_subscription_with_plan(subscription_id).await?;
        
        // 验证用户权限
        if subscription.subscriber_id != user_id {
            return Err(AppError::Authorization("无权限取消该订阅".to_string()));
        }

        if subscription.status == SubscriptionStatus::Canceled {
            return Err(AppError::BadRequest("订阅已经被取消".to_string()));
        }

        // 取消 Stripe 订阅（如果存在）
        #[cfg(feature = "payments")]
        if let Some(ref stripe_client) = self.stripe_client {
            if let Some(stripe_subscription_id) = &subscription.stripe_subscription_id {
                self.cancel_stripe_subscription(stripe_subscription_id, stripe_client).await?;
            }
        }

        let query = r#"
            UPDATE subscription SET 
                status = "canceled",
                canceled_at = time::now(),
                updated_at = time::now()
            WHERE id = $subscription_id
            RETURN AFTER
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscription_id": subscription_id
        })).await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        let updated_subscription = subscriptions.into_iter().next()
            .ok_or_else(|| AppError::NotFound("订阅未找到".to_string()))?;

        let plan = self.get_subscription_plan(&subscription.plan.id).await?;
        let subscription_details = self.build_subscription_details_sync(updated_subscription, plan)?;

        info!("Subscription canceled: {}", subscription_id);
        Ok(subscription_details)
    }

    /// 检查用户订阅状态
    pub async fn check_subscription(
        &self,
        subscriber_id: &str,
        creator_id: &str,
    ) -> Result<SubscriptionCheck> {
        debug!("Checking subscription for user: {} to creator: {}", subscriber_id, creator_id);

        let query = r#"
            SELECT s.*, sp.* FROM subscription s
            JOIN subscription_plan sp ON s.plan_id = sp.id
            WHERE s.subscriber_id = $subscriber_id 
            AND s.creator_id = $creator_id
            AND s.status = "active"
            AND s.current_period_end > time::now()
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscriber_id": subscriber_id,
            "creator_id": creator_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;

        if let Some(result) = results.first() {
            let plan = self.parse_subscription_plan(result.clone())?;
            let subscription_details = self.build_subscription_details_sync(result.clone(), plan)?;
            
            Ok(SubscriptionCheck {
                is_subscribed: true,
                subscription: Some(subscription_details),
                can_access_paid_content: true,
            })
        } else {
            Ok(SubscriptionCheck {
                is_subscribed: false,
                subscription: None,
                can_access_paid_content: false,
            })
        }
    }

    /// 获取用户的订阅列表
    pub async fn get_user_subscriptions(
        &self,
        user_id: &str,
        query: SubscriptionQuery,
    ) -> Result<SubscriptionListResponse> {
        debug!("Getting subscriptions for user: {}", user_id);

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let mut where_conditions = vec!["s.subscriber_id = $user_id".to_string()];
        let mut query_params = json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        });

        if let Some(status) = &query.status {
            where_conditions.push("s.status = $status".to_string());
            query_params["status"] = json!(status.to_string());
        }

        let where_clause = where_conditions.join(" AND ");

        let query_str = format!(r#"
            SELECT s.*, sp.*, up.username, up.display_name, up.avatar_url, up.is_verified
            FROM subscription s
            JOIN subscription_plan sp ON s.plan_id = sp.id
            JOIN user_profile up ON s.creator_id = up.user_id
            WHERE {}
            ORDER BY s.created_at DESC
            LIMIT $limit START $offset
        "#, where_clause);

        let count_query = format!(r#"
            SELECT count() as total FROM subscription s WHERE {}
        "#, where_clause);

        let mut response = self.db.query_with_params(&query_str, query_params.clone()).await?;
        let results: Vec<Value> = response.take(0)?;

        let mut count_response = self.db.query_with_params(&count_query, query_params).await?;
        let count_result: Vec<Value> = count_response.take(0)?;
        let total = count_result.first()
            .and_then(|v| v["total"].as_i64())
            .unwrap_or(0);

        let mut subscriptions = Vec::new();
        for result in results {
            let plan = self.parse_subscription_plan(result.clone())?;
            let subscription_detail = self.build_subscription_details_sync(result, plan)?;
            subscriptions.push(subscription_detail);
        }
        let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;

        Ok(SubscriptionListResponse {
            subscriptions,
            total,
            page,
            limit,
            total_pages,
        })
    }

    /// 获取创作者收益统计
    pub async fn get_creator_revenue(
        &self,
        creator_id: &str,
    ) -> Result<CreatorRevenue> {
        debug!("Getting revenue stats for creator: {}", creator_id);

        // 获取订阅统计
        let stats_query = r#"
            SELECT 
                count() as total_subscribers,
                count(s.id WHERE s.status = "active") as active_subscribers,
                sum(sp.price WHERE s.status = "active") as monthly_revenue
            FROM subscription s
            JOIN subscription_plan sp ON s.plan_id = sp.id
            WHERE s.creator_id = $creator_id
        "#;

        let mut response = self.db.query_with_params(stats_query, json!({
            "creator_id": creator_id
        })).await?;

        let stats: Vec<Value> = response.take(0)?;
        let stat = stats.first().ok_or_else(|| {
            AppError::NotFound("无法获取创作者统计".to_string())
        })?;

        let total_subscribers = stat["total_subscribers"].as_i64().unwrap_or(0);
        let monthly_revenue = stat["monthly_revenue"].as_i64().unwrap_or(0);

        // 获取订阅计划
        let plans = self.get_creator_plans(creator_id, SubscriptionPlanQuery {
            page: Some(1),
            limit: Some(100),
            creator_id: Some(creator_id.to_string()),
            is_active: Some(true),
        }).await?;

        // 获取最近订阅
        let recent_subscriptions = self.get_creator_recent_subscriptions(creator_id, 10).await?;

        // 计算总收益（简化版本，实际应该从历史数据计算）
        let total_revenue = monthly_revenue * 12; // 简化计算

        Ok(CreatorRevenue {
            creator_id: creator_id.to_string(),
            total_subscribers,
            monthly_revenue,
            total_revenue,
            subscription_plans: plans.plans,
            recent_subscriptions,
        })
    }

    // 私有辅助方法
    async fn verify_creator_exists(&self, creator_id: &str) -> Result<()> {
        let query = "SELECT id FROM user_profile WHERE user_id = $creator_id";
        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id
        })).await?;

        let users: Vec<Value> = response.take(0)?;
        if users.is_empty() {
            return Err(AppError::NotFound("创作者不存在".to_string()));
        }
        Ok(())
    }

    async fn verify_plan_ownership(&self, plan_id: &str, creator_id: &str) -> Result<()> {
        let query = "SELECT id FROM subscription_plan WHERE id = $plan_id AND creator_id = $creator_id";
        let mut response = self.db.query_with_params(query, json!({
            "plan_id": plan_id,
            "creator_id": creator_id
        })).await?;

        let plans: Vec<Value> = response.take(0)?;
        if plans.is_empty() {
            return Err(AppError::NotFound("订阅计划不存在或无权限访问".to_string()));
        }
        Ok(())
    }

    pub async fn get_subscription_plan(&self, plan_id: &str) -> Result<SubscriptionPlan> {
        let query = "SELECT * FROM subscription_plan WHERE id = $plan_id";
        let mut response = self.db.query_with_params(query, json!({
            "plan_id": plan_id
        })).await?;

        let plans: Vec<Value> = response.take(0)?;
        let plan = plans.into_iter().next()
            .ok_or_else(|| AppError::NotFound("订阅计划不存在".to_string()))?;

        self.parse_subscription_plan(plan)
    }

    async fn check_existing_subscription(&self, subscriber_id: &str, creator_id: &str) -> Result<bool> {
        let query = r#"
            SELECT id FROM subscription 
            WHERE subscriber_id = $subscriber_id 
            AND creator_id = $creator_id 
            AND status = "active"
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscriber_id": subscriber_id,
            "creator_id": creator_id
        })).await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        Ok(!subscriptions.is_empty())
    }

    pub async fn get_subscription_with_plan(&self, subscription_id: &str) -> Result<SubscriptionDetails> {
        let query = r#"
            SELECT s.*, sp.*, up.username, up.display_name, up.avatar_url, up.is_verified
            FROM subscription s
            JOIN subscription_plan sp ON s.plan_id = sp.id
            JOIN user_profile up ON s.creator_id = up.user_id
            WHERE s.id = $subscription_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscription_id": subscription_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let result = results.into_iter().next()
            .ok_or_else(|| AppError::NotFound("订阅不存在".to_string()))?;

        let plan = self.parse_subscription_plan(result.clone())?;
        self.build_subscription_details_sync(result, plan)
    }

    async fn get_creator_recent_subscriptions(
        &self,
        creator_id: &str,
        limit: i32,
    ) -> Result<Vec<SubscriptionDetails>> {
        let query = r#"
            SELECT s.*, sp.*, up.username, up.display_name, up.avatar_url, up.is_verified
            FROM subscription s
            JOIN subscription_plan sp ON s.plan_id = sp.id
            JOIN user_profile up ON s.creator_id = up.user_id
            WHERE s.creator_id = $creator_id
            ORDER BY s.created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "limit": limit
        })).await?;

        let results: Vec<Value> = response.take(0)?;

        let mut subscriptions = Vec::new();
        for result in results {
            let plan = self.parse_subscription_plan(result.clone())?;
            let subscription_detail = self.build_subscription_details_sync(result, plan)?;
            subscriptions.push(subscription_detail);
        }
        
        Ok(subscriptions)
    }

    fn build_subscription_details_sync(
        &self,
        subscription_data: Value,
        plan: SubscriptionPlan,
    ) -> Result<SubscriptionDetails> {
        let creator = SubscriptionCreator {
            user_id: subscription_data["creator_id"].as_str().unwrap().to_string(),
            username: subscription_data["username"].as_str().unwrap_or("").to_string(),
            display_name: subscription_data["display_name"].as_str().unwrap_or("").to_string(),
            avatar_url: subscription_data["avatar_url"].as_str().map(|s| s.to_string()),
            is_verified: subscription_data["is_verified"].as_bool().unwrap_or(false),
        };

        let status = match subscription_data["status"].as_str().unwrap_or("active") {
            "active" => SubscriptionStatus::Active,
            "canceled" => SubscriptionStatus::Canceled,
            "expired" => SubscriptionStatus::Expired,
            "past_due" => SubscriptionStatus::PastDue,
            _ => SubscriptionStatus::Active,
        };

        Ok(SubscriptionDetails {
            id: subscription_data["id"].as_str().unwrap().to_string(),
            subscriber_id: subscription_data["subscriber_id"].as_str().unwrap().to_string(),
            plan,
            creator,
            status,
            started_at: chrono::DateTime::parse_from_rfc3339(
                subscription_data["started_at"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
            current_period_end: chrono::DateTime::parse_from_rfc3339(
                subscription_data["current_period_end"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
            canceled_at: subscription_data["canceled_at"].as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            created_at: chrono::DateTime::parse_from_rfc3339(
                subscription_data["created_at"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                subscription_data["updated_at"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
        })
    }

    fn parse_subscription_plan(&self, plan_data: Value) -> Result<SubscriptionPlan> {
        Ok(SubscriptionPlan {
            id: plan_data["id"].as_str().unwrap().to_string(),
            creator_id: plan_data["creator_id"].as_str().unwrap().to_string(),
            name: plan_data["name"].as_str().unwrap().to_string(),
            description: plan_data["description"].as_str().map(|s| s.to_string()),
            price: plan_data["price"].as_i64().unwrap(),
            currency: plan_data["currency"].as_str().unwrap().to_string(),
            benefits: plan_data["benefits"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            is_active: plan_data["is_active"].as_bool().unwrap_or(true),
            created_at: chrono::DateTime::parse_from_rfc3339(
                plan_data["created_at"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                plan_data["updated_at"].as_str().unwrap()
            ).unwrap().with_timezone(&Utc),
        })
    }

    // Stripe 集成方法（仅在启用支付功能时编译）
    #[cfg(feature = "payments")]
    async fn create_stripe_product_and_price(
        &self,
        plan_id: &str,
        name: &str,
        description: Option<&str>,
        price: i64,
        currency: &str,
        stripe_client: &StripeClient,
    ) -> Result<Option<String>> {
        use stripe::{CreateProduct, CreatePrice, Currency, Price, Product};

        // 创建产品
        let mut create_product = CreateProduct::new(name);
        if let Some(desc) = description {
            create_product.description = Some(desc);
        }

        let product = Product::create(stripe_client, create_product)
            .await
            .map_err(|e| AppError::ExternalService(format!("Stripe产品创建失败: {}", e)))?;

        // 创建价格
        let currency = currency.parse::<Currency>()
            .map_err(|_| AppError::ValidationError("无效的货币代码".to_string()))?;

        let create_price = CreatePrice {
            currency,
            product: Some(stripe::IdOrObject::Id(product.id.clone())),
            unit_amount: Some(price),
            recurring: Some(stripe::CreatePriceRecurring {
                interval: stripe::CreatePriceRecurringInterval::Month,
                interval_count: None,
                usage_type: None,
            }),
            ..Default::default()
        };

        let price = Price::create(stripe_client, create_price)
            .await
            .map_err(|e| AppError::ExternalService(format!("Stripe价格创建失败: {}", e)))?;

        Ok(Some(price.id.to_string()))
    }

    #[cfg(feature = "payments")]
    async fn create_stripe_subscription(
        &self,
        customer_id: &str,
        plan: &SubscriptionPlan,
        payment_method_id: &str,
        stripe_client: &StripeClient,
    ) -> Result<Option<String>> {
        // 这里需要实现 Stripe 订阅创建逻辑
        // 由于这需要更复杂的 Stripe 集成，暂时返回 None
        warn!("Stripe subscription creation not fully implemented");
        Ok(None)
    }

    #[cfg(feature = "payments")]
    async fn cancel_stripe_subscription(
        &self,
        stripe_subscription_id: &str,
        stripe_client: &StripeClient,
    ) -> Result<()> {
        // 这里需要实现 Stripe 订阅取消逻辑
        warn!("Stripe subscription cancellation not fully implemented");
        Ok(())
    }
}