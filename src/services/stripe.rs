use crate::{
    error::{AppError, Result},
    models::stripe::*,
    services::Database,
};
use chrono::{DateTime, Utc};
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE}};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn, error};
use validator::Validate;

#[derive(Clone)]
pub struct StripeService {
    db: Arc<Database>,
    http_client: Client,
    config: StripeConfig,
}

impl StripeService {
    pub async fn new(db: Arc<Database>, config: StripeConfig) -> Result<Self> {
        let http_client = Client::new();
        
        Ok(Self {
            db,
            http_client,
            config,
        })
    }

    /// 获取Stripe API请求头
    fn get_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.secret_key))
                .unwrap_or_else(|_| HeaderValue::from_static(""))
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/x-www-form-urlencoded"));
        headers.insert("Stripe-Version", HeaderValue::from_str(&self.config.api_version).unwrap());
        headers
    }

    // ============ 客户管理 ============

    /// 创建或获取Stripe客户
    pub async fn get_or_create_customer(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
    ) -> Result<StripeCustomer> {
        debug!("Getting or creating Stripe customer for user: {}", user_id);

        // 首先检查是否已存在
        if let Some(customer) = self.get_customer_by_user_id(user_id).await? {
            return Ok(customer);
        }

        // 创建新的Stripe客户
        let stripe_customer = self.create_stripe_customer(email, name).await?;
        
        // 保存到数据库
        let customer_id = format!("stripe_customer:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE stripe_customer CONTENT {
                id: $customer_id,
                user_id: $user_id,
                stripe_customer_id: $stripe_customer_id,
                email: $email,
                name: $name,
                default_payment_method: NULL,
                created_at: $created_at,
                updated_at: $updated_at
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "customer_id": customer_id,
            "user_id": user_id,
            "stripe_customer_id": stripe_customer["id"],
            "email": email,
            "name": name,
            "created_at": now,
            "updated_at": now
        })).await?;

        let customers: Vec<Value> = response.take(0)?;
        let customer_data = customers.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create customer".to_string()))?;

        Ok(StripeCustomer {
            id: customer_data["id"].as_str().unwrap_or_default().to_string(),
            user_id: user_id.to_string(),
            stripe_customer_id: stripe_customer["id"].as_str().unwrap_or_default().to_string(),
            email: email.to_string(),
            name: name.map(|n| n.to_string()),
            default_payment_method: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// 从数据库获取客户信息
    async fn get_customer_by_user_id(&self, user_id: &str) -> Result<Option<StripeCustomer>> {
        let query = r#"
            SELECT * FROM stripe_customer WHERE user_id = $user_id LIMIT 1
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;

        let customers: Vec<Value> = response.take(0)?;
        
        if let Some(customer_data) = customers.into_iter().next() {
            Ok(Some(StripeCustomer {
                id: customer_data["id"].as_str().unwrap_or_default().to_string(),
                user_id: customer_data["user_id"].as_str().unwrap_or_default().to_string(),
                stripe_customer_id: customer_data["stripe_customer_id"].as_str().unwrap_or_default().to_string(),
                email: customer_data["email"].as_str().unwrap_or_default().to_string(),
                name: customer_data["name"].as_str().map(|s| s.to_string()),
                default_payment_method: customer_data["default_payment_method"].as_str().map(|s| s.to_string()),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    customer_data["created_at"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(
                    customer_data["updated_at"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    /// 在Stripe创建客户
    async fn create_stripe_customer(&self, email: &str, name: Option<&str>) -> Result<Value> {
        let mut params = vec![("email", email)];
        if let Some(name) = name {
            params.push(("name", name));
        }

        let response = self.http_client
            .post("https://api.stripe.com/v1/customers")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe customer creation failed: {}", error_text)));
        }

        let customer: Value = response.json().await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(customer)
    }

    // ============ 支付意图 ============

    /// 创建支付意图
    pub async fn create_payment_intent(
        &self,
        user_id: &str,
        request: CreatePaymentIntentRequest,
    ) -> Result<StripePaymentIntent> {
        request.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
        
        debug!("Creating payment intent for user: {}", user_id);

        // 获取或创建客户
        let customer = self.get_customer_by_user_id(user_id).await?
            .ok_or_else(|| AppError::BadRequest("Customer not found".to_string()))?;

        // 创建Stripe支付意图
        let stripe_intent = self.create_stripe_payment_intent(&customer.stripe_customer_id, &request).await?;

        // 保存到数据库
        let intent_id = format!("payment_intent:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE payment_intent CONTENT {
                id: $intent_id,
                stripe_payment_intent_id: $stripe_intent_id,
                user_id: $user_id,
                amount: $amount,
                currency: $currency,
                status: $status,
                payment_method_id: $payment_method_id,
                article_id: $article_id,
                metadata: $metadata,
                created_at: $created_at,
                updated_at: $updated_at
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "intent_id": intent_id,
            "stripe_intent_id": stripe_intent["id"],
            "user_id": user_id,
            "amount": request.amount,
            "currency": request.currency,
            "status": PaymentIntentStatus::RequiresPaymentMethod,
            "payment_method_id": request.payment_method_id.clone(),
            "article_id": request.article_id.clone(),
            "metadata": request.metadata.clone().unwrap_or_default(),
            "created_at": now,
            "updated_at": now
        })).await?;

        let intents: Vec<Value> = response.take(0)?;
        let intent_data = intents.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create payment intent".to_string()))?;

        Ok(StripePaymentIntent {
            id: intent_data["id"].as_str().unwrap_or_default().to_string(),
            stripe_payment_intent_id: stripe_intent["id"].as_str().unwrap_or_default().to_string(),
            user_id: user_id.to_string(),
            amount: request.amount,
            currency: request.currency,
            status: PaymentIntentStatus::RequiresPaymentMethod,
            payment_method_id: request.payment_method_id,
            article_id: request.article_id,
            metadata: request.metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// 在Stripe创建支付意图
    async fn create_stripe_payment_intent(
        &self,
        customer_id: &str,
        request: &CreatePaymentIntentRequest,
    ) -> Result<Value> {
        let mut params = vec![
            ("amount", request.amount.to_string()),
            ("currency", request.currency.clone()),
            ("customer", customer_id.to_string()),
            ("automatic_payment_methods[enabled]", "true".to_string()),
        ];

        if let Some(payment_method_id) = &request.payment_method_id {
            params.push(("payment_method", payment_method_id.clone()));
        }

        if let Some(confirm) = request.confirm {
            params.push(("confirm", confirm.to_string()));
        }

        let mut metadata_params = Vec::new();
        if let Some(metadata) = &request.metadata {
            if let Some(obj) = metadata.as_object() {
                for (key, value) in obj {
                    let metadata_key = format!("metadata[{}]", key);
                    let metadata_value = value.as_str().unwrap_or("").to_string();
                    metadata_params.push((metadata_key, metadata_value));
                }
            }
        }
        
        // Add metadata params to the main params
        for (key, value) in &metadata_params {
            params.push((key.as_str(), value.clone()));
        }

        let response = self.http_client
            .post("https://api.stripe.com/v1/payment_intents")
            .headers(self.get_headers())
            .form(&params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe payment intent creation failed: {}", error_text)));
        }

        let intent: Value = response.json().await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(intent)
    }

    // ============ 订阅管理 ============

    /// 创建订阅
    pub async fn create_subscription(
        &self,
        user_id: &str,
        request: CreateStripeSubscriptionRequest,
    ) -> Result<StripeSubscription> {
        request.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
        
        debug!("Creating subscription for user: {}", user_id);

        // 获取客户信息
        let customer = self.get_customer_by_user_id(user_id).await?
            .ok_or_else(|| AppError::BadRequest("Customer not found".to_string()))?;

        // 创建Stripe订阅
        let stripe_subscription = self.create_stripe_subscription(&customer.stripe_customer_id, &request).await?;

        // 保存到数据库
        let subscription_id = format!("stripe_subscription:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE stripe_subscription CONTENT {
                id: $subscription_id,
                subscription_id: $internal_subscription_id,
                stripe_subscription_id: $stripe_subscription_id,
                stripe_customer_id: $stripe_customer_id,
                stripe_price_id: $stripe_price_id,
                status: $status,
                current_period_start: $current_period_start,
                current_period_end: $current_period_end,
                cancel_at_period_end: $cancel_at_period_end,
                canceled_at: NULL,
                trial_start: $trial_start,
                trial_end: $trial_end,
                created_at: $created_at,
                updated_at: $updated_at
            }
        "#;

        let current_period_start = DateTime::from_timestamp(
            stripe_subscription["current_period_start"].as_i64().unwrap_or_default(),
            0
        ).unwrap_or_default();
        
        let current_period_end = DateTime::from_timestamp(
            stripe_subscription["current_period_end"].as_i64().unwrap_or_default(),
            0
        ).unwrap_or_default();

        let trial_start = stripe_subscription["trial_start"].as_i64()
            .and_then(|ts| DateTime::from_timestamp(ts, 0));
        let trial_end = stripe_subscription["trial_end"].as_i64()
            .and_then(|ts| DateTime::from_timestamp(ts, 0));

        let mut response = self.db.query_with_params(query, json!({
            "subscription_id": subscription_id,
            "internal_subscription_id": format!("subscription:{}", uuid::Uuid::new_v4()),
            "stripe_subscription_id": stripe_subscription["id"],
            "stripe_customer_id": customer.stripe_customer_id,
            "stripe_price_id": request.price_id,
            "status": StripeSubscriptionStatus::Active,
            "current_period_start": current_period_start,
            "current_period_end": current_period_end,
            "cancel_at_period_end": false,
            "trial_start": trial_start,
            "trial_end": trial_end,
            "created_at": now,
            "updated_at": now
        })).await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        let subscription_data = subscriptions.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create subscription".to_string()))?;

        Ok(StripeSubscription {
            id: subscription_data["id"].as_str().unwrap_or_default().to_string(),
            subscription_id: subscription_data["subscription_id"].as_str().unwrap_or_default().to_string(),
            stripe_subscription_id: stripe_subscription["id"].as_str().unwrap_or_default().to_string(),
            stripe_customer_id: customer.stripe_customer_id,
            stripe_price_id: request.price_id,
            status: StripeSubscriptionStatus::Active,
            current_period_start,
            current_period_end,
            cancel_at_period_end: false,
            canceled_at: None,
            trial_start,
            trial_end,
            created_at: now,
            updated_at: now,
        })
    }

    /// 在Stripe创建订阅
    async fn create_stripe_subscription(
        &self,
        customer_id: &str,
        request: &CreateStripeSubscriptionRequest,
    ) -> Result<Value> {
        let mut params = vec![
            ("customer", customer_id.to_string()),
            ("items[0][price]", request.price_id.clone()),
        ];

        if let Some(payment_method_id) = &request.payment_method_id {
            params.push(("default_payment_method", payment_method_id.clone()));
        }

        if let Some(trial_days) = request.trial_period_days {
            params.push(("trial_period_days", trial_days.to_string()));
        }

        if let Some(coupon) = &request.coupon {
            params.push(("coupon", coupon.clone()));
        }

        let mut metadata_params = Vec::new();
        if let Some(metadata) = &request.metadata {
            if let Some(obj) = metadata.as_object() {
                for (key, value) in obj {
                    let metadata_key = format!("metadata[{}]", key);
                    let metadata_value = value.as_str().unwrap_or("").to_string();
                    metadata_params.push((metadata_key, metadata_value));
                }
            }
        }
        
        // Add metadata params to the main params
        for (key, value) in &metadata_params {
            params.push((key.as_str(), value.clone()));
        }

        let response = self.http_client
            .post("https://api.stripe.com/v1/subscriptions")
            .headers(self.get_headers())
            .form(&params.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe subscription creation failed: {}", error_text)));
        }

        let subscription: Value = response.json().await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(subscription)
    }

    /// 取消订阅
    pub async fn cancel_subscription(&self, subscription_id: &str, at_period_end: bool) -> Result<()> {
        debug!("Canceling subscription: {}", subscription_id);

        // 获取订阅信息
        let subscription = self.get_stripe_subscription_by_id(subscription_id).await?
            .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

        // 在Stripe取消订阅
        self.cancel_stripe_subscription(&subscription.stripe_subscription_id, at_period_end).await?;

        // 更新数据库
        let query = if at_period_end {
            r#"
                UPDATE stripe_subscription SET 
                    cancel_at_period_end = true,
                    updated_at = $updated_at
                WHERE id = $subscription_id
            "#
        } else {
            r#"
                UPDATE stripe_subscription SET 
                    status = $status,
                    canceled_at = $canceled_at,
                    updated_at = $updated_at
                WHERE id = $subscription_id
            "#
        };

        let now = Utc::now();
        let mut params = json!({
            "subscription_id": subscription_id,
            "updated_at": now
        });

        if !at_period_end {
            params["status"] = json!(StripeSubscriptionStatus::Canceled);
            params["canceled_at"] = json!(now);
        }

        self.db.query_with_params(query, params).await?;

        Ok(())
    }

    /// 获取订阅信息
    async fn get_stripe_subscription_by_id(&self, subscription_id: &str) -> Result<Option<StripeSubscription>> {
        let query = r#"
            SELECT * FROM stripe_subscription WHERE id = $subscription_id LIMIT 1
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "subscription_id": subscription_id
        })).await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        
        if let Some(sub_data) = subscriptions.into_iter().next() {
            Ok(Some(StripeSubscription {
                id: sub_data["id"].as_str().unwrap_or_default().to_string(),
                subscription_id: sub_data["subscription_id"].as_str().unwrap_or_default().to_string(),
                stripe_subscription_id: sub_data["stripe_subscription_id"].as_str().unwrap_or_default().to_string(),
                stripe_customer_id: sub_data["stripe_customer_id"].as_str().unwrap_or_default().to_string(),
                stripe_price_id: sub_data["stripe_price_id"].as_str().unwrap_or_default().to_string(),
                status: serde_json::from_value(sub_data["status"].clone()).unwrap_or(StripeSubscriptionStatus::Active),
                current_period_start: chrono::DateTime::parse_from_rfc3339(
                    sub_data["current_period_start"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
                current_period_end: chrono::DateTime::parse_from_rfc3339(
                    sub_data["current_period_end"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
                cancel_at_period_end: sub_data["cancel_at_period_end"].as_bool().unwrap_or(false),
                canceled_at: sub_data["canceled_at"].as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                trial_start: sub_data["trial_start"].as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                trial_end: sub_data["trial_end"].as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    sub_data["created_at"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(
                    sub_data["updated_at"].as_str().unwrap_or_default()
                ).unwrap_or_default().with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    /// 在Stripe取消订阅
    async fn cancel_stripe_subscription(&self, stripe_subscription_id: &str, at_period_end: bool) -> Result<()> {
        let url = format!("https://api.stripe.com/v1/subscriptions/{}", stripe_subscription_id);
        let params = if at_period_end {
            vec![("cancel_at_period_end", "true")]
        } else {
            vec![("cancel_at_period_end", "false")]
        };

        let response = self.http_client
            .post(&url)
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe subscription cancellation failed: {}", error_text)));
        }

        Ok(())
    }

    // ============ Webhook处理 ============

    /// 处理Stripe webhook事件
    pub async fn handle_webhook(&self, event_data: Value) -> Result<()> {
        let event_type = event_data["type"].as_str()
            .ok_or_else(|| AppError::BadRequest("Invalid webhook event type".to_string()))?;

        debug!("Processing Stripe webhook event: {}", event_type);

        // 保存webhook事件
        self.save_webhook_event(&event_data).await?;

        // 根据事件类型处理
        match event_type {
            "payment_intent.succeeded" => self.handle_payment_intent_succeeded(&event_data).await?,
            "payment_intent.payment_failed" => self.handle_payment_intent_failed(&event_data).await?,
            "invoice.payment_succeeded" => self.handle_invoice_payment_succeeded(&event_data).await?,
            "invoice.payment_failed" => self.handle_invoice_payment_failed(&event_data).await?,
            "customer.subscription.updated" => self.handle_subscription_updated(&event_data).await?,
            "customer.subscription.deleted" => self.handle_subscription_deleted(&event_data).await?,
            _ => {
                info!("Unhandled webhook event type: {}", event_type);
            }
        }

        Ok(())
    }

    /// 保存webhook事件
    async fn save_webhook_event(&self, event_data: &Value) -> Result<()> {
        let event_id = format!("webhook_event:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE webhook_event CONTENT {
                id: $event_id,
                stripe_event_id: $stripe_event_id,
                event_type: $event_type,
                processed: false,
                processed_at: NULL,
                data: $data,
                created_at: $created_at
            }
        "#;

        self.db.query_with_params(query, json!({
            "event_id": event_id,
            "stripe_event_id": event_data["id"],
            "event_type": event_data["type"],
            "data": event_data,
            "created_at": now
        })).await?;

        Ok(())
    }

    /// 处理支付意图成功事件
    async fn handle_payment_intent_succeeded(&self, event_data: &Value) -> Result<()> {
        let payment_intent = &event_data["data"]["object"];
        let stripe_payment_intent_id = payment_intent["id"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing payment intent ID".to_string()))?;

        debug!("Handling payment intent succeeded: {}", stripe_payment_intent_id);

        // 更新支付意图状态
        let query = r#"
            UPDATE payment_intent SET 
                status = $status,
                updated_at = $updated_at
            WHERE stripe_payment_intent_id = $stripe_payment_intent_id
        "#;

        self.db.query_with_params(query, json!({
            "status": PaymentIntentStatus::Succeeded,
            "stripe_payment_intent_id": stripe_payment_intent_id,
            "updated_at": Utc::now()
        })).await?;

        Ok(())
    }

    /// 处理支付意图失败事件
    async fn handle_payment_intent_failed(&self, event_data: &Value) -> Result<()> {
        let payment_intent = &event_data["data"]["object"];
        let stripe_payment_intent_id = payment_intent["id"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing payment intent ID".to_string()))?;

        debug!("Handling payment intent failed: {}", stripe_payment_intent_id);

        // 更新支付意图状态
        let query = r#"
            UPDATE payment_intent SET 
                status = $status,
                updated_at = $updated_at
            WHERE stripe_payment_intent_id = $stripe_payment_intent_id
        "#;

        self.db.query_with_params(query, json!({
            "status": PaymentIntentStatus::Canceled,
            "stripe_payment_intent_id": stripe_payment_intent_id,
            "updated_at": Utc::now()
        })).await?;

        Ok(())
    }

    /// 处理发票支付成功事件
    async fn handle_invoice_payment_succeeded(&self, _event_data: &Value) -> Result<()> {
        debug!("Handling invoice payment succeeded");
        // TODO: 实现发票支付成功逻辑
        Ok(())
    }

    /// 处理发票支付失败事件
    async fn handle_invoice_payment_failed(&self, _event_data: &Value) -> Result<()> {
        debug!("Handling invoice payment failed");
        // TODO: 实现发票支付失败逻辑
        Ok(())
    }

    /// 处理订阅更新事件
    async fn handle_subscription_updated(&self, event_data: &Value) -> Result<()> {
        let subscription = &event_data["data"]["object"];
        let stripe_subscription_id = subscription["id"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing subscription ID".to_string()))?;

        debug!("Handling subscription updated: {}", stripe_subscription_id);

        // 更新订阅信息
        let status = subscription["status"].as_str().unwrap_or("active");
        let stripe_status = match status {
            "active" => StripeSubscriptionStatus::Active,
            "past_due" => StripeSubscriptionStatus::PastDue,
            "canceled" => StripeSubscriptionStatus::Canceled,
            "unpaid" => StripeSubscriptionStatus::Unpaid,
            "trialing" => StripeSubscriptionStatus::Trialing,
            "incomplete" => StripeSubscriptionStatus::Incomplete,
            "incomplete_expired" => StripeSubscriptionStatus::IncompleteExpired,
            _ => StripeSubscriptionStatus::Active,
        };

        let query = r#"
            UPDATE stripe_subscription SET 
                status = $status,
                cancel_at_period_end = $cancel_at_period_end,
                updated_at = $updated_at
            WHERE stripe_subscription_id = $stripe_subscription_id
        "#;

        self.db.query_with_params(query, json!({
            "status": stripe_status,
            "cancel_at_period_end": subscription["cancel_at_period_end"].as_bool().unwrap_or(false),
            "stripe_subscription_id": stripe_subscription_id,
            "updated_at": Utc::now()
        })).await?;

        Ok(())
    }

    /// 处理订阅删除事件
    async fn handle_subscription_deleted(&self, event_data: &Value) -> Result<()> {
        let subscription = &event_data["data"]["object"];
        let stripe_subscription_id = subscription["id"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing subscription ID".to_string()))?;

        debug!("Handling subscription deleted: {}", stripe_subscription_id);

        // 更新订阅状态为已取消
        let query = r#"
            UPDATE stripe_subscription SET 
                status = $status,
                canceled_at = $canceled_at,
                updated_at = $updated_at
            WHERE stripe_subscription_id = $stripe_subscription_id
        "#;

        self.db.query_with_params(query, json!({
            "status": StripeSubscriptionStatus::Canceled,
            "canceled_at": Utc::now(),
            "stripe_subscription_id": stripe_subscription_id,
            "updated_at": Utc::now()
        })).await?;

        Ok(())
    }

    // ============ Connect账户管理 (可选) ============

    /// 创建Connect账户
    pub async fn create_connect_account(
        &self,
        user_id: &str,
        request: CreateConnectAccountRequest,
    ) -> Result<StripeConnectAccount> {
        request.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
        
        debug!("Creating Connect account for user: {}", user_id);

        // 创建Stripe Connect账户
        let stripe_account = self.create_stripe_connect_account(&request).await?;

        // 保存到数据库
        let account_id = format!("connect_account:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE connect_account CONTENT {
                id: $account_id,
                user_id: $user_id,
                stripe_account_id: $stripe_account_id,
                account_type: $account_type,
                country: $country,
                currency: $currency,
                details_submitted: $details_submitted,
                charges_enabled: $charges_enabled,
                payouts_enabled: $payouts_enabled,
                requirements: $requirements,
                created_at: $created_at,
                updated_at: $updated_at
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "account_id": account_id,
            "user_id": user_id,
            "stripe_account_id": stripe_account["id"],
            "account_type": request.account_type,
            "country": request.country,
            "currency": "usd", // 默认USD
            "details_submitted": stripe_account["details_submitted"].as_bool().unwrap_or(false),
            "charges_enabled": stripe_account["charges_enabled"].as_bool().unwrap_or(false),
            "payouts_enabled": stripe_account["payouts_enabled"].as_bool().unwrap_or(false),
            "requirements": stripe_account["requirements"].clone(),
            "created_at": now,
            "updated_at": now
        })).await?;

        let accounts: Vec<Value> = response.take(0)?;
        let account_data = accounts.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create Connect account".to_string()))?;

        Ok(StripeConnectAccount {
            id: account_data["id"].as_str().unwrap_or_default().to_string(),
            user_id: user_id.to_string(),
            stripe_account_id: stripe_account["id"].as_str().unwrap_or_default().to_string(),
            account_type: request.account_type,
            country: request.country,
            currency: "usd".to_string(),
            details_submitted: stripe_account["details_submitted"].as_bool().unwrap_or(false),
            charges_enabled: stripe_account["charges_enabled"].as_bool().unwrap_or(false),
            payouts_enabled: stripe_account["payouts_enabled"].as_bool().unwrap_or(false),
            requirements: stripe_account["requirements"].clone(),
            created_at: now,
            updated_at: now,
        })
    }

    /// 在Stripe创建Connect账户
    async fn create_stripe_connect_account(&self, request: &CreateConnectAccountRequest) -> Result<Value> {
        let account_type_str = match request.account_type {
            ConnectAccountType::Express => "express",
            ConnectAccountType::Standard => "standard",
            ConnectAccountType::Custom => "custom",
        };

        let params = vec![
            ("type", account_type_str),
            ("country", &request.country),
            ("email", &request.email),
        ];

        let response = self.http_client
            .post("https://api.stripe.com/v1/accounts")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Stripe Connect account creation failed: {}", error_text)));
        }

        let account: Value = response.json().await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stripe_service_creation() {
        // 模拟测试，实际测试需要有效的数据库连接
        let config = StripeConfig::default();
        assert_eq!(config.api_version, "2023-10-16");
    }
}