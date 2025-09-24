use crate::{
    error::{AppError, Result},
    models::{
        payment::AccessType, revenue::RevenueSourceType, stripe::*,
        subscription::SubscriptionStatus,
    },
    services::Database,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde_json::{json, Map, Value};
use sha2::Sha256;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use validator::Validate;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Default)]
pub struct StripeWebhookOutcome {
    pub purchase_updates: Vec<StripePurchaseUpdate>,
    pub subscription_revenues: Vec<StripeSubscriptionRevenue>,
    pub subscription_status_updates: Vec<StripeSubscriptionStatusUpdate>,
}

#[derive(Debug)]
struct SavedWebhookEvent {
    id: String,
    already_processed: bool,
}

#[derive(Debug, Clone)]
pub struct StripePurchaseUpdate {
    pub stripe_payment_intent_id: String,
    pub buyer_id: String,
    pub creator_id: String,
    pub article_id: String,
    pub purchase_id: Option<String>,
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct StripeSubscriptionRevenue {
    pub subscription_id: String,
    pub creator_id: String,
    pub subscriber_id: String,
    pub amount: i64,
    pub currency: String,
    pub current_period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct StripeSubscriptionStatusUpdate {
    pub subscription_id: String,
    pub creator_id: String,
    pub subscriber_id: String,
    pub status: SubscriptionStatus,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: Option<bool>,
    pub canceled_at: Option<DateTime<Utc>>,
}

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

    /// 验证 Stripe Webhook 签名
    pub async fn verify_webhook_signature(
        &self,
        payload: &str,
        signature_header: &str,
    ) -> Result<()> {
        let secret = self.config.webhook_endpoint_secret.trim().to_string();

        if secret.is_empty() {
            return Err(AppError::ServiceUnavailable(
                "未配置 Stripe Webhook Secret，请联系管理员".to_string(),
            ));
        }

        let mut timestamp: Option<i64> = None;
        let mut signatures: Vec<&str> = Vec::new();

        for part in signature_header.split(',') {
            let mut iter = part.trim().splitn(2, '=');
            let key = iter.next().unwrap_or("");
            let value = iter.next().unwrap_or("");
            match key {
                "t" => {
                    timestamp = value.parse::<i64>().ok();
                }
                "v1" => signatures.push(value),
                _ => {}
            }
        }

        let timestamp = timestamp
            .ok_or_else(|| AppError::BadRequest("Stripe Webhook 签名缺少时间戳".to_string()))?;

        // 防止重放攻击：时间戳与当前时间差超过5分钟则拒绝
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppError::Internal("系统时间异常".to_string()))?
            .as_secs() as i64;

        if (current_timestamp - timestamp).abs() > 300 {
            return Err(AppError::Authorization(
                "Stripe Webhook 已过期，请重试".to_string(),
            ));
        }

        if signatures.is_empty() {
            return Err(AppError::BadRequest(
                "Stripe Webhook 签名缺少 v1 值".to_string(),
            ));
        }

        let signed_payload = format!("{}.{}", timestamp, payload);

        for signature in signatures {
            if let Ok(expected) = hex::decode(signature) {
                if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
                    mac.update(signed_payload.as_bytes());
                    if mac.verify_slice(&expected).is_ok() {
                        return Ok(());
                    }
                }
            }
        }

        Err(AppError::Authorization(
            "Stripe Webhook 签名验证失败".to_string(),
        ))
    }

    /// 获取Stripe API请求头
    fn get_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.secret_key))
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            "Stripe-Version",
            HeaderValue::from_str(&self.config.api_version).unwrap(),
        );
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

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "customer_id": customer_id,
                    "user_id": user_id,
                    "stripe_customer_id": stripe_customer["id"],
                    "email": email,
                    "name": name,
                    "created_at": now,
                    "updated_at": now
                }),
            )
            .await?;

        let customers: Vec<Value> = response.take(0)?;
        let customer_data = customers
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to create customer".to_string()))?;

        Ok(StripeCustomer {
            id: customer_data["id"].as_str().unwrap_or_default().to_string(),
            user_id: user_id.to_string(),
            stripe_customer_id: stripe_customer["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
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

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;

        let customers: Vec<Value> = response.take(0)?;

        if let Some(customer_data) = customers.into_iter().next() {
            Ok(Some(StripeCustomer {
                id: customer_data["id"].as_str().unwrap_or_default().to_string(),
                user_id: customer_data["user_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                stripe_customer_id: customer_data["stripe_customer_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                email: customer_data["email"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                name: customer_data["name"].as_str().map(|s| s.to_string()),
                default_payment_method: customer_data["default_payment_method"]
                    .as_str()
                    .map(|s| s.to_string()),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    customer_data["created_at"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(
                    customer_data["updated_at"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
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

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/customers")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe customer creation failed: {}",
                error_text
            )));
        }

        let customer: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(customer)
    }

    // ============ 支付意图 ============

    /// 创建支付意图
    pub async fn create_payment_intent(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
        request: CreateStripeIntentRequest,
    ) -> Result<StripeIntentResponse> {
        debug!("Creating Stripe intent for user: {}", user_id);

        let CreateStripeIntentRequest {
            mode,
            amount,
            currency,
            payment_method_id,
            article_id,
            confirm,
            metadata,
        } = request;

        let customer = self.get_or_create_customer(user_id, email, name).await?;

        let mut metadata_map =
            Self::prepare_intent_metadata(metadata, user_id, article_id.as_deref())?;

        match mode {
            StripeIntentMode::Setup => {
                let setup_intent = self
                    .create_stripe_setup_intent(&customer.stripe_customer_id, &metadata_map)
                    .await?;

                let client_secret = setup_intent
                    .get("client_secret")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AppError::Internal("Stripe SetupIntent missing client_secret".to_string())
                    })?
                    .to_string();

                let setup_intent_id = setup_intent
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                Ok(StripeIntentResponse {
                    mode: StripeIntentMode::Setup,
                    client_secret,
                    payment_intent: None,
                    setup_intent_id,
                })
            }
            StripeIntentMode::Payment => {
                let amount = amount.ok_or_else(|| {
                    AppError::BadRequest("Payment intent requires a valid amount".to_string())
                })?;

                if amount < 50 {
                    return Err(AppError::BadRequest(
                        "Payment amount must be at least 50 (cents)".to_string(),
                    ));
                }

                let mut currency = currency.unwrap_or_else(|| "USD".to_string());
                currency.make_ascii_uppercase();
                if currency.len() != 3 {
                    return Err(AppError::BadRequest(
                        "Currency must be a 3-letter ISO code".to_string(),
                    ));
                }

                let stripe_intent = self
                    .create_stripe_payment_intent(
                        &customer.stripe_customer_id,
                        amount,
                        &currency,
                        payment_method_id.as_deref(),
                        confirm,
                        &metadata_map,
                    )
                    .await?;

                let status_str = stripe_intent
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("requires_payment_method");
                let status = Self::map_payment_intent_status(status_str);

                let client_secret = stripe_intent
                    .get("client_secret")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AppError::Internal("Stripe PaymentIntent missing client_secret".to_string())
                    })?
                    .to_string();

                metadata_map.insert(
                    "mode".to_string(),
                    serde_json::Value::String("payment".to_string()),
                );
                let metadata_value = serde_json::Value::Object(metadata_map.clone());

                let intent_id = format!("payment_intent:{}", uuid::Uuid::new_v4());
                let now = Utc::now();

                let mut response = self
                    .db
                    .query_with_params(
                        r#"
            CREATE payment_intent CONTENT $content
        "#,
                        json!({
                            "content": {
                                "id": intent_id,
                                "stripe_payment_intent_id": stripe_intent["id"],
                                "user_id": user_id,
                                "amount": amount,
                                "currency": currency,
                                "status": status_str,
                                "mode": StripeIntentMode::Payment,
                                "payment_method_id": payment_method_id,
                                "article_id": article_id,
                                "metadata": metadata_value,
                                "created_at": now,
                                "updated_at": now,
                            }
                        }),
                    )
                    .await?;

                let intents: Vec<Value> = response.take(0)?;
                let intent_record = intents.into_iter().next().ok_or_else(|| {
                    AppError::Internal("Failed to persist payment intent".to_string())
                })?;

                let mut payment_intent = self
                    .build_payment_intent_from_record(intent_record, Some(client_secret.clone()))?;
                payment_intent.status = status;

                Ok(StripeIntentResponse {
                    mode: StripeIntentMode::Payment,
                    client_secret,
                    payment_intent: Some(payment_intent),
                    setup_intent_id: None,
                })
            }
        }
    }

    /// 在 Stripe 创建支付意图
    async fn create_stripe_payment_intent(
        &self,
        customer_id: &str,
        amount: i64,
        currency: &str,
        payment_method_id: Option<&str>,
        confirm: Option<bool>,
        metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Value> {
        let mut params: Vec<(String, String)> = vec![
            ("amount".to_string(), amount.to_string()),
            ("currency".to_string(), currency.to_string()),
            ("customer".to_string(), customer_id.to_string()),
            (
                "automatic_payment_methods[enabled]".to_string(),
                "true".to_string(),
            ),
        ];

        if let Some(payment_method_id) = payment_method_id {
            params.push(("payment_method".to_string(), payment_method_id.to_string()));
        }

        if let Some(confirm) = confirm {
            params.push(("confirm".to_string(), confirm.to_string()));
        }

        for (key, value) in metadata {
            let meta_key = format!("metadata[{}]", key);
            let meta_value = if let Some(s) = value.as_str() {
                s.to_string()
            } else {
                value.to_string()
            };
            params.push((meta_key, meta_value));
        }

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/payment_intents")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe payment intent creation failed: {}",
                error_text
            )));
        }

        let intent: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(intent)
    }

    /// 创建 Stripe SetupIntent
    async fn create_stripe_setup_intent(
        &self,
        customer_id: &str,
        metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Value> {
        let mut params: Vec<(String, String)> = vec![
            ("customer".to_string(), customer_id.to_string()),
            ("usage".to_string(), "off_session".to_string()),
            ("payment_method_types[]".to_string(), "card".to_string()),
        ];

        for (key, value) in metadata {
            let meta_key = format!("metadata[{}]", key);
            let meta_value = if let Some(s) = value.as_str() {
                s.to_string()
            } else {
                value.to_string()
            };
            params.push((meta_key, meta_value));
        }

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/setup_intents")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe setup intent creation failed: {}",
                error_text
            )));
        }

        let intent: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(intent)
    }

    fn prepare_intent_metadata(
        metadata: Option<serde_json::Value>,
        user_id: &str,
        article_id: Option<&str>,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let mut map = match metadata {
            Some(Value::Object(map)) => map,
            Some(_) => {
                return Err(AppError::BadRequest(
                    "Stripe intent metadata must be a JSON object".to_string(),
                ))
            }
            None => serde_json::Map::new(),
        };

        map.entry("user_id".to_string())
            .or_insert_with(|| Value::String(user_id.to_string()));

        if let Some(article_id) = article_id {
            map.entry("article_id".to_string())
                .or_insert_with(|| Value::String(article_id.to_string()));
        }

        Ok(map)
    }

    fn build_payment_intent_from_record(
        &self,
        record: Value,
        client_secret: Option<String>,
    ) -> Result<StripePaymentIntent> {
        let mut intent: StripePaymentIntent = serde_json::from_value(record).map_err(|e| {
            AppError::Internal(format!("Failed to deserialize payment intent: {}", e))
        })?;

        if client_secret.is_some() {
            intent.client_secret = client_secret;
        }

        Ok(intent)
    }

    fn map_payment_intent_status(status: &str) -> PaymentIntentStatus {
        match status {
            "requires_confirmation" => PaymentIntentStatus::RequiresConfirmation,
            "requires_action" => PaymentIntentStatus::RequiresAction,
            "processing" => PaymentIntentStatus::Processing,
            "requires_capture" => PaymentIntentStatus::RequiresCapture,
            "succeeded" => PaymentIntentStatus::Succeeded,
            "canceled" => PaymentIntentStatus::Canceled,
            _ => PaymentIntentStatus::RequiresPaymentMethod,
        }
    }

    /// 创建订阅计划对应的 Stripe Product 与 Price
    pub async fn create_plan_product_and_price(
        &self,
        plan_id: &str,
        creator_id: &str,
        name: &str,
        description: Option<&str>,
        amount: i64,
        currency: &str,
    ) -> Result<(String, String)> {
        let mut product_params = vec![
            ("name".to_string(), name.to_string()),
            ("type".to_string(), "service".to_string()),
            ("metadata[plan_id]".to_string(), plan_id.to_string()),
            ("metadata[creator_id]".to_string(), creator_id.to_string()),
        ];

        if let Some(desc) = description {
            product_params.push(("description".to_string(), desc.to_string()));
        }

        let product_response = self
            .http_client
            .post("https://api.stripe.com/v1/products")
            .headers(self.get_headers())
            .form(&product_params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !product_response.status().is_success() {
            let error_text = product_response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe product creation failed: {}",
                error_text
            )));
        }

        let product: Value = product_response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        let product_id = product
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("Stripe product missing id".to_string()))?
            .to_string();

        let mut price_params = vec![
            ("product".to_string(), product_id.clone()),
            ("currency".to_string(), currency.to_lowercase()),
            ("unit_amount".to_string(), amount.to_string()),
            ("recurring[interval]".to_string(), "month".to_string()),
            ("metadata[plan_id]".to_string(), plan_id.to_string()),
            ("metadata[creator_id]".to_string(), creator_id.to_string()),
        ];

        let price_response = self
            .http_client
            .post("https://api.stripe.com/v1/prices")
            .headers(self.get_headers())
            .form(&price_params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !price_response.status().is_success() {
            let error_text = price_response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe price creation failed: {}",
                error_text
            )));
        }

        let price: Value = price_response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        let price_id = price
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("Stripe price missing id".to_string()))?
            .to_string();

        Ok((product_id, price_id))
    }

    /// 更新订阅计划对应的 Stripe Product
    pub async fn update_plan_product(
        &self,
        product_id: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        if name.is_none() && description.is_none() {
            return Ok(());
        }

        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(name) = name {
            params.push(("name".to_string(), name.to_string()));
        }
        if let Some(description) = description {
            params.push(("description".to_string(), description.to_string()));
        }

        if params.is_empty() {
            return Ok(());
        }

        let url = format!("https://api.stripe.com/v1/products/{}", product_id);
        let response = self
            .http_client
            .post(url)
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe product update failed: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 为现有 Stripe Product 创建新的价格
    pub async fn create_price_for_product(
        &self,
        product_id: &str,
        amount: i64,
        currency: &str,
    ) -> Result<String> {
        let params = vec![
            ("product".to_string(), product_id.to_string()),
            ("currency".to_string(), currency.to_lowercase()),
            ("unit_amount".to_string(), amount.to_string()),
            ("recurring[interval]".to_string(), "month".to_string()),
        ];

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/prices")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe price creation failed: {}",
                error_text
            )));
        }

        let price: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        let price_id = price
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("Stripe price missing id".to_string()))?
            .to_string();

        Ok(price_id)
    }

    // ============ 支付方式管理 ============

    /// 获取用户保存的支付方式列表
    pub async fn list_payment_methods(&self, user_id: &str) -> Result<Vec<StripePaymentMethod>> {
        let query = r#"
            SELECT * FROM stripe_payment_method
            WHERE user_id = $user_id
            ORDER BY is_default DESC, updated_at DESC
        "#;

        let mut response = self
            .db
            .query_with_params(query, json!({ "user_id": user_id }))
            .await?;

        let records: Vec<Value> = response.take(0)?;
        let mut methods: Vec<StripePaymentMethod> = records
            .into_iter()
            .map(|record| self.map_payment_method_record(record))
            .collect::<Result<Vec<_>>>()?;

        methods.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(b.updated_at.cmp(&a.updated_at))
        });

        Ok(methods)
    }

    /// 添加支付方式
    pub async fn add_payment_method(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
        request: CreatePaymentMethodRequest,
    ) -> Result<StripePaymentMethod> {
        if request.payment_method_id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "缺少 Stripe payment_method_id".to_string(),
            ));
        }

        let customer = self.get_or_create_customer(user_id, email, name).await?;

        let stripe_pm = self
            .attach_payment_method_to_customer(
                &customer.stripe_customer_id,
                &request.payment_method_id,
            )
            .await?;

        let payment_method_type = stripe_pm
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("card");

        let pm_type = Self::parse_payment_method_type(payment_method_type);
        let card_info = stripe_pm.get("card");
        let card_brand = card_info
            .and_then(|v| v.get("brand"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let card_last4 = card_info
            .and_then(|v| v.get("last4"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let card_exp_month = card_info
            .and_then(|v| v.get("exp_month"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let card_exp_year = card_info
            .and_then(|v| v.get("exp_year"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        let mut should_set_default = request.set_as_default;
        if !should_set_default {
            let existing = self.list_payment_methods(user_id).await?;
            if !existing.iter().any(|pm| pm.is_default) {
                should_set_default = true;
            }
        }

        let saved = self
            .store_payment_method(
                user_id,
                &request.payment_method_id,
                pm_type,
                card_brand,
                card_last4,
                card_exp_month,
                card_exp_year,
                should_set_default,
            )
            .await?;

        if should_set_default {
            self.update_default_payment_method(
                user_id,
                &customer.stripe_customer_id,
                Some(&request.payment_method_id),
            )
            .await?;

            return self
                .fetch_payment_method(user_id, &request.payment_method_id)
                .await?
                .ok_or_else(|| AppError::Internal("未能获取已保存的默认支付方式".to_string()));
        }

        Ok(saved)
    }

    /// 设置默认支付方式
    pub async fn set_default_payment_method(
        &self,
        user_id: &str,
        payment_method_id: &str,
    ) -> Result<StripePaymentMethod> {
        let customer = self
            .get_customer_by_user_id(user_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("尚未创建 Stripe Customer".to_string()))?;

        let existing = self
            .fetch_payment_method(user_id, payment_method_id)
            .await?
            .ok_or_else(|| AppError::NotFound("支付方式不存在".to_string()))?;

        if existing.is_default {
            return Ok(existing);
        }

        self.update_default_payment_method(
            user_id,
            &customer.stripe_customer_id,
            Some(payment_method_id),
        )
        .await?;

        self.fetch_payment_method(user_id, payment_method_id)
            .await?
            .ok_or_else(|| AppError::Internal("未能更新默认支付方式".to_string()))
    }

    /// 删除支付方式
    pub async fn delete_payment_method(
        &self,
        user_id: &str,
        payment_method_id: &str,
    ) -> Result<()> {
        let customer = self
            .get_customer_by_user_id(user_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("尚未创建 Stripe Customer".to_string()))?;

        let existing = self
            .fetch_payment_method(user_id, payment_method_id)
            .await?
            .ok_or_else(|| AppError::NotFound("支付方式不存在".to_string()))?;

        self.detach_payment_method(payment_method_id).await?;

        self
            .db
            .query_with_params(
                "DELETE stripe_payment_method WHERE user_id = $user_id AND stripe_payment_method_id = $payment_method_id",
                json!({
                    "user_id": user_id,
                    "payment_method_id": payment_method_id,
                }),
            )
            .await?;

        if existing.is_default {
            let remaining = self.list_payment_methods(user_id).await?;
            if let Some(next) = remaining.first() {
                self.update_default_payment_method(
                    user_id,
                    &customer.stripe_customer_id,
                    Some(&next.id),
                )
                .await?;
            } else {
                self.update_default_payment_method(user_id, &customer.stripe_customer_id, None)
                    .await?;
            }
        }

        Ok(())
    }

    async fn attach_payment_method_to_customer(
        &self,
        customer_id: &str,
        payment_method_id: &str,
    ) -> Result<Value> {
        let url = format!(
            "https://api.stripe.com/v1/payment_methods/{}/attach",
            payment_method_id
        );

        let response = self
            .http_client
            .post(url)
            .headers(self.get_headers())
            .form(&[("customer", customer_id)])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe attach payment method failed: {}",
                error_text
            )));
        }

        let payment_method: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(payment_method)
    }

    async fn detach_payment_method(&self, payment_method_id: &str) -> Result<()> {
        let url = format!(
            "https://api.stripe.com/v1/payment_methods/{}/detach",
            payment_method_id
        );

        let response = self
            .http_client
            .post(url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe detach payment method failed: {}",
                error_text
            )));
        }

        Ok(())
    }

    async fn update_default_payment_method(
        &self,
        user_id: &str,
        customer_id: &str,
        payment_method_id: Option<&str>,
    ) -> Result<()> {
        let mut form_params: Vec<(String, String)> = Vec::new();
        match payment_method_id {
            Some(id) => form_params.push((
                "invoice_settings[default_payment_method]".to_string(),
                id.to_string(),
            )),
            None => form_params.push((
                "invoice_settings[default_payment_method]".to_string(),
                String::new(),
            )),
        }

        let update_url = format!("https://api.stripe.com/v1/customers/{}", customer_id);
        let response = self
            .http_client
            .post(update_url)
            .headers(self.get_headers())
            .form(&form_params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe update default payment method failed: {}",
                error_text
            )));
        }

        self
            .db
            .query_with_params(
                "UPDATE stripe_payment_method SET is_default = false, updated_at = time::now() WHERE user_id = $user_id",
                json!({ "user_id": user_id }),
            )
            .await?;

        if let Some(id) = payment_method_id {
            self
                .db
                .query_with_params(
                    "UPDATE stripe_payment_method SET is_default = true, updated_at = time::now() WHERE user_id = $user_id AND stripe_payment_method_id = $payment_method_id",
                    json!({
                        "user_id": user_id,
                        "payment_method_id": id,
                    }),
                )
                .await?;
        }

        self
            .db
            .query_with_params(
                "UPDATE stripe_customer SET default_payment_method = $payment_method_id, updated_at = time::now() WHERE user_id = $user_id",
                json!({
                    "user_id": user_id,
                    "payment_method_id": payment_method_id,
                }),
            )
            .await?;

        Ok(())
    }

    async fn store_payment_method(
        &self,
        user_id: &str,
        payment_method_id: &str,
        payment_method_type: PaymentMethodType,
        card_brand: Option<String>,
        card_last4: Option<String>,
        card_exp_month: Option<i32>,
        card_exp_year: Option<i32>,
        is_default: bool,
    ) -> Result<StripePaymentMethod> {
        let record_id = format!("stripe_payment_method:{}", payment_method_id);
        let existing = self
            .fetch_payment_method(user_id, payment_method_id)
            .await?;

        let created_at = existing
            .as_ref()
            .map(|m| m.created_at)
            .unwrap_or_else(Utc::now);
        let now = Utc::now();

        self.db
            .query_with_params(
                r#"
            UPSERT $record_id CONTENT {
                id: $record_id,
                user_id: $user_id,
                stripe_payment_method_id: $payment_method_id,
                payment_method_type: $payment_method_type,
                card_brand: $card_brand,
                card_last4: $card_last4,
                card_exp_month: $card_exp_month,
                card_exp_year: $card_exp_year,
                is_default: $is_default,
                created_at: $created_at,
                updated_at: $updated_at
            }
        "#,
                json!({
                    "record_id": record_id,
                    "user_id": user_id,
                    "payment_method_id": payment_method_id,
                    "payment_method_type": payment_method_type,
                    "card_brand": card_brand,
                    "card_last4": card_last4,
                    "card_exp_month": card_exp_month,
                    "card_exp_year": card_exp_year,
                    "is_default": is_default,
                    "created_at": created_at.to_rfc3339(),
                    "updated_at": now.to_rfc3339(),
                }),
            )
            .await?;

        self.fetch_payment_method(user_id, payment_method_id)
            .await?
            .ok_or_else(|| AppError::Internal("未能保存支付方式".to_string()))
    }

    async fn fetch_payment_method(
        &self,
        user_id: &str,
        payment_method_id: &str,
    ) -> Result<Option<StripePaymentMethod>> {
        let query = r#"
            SELECT * FROM stripe_payment_method
            WHERE user_id = $user_id AND stripe_payment_method_id = $payment_method_id
            LIMIT 1
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id,
                    "payment_method_id": payment_method_id,
                }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        if let Some(record) = records.into_iter().next() {
            Ok(Some(self.map_payment_method_record(record)?))
        } else {
            Ok(None)
        }
    }

    fn map_payment_method_record(&self, record: Value) -> Result<StripePaymentMethod> {
        let pm_id = record
            .get("stripe_payment_method_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("支付方式缺少 Stripe ID".to_string()))?
            .to_string();

        let pm_type_value = record
            .get("payment_method_type")
            .cloned()
            .unwrap_or(Value::String("card".to_string()));
        let payment_method_type: PaymentMethodType = serde_json::from_value(pm_type_value)
            .map_err(|e| AppError::Internal(format!("无法解析支付方式类型: {}", e)))?;

        let created_at = Self::parse_datetime_field(record.get("created_at"));
        let updated_at = Self::parse_datetime_field(record.get("updated_at"));

        Ok(StripePaymentMethod {
            id: pm_id.clone(),
            user_id: record
                .get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            stripe_payment_method_id: pm_id,
            payment_method_type,
            card_brand: record
                .get("card_brand")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            card_last4: record
                .get("card_last4")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            card_exp_month: record
                .get("card_exp_month")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            card_exp_year: record
                .get("card_exp_year")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            is_default: record
                .get("is_default")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            created_at,
            updated_at,
        })
    }

    fn parse_payment_method_type(value: &str) -> PaymentMethodType {
        match value {
            "bank_account" => PaymentMethodType::BankAccount,
            "alipay" => PaymentMethodType::Alipay,
            "wechat" | "wechat_pay" => PaymentMethodType::Wechat,
            _ => PaymentMethodType::Card,
        }
    }

    fn parse_datetime_field(value: Option<&Value>) -> DateTime<Utc> {
        value
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }

    // ============ 订阅管理 ============

    /// 创建订阅
    pub async fn create_subscription(
        &self,
        user_id: &str,
        request: CreateStripeSubscriptionRequest,
    ) -> Result<StripeSubscription> {
        request
            .validate()
            .map_err(|e| AppError::BadRequest(e.to_string()))?;

        debug!("Creating subscription for user: {}", user_id);

        // 获取客户信息
        let customer = self
            .get_customer_by_user_id(user_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Customer not found".to_string()))?;

        // 创建Stripe订阅
        let stripe_subscription = self
            .create_stripe_subscription(&customer.stripe_customer_id, &request)
            .await?;

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
            stripe_subscription["current_period_start"]
                .as_i64()
                .unwrap_or_default(),
            0,
        )
        .unwrap_or_default();

        let current_period_end = DateTime::from_timestamp(
            stripe_subscription["current_period_end"]
                .as_i64()
                .unwrap_or_default(),
            0,
        )
        .unwrap_or_default();

        let trial_start = stripe_subscription["trial_start"]
            .as_i64()
            .and_then(|ts| DateTime::from_timestamp(ts, 0));
        let trial_end = stripe_subscription["trial_end"]
            .as_i64()
            .and_then(|ts| DateTime::from_timestamp(ts, 0));

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
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
                }),
            )
            .await?;

        let subscriptions: Vec<Value> = response.take(0)?;
        let subscription_data = subscriptions
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to create subscription".to_string()))?;

        Ok(StripeSubscription {
            id: subscription_data["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            subscription_id: subscription_data["subscription_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            stripe_subscription_id: stripe_subscription["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
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

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/subscriptions")
            .headers(self.get_headers())
            .form(
                &params
                    .iter()
                    .map(|(k, v)| (*k, v.as_str()))
                    .collect::<Vec<_>>(),
            )
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe subscription creation failed: {}",
                error_text
            )));
        }

        let subscription: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(subscription)
    }

    /// 取消订阅
    pub async fn cancel_subscription(
        &self,
        subscription_id: &str,
        at_period_end: bool,
    ) -> Result<()> {
        debug!("Canceling subscription: {}", subscription_id);

        // 获取订阅信息
        let subscription = self
            .get_stripe_subscription_by_id(subscription_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

        // 在Stripe取消订阅
        self.cancel_stripe_subscription(&subscription.stripe_subscription_id, at_period_end)
            .await?;

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
    async fn get_stripe_subscription_by_id(
        &self,
        subscription_id: &str,
    ) -> Result<Option<StripeSubscription>> {
        let query = r#"
            SELECT * FROM stripe_subscription WHERE id = $subscription_id LIMIT 1
        "#;

        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "subscription_id": subscription_id
                }),
            )
            .await?;

        let subscriptions: Vec<Value> = response.take(0)?;

        if let Some(sub_data) = subscriptions.into_iter().next() {
            Ok(Some(StripeSubscription {
                id: sub_data["id"].as_str().unwrap_or_default().to_string(),
                subscription_id: sub_data["subscription_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                stripe_subscription_id: sub_data["stripe_subscription_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                stripe_customer_id: sub_data["stripe_customer_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                stripe_price_id: sub_data["stripe_price_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                status: serde_json::from_value(sub_data["status"].clone())
                    .unwrap_or(StripeSubscriptionStatus::Active),
                current_period_start: chrono::DateTime::parse_from_rfc3339(
                    sub_data["current_period_start"]
                        .as_str()
                        .unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
                current_period_end: chrono::DateTime::parse_from_rfc3339(
                    sub_data["current_period_end"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
                cancel_at_period_end: sub_data["cancel_at_period_end"].as_bool().unwrap_or(false),
                canceled_at: sub_data["canceled_at"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                trial_start: sub_data["trial_start"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                trial_end: sub_data["trial_end"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    sub_data["created_at"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(
                    sub_data["updated_at"].as_str().unwrap_or_default(),
                )
                .unwrap_or_default()
                .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    /// 在Stripe取消订阅
    async fn cancel_stripe_subscription(
        &self,
        stripe_subscription_id: &str,
        at_period_end: bool,
    ) -> Result<()> {
        let url = format!(
            "https://api.stripe.com/v1/subscriptions/{}",
            stripe_subscription_id
        );
        let params = if at_period_end {
            vec![("cancel_at_period_end", "true")]
        } else {
            vec![("cancel_at_period_end", "false")]
        };

        let response = self
            .http_client
            .post(&url)
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe subscription cancellation failed: {}",
                error_text
            )));
        }

        Ok(())
    }

    // ============ Webhook处理 ============

    /// 处理Stripe webhook事件
    pub async fn process_webhook_event(&self, event_data: Value) -> Result<StripeWebhookOutcome> {
        let event_type = event_data["type"]
            .as_str()
            .ok_or_else(|| AppError::BadRequest("Invalid webhook event type".to_string()))?;

        debug!("Processing Stripe webhook event: {}", event_type);

        let saved_event = self.save_webhook_event(&event_data).await?;

        if saved_event.already_processed {
            info!(
                "Stripe webhook event {} already processed, skipping",
                event_type
            );
            return Ok(StripeWebhookOutcome::default());
        }

        let mut outcome = StripeWebhookOutcome::default();

        // 根据事件类型处理
        match event_type {
            "payment_intent.succeeded" => {
                if let Some(update) = self.handle_payment_intent_succeeded(&event_data).await? {
                    outcome.purchase_updates.push(update);
                }
            }
            "payment_intent.payment_failed" => {
                self.handle_payment_intent_failed(&event_data).await?;
            }
            "invoice.payment_succeeded" => {
                if let Some(revenue) = self.handle_invoice_payment_succeeded(&event_data).await? {
                    outcome.subscription_revenues.push(revenue);
                }
            }
            "invoice.payment_failed" => {
                if let Some(status) = self.handle_invoice_payment_failed(&event_data).await? {
                    outcome.subscription_status_updates.push(status);
                }
            }
            "customer.subscription.updated" => {
                if let Some(status) = self.handle_subscription_updated(&event_data).await? {
                    outcome.subscription_status_updates.push(status);
                }
            }
            "customer.subscription.deleted" => {
                if let Some(status) = self.handle_subscription_deleted(&event_data).await? {
                    outcome.subscription_status_updates.push(status);
                }
            }
            _ => {
                info!("Unhandled webhook event type: {}", event_type);
            }
        }

        let summary = json!({
            "purchase_updates": outcome.purchase_updates.len(),
            "subscription_revenues": outcome.subscription_revenues.len(),
            "subscription_status_updates": outcome.subscription_status_updates.len(),
        });

        self.mark_webhook_event_processed(&saved_event.id, summary)
            .await?;

        Ok(outcome)
    }

    /// 保存webhook事件
    async fn save_webhook_event(&self, event_data: &Value) -> Result<SavedWebhookEvent> {
        let stripe_event_id = event_data["id"]
            .as_str()
            .ok_or_else(|| AppError::BadRequest("Stripe webhook 缺少事件 ID".to_string()))?;

        let mut existing = self
            .db
            .query_with_params(
                "SELECT id, processed FROM webhook_event WHERE stripe_event_id = $stripe_event_id LIMIT 1",
                json!({ "stripe_event_id": stripe_event_id }),
            )
            .await?;

        let records: Vec<Value> = existing.take(0)?;
        if let Some(record) = records.into_iter().next() {
            let id = record
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let processed = record
                .get("processed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !processed {
                self.db
                    .query_with_params(
                        "UPDATE webhook_event SET data = $data WHERE id = $event_id",
                        json!({
                            "event_id": &id,
                            "data": event_data,
                        }),
                    )
                    .await?;
            }

            return Ok(SavedWebhookEvent {
                id,
                already_processed: processed,
            });
        }

        let event_id = format!("webhook_event:{}", uuid::Uuid::new_v4());
        let now = Utc::now();
        let event_type = event_data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let query = r#"
            CREATE webhook_event CONTENT {
                id: $event_id,
                stripe_event_id: $stripe_event_id,
                event_type: $event_type,
                processed: false,
                processed_at: NULL,
                processing_summary: NULL,
                data: $data,
                created_at: $created_at
            }
        "#;

        self.db
            .query_with_params(
                query,
                json!({
                    "event_id": &event_id,
                    "stripe_event_id": stripe_event_id,
                    "event_type": event_type,
                    "data": event_data,
                    "created_at": now
                }),
            )
            .await?;

        Ok(SavedWebhookEvent {
            id: event_id,
            already_processed: false,
        })
    }

    async fn mark_webhook_event_processed(&self, event_id: &str, summary: Value) -> Result<()> {
        self.db
            .query_with_params(
                r#"
            UPDATE webhook_event SET
                processed = true,
                processed_at = time::now(),
                processing_summary = $summary
            WHERE id = $event_id
        "#,
                json!({
                    "event_id": event_id,
                    "summary": summary,
                }),
            )
            .await?;

        Ok(())
    }

    /// 处理支付意图成功事件
    async fn handle_payment_intent_succeeded(
        &self,
        event_data: &Value,
    ) -> Result<Option<StripePurchaseUpdate>> {
        let payment_intent = &event_data["data"]["object"];
        let stripe_payment_intent_id = payment_intent["id"]
            .as_str()
            .ok_or_else(|| AppError::BadRequest("Missing payment intent ID".to_string()))?;

        debug!(
            "Handling payment intent succeeded: {}",
            stripe_payment_intent_id
        );

        // 更新支付意图状态
        let query = r#"
            UPDATE payment_intent SET 
                status = $status,
                updated_at = $updated_at
            WHERE stripe_payment_intent_id = $stripe_payment_intent_id
        "#;

        self.db
            .query_with_params(
                query,
                json!({
                    "status": PaymentIntentStatus::Succeeded,
                    "stripe_payment_intent_id": stripe_payment_intent_id,
                    "updated_at": Utc::now()
                }),
            )
            .await?;

        // 尝试构造购买更新信息
        let mut response = self
            .db
            .query_with_params(
                "SELECT * FROM payment_intent WHERE stripe_payment_intent_id = $stripe_payment_intent_id LIMIT 1",
                json!({
                    "stripe_payment_intent_id": stripe_payment_intent_id
                }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        let Some(record) = records.into_iter().next() else {
            return Ok(None);
        };

        let user_id = record
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("支付意图缺少 user_id".to_string()))?
            .to_string();

        let metadata = payment_intent
            .get("metadata")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let article_id = record
            .get("article_id")
            .and_then(Self::extract_record_id)
            .or_else(|| {
                metadata
                    .get("article_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let Some(article_id) = article_id else {
            return Ok(None);
        };

        let purchase_id = metadata
            .get("purchase_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut creator_id = metadata
            .get("creator_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if creator_id.is_none() {
            creator_id = self.get_article_creator_id(&article_id).await?;
        }

        let Some(creator_id) = creator_id else {
            return Ok(None);
        };

        let amount = payment_intent
            .get("amount_received")
            .and_then(|v| v.as_i64())
            .or_else(|| payment_intent.get("amount").and_then(|v| v.as_i64()))
            .unwrap_or_else(|| record.get("amount").and_then(|v| v.as_i64()).unwrap_or(0));

        if amount <= 0 {
            return Ok(None);
        }

        let currency = payment_intent
            .get("currency")
            .and_then(|v| v.as_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| {
                record
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("USD")
                    .to_uppercase()
            });

        Ok(Some(StripePurchaseUpdate {
            stripe_payment_intent_id: stripe_payment_intent_id.to_string(),
            buyer_id: user_id,
            creator_id,
            article_id,
            purchase_id,
            amount,
            currency,
        }))
    }

    /// 处理支付意图失败事件
    async fn handle_payment_intent_failed(&self, event_data: &Value) -> Result<()> {
        let payment_intent = &event_data["data"]["object"];
        let stripe_payment_intent_id = payment_intent["id"]
            .as_str()
            .ok_or_else(|| AppError::BadRequest("Missing payment intent ID".to_string()))?;

        debug!(
            "Handling payment intent failed: {}",
            stripe_payment_intent_id
        );

        // 更新支付意图状态
        let query = r#"
            UPDATE payment_intent SET 
                status = $status,
                updated_at = $updated_at
            WHERE stripe_payment_intent_id = $stripe_payment_intent_id
        "#;

        self.db
            .query_with_params(
                query,
                json!({
                    "status": PaymentIntentStatus::Canceled,
                    "stripe_payment_intent_id": stripe_payment_intent_id,
                    "updated_at": Utc::now()
                }),
            )
            .await?;

        Ok(())
    }

    /// 处理发票支付成功事件
    async fn handle_invoice_payment_succeeded(
        &self,
        event_data: &Value,
    ) -> Result<Option<StripeSubscriptionRevenue>> {
        let invoice = &event_data["data"]["object"];
        let stripe_subscription_id = invoice
            .get("subscription")
            .and_then(|v| v.as_str())
            .or_else(|| {
                invoice
                    .get("lines")
                    .and_then(|v| v.get("data"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("subscription"))
                    .and_then(|v| v.as_str())
            });

        let Some(stripe_subscription_id) = stripe_subscription_id else {
            warn!("Invoice 没有关联订阅，忽略收益处理");
            return Ok(None);
        };

        let mut response = self
            .db
            .query_with_params(
                "SELECT * FROM subscription WHERE stripe_subscription_id = $stripe_subscription_id LIMIT 1",
                json!({ "stripe_subscription_id": stripe_subscription_id }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        let Some(subscription_record) = records.into_iter().next() else {
            warn!("未找到匹配的内部订阅记录: {}", stripe_subscription_id);
            return Ok(None);
        };

        let subscription_id = subscription_record
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let creator_id = subscription_record
            .get("creator_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let subscriber_id = subscription_record
            .get("subscriber_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let amount = invoice
            .get("amount_paid")
            .and_then(|v| v.as_i64())
            .or_else(|| invoice.get("total").and_then(|v| v.as_i64()))
            .unwrap_or(0);

        if amount <= 0 {
            return Ok(None);
        }

        let currency = invoice
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("usd")
            .to_uppercase();

        let period_end = invoice
            .get("lines")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("period"))
            .and_then(|period| period.get("end"))
            .and_then(|v| v.as_i64())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&Utc));

        let _status_update = self
            .update_internal_subscription(
                &subscription_id,
                SubscriptionStatus::Active,
                period_end,
                Some(false),
                None,
            )
            .await?;

        Ok(Some(StripeSubscriptionRevenue {
            subscription_id,
            creator_id,
            subscriber_id,
            amount,
            currency,
            current_period_end: period_end,
        }))
    }

    /// 处理发票支付失败事件
    async fn handle_invoice_payment_failed(
        &self,
        event_data: &Value,
    ) -> Result<Option<StripeSubscriptionStatusUpdate>> {
        let invoice = &event_data["data"]["object"];
        let stripe_subscription_id = invoice.get("subscription").and_then(|v| v.as_str());

        let Some(stripe_subscription_id) = stripe_subscription_id else {
            return Ok(None);
        };

        self.update_internal_subscription_by_stripe_id(
            stripe_subscription_id,
            SubscriptionStatus::PastDue,
            None,
            None,
            Some(true),
        )
        .await
    }

    /// 处理订阅更新事件
    async fn handle_subscription_updated(
        &self,
        event_data: &Value,
    ) -> Result<Option<StripeSubscriptionStatusUpdate>> {
        let subscription = &event_data["data"]["object"];
        let stripe_subscription_id = subscription["id"]
            .as_str()
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
        let internal_status = Self::map_subscription_status(status);

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

        let current_period_end = subscription
            .get("current_period_end")
            .and_then(|v| v.as_i64())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&Utc));

        let canceled_at = subscription
            .get("canceled_at")
            .and_then(|v| v.as_i64())
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&Utc));

        let cancel_at_period_end = subscription
            .get("cancel_at_period_end")
            .and_then(|v| v.as_bool());

        let internal_update = self
            .update_internal_subscription_by_stripe_id(
                stripe_subscription_id,
                internal_status,
                current_period_end,
                canceled_at,
                cancel_at_period_end,
            )
            .await?;

        Ok(internal_update)
    }

    /// 处理订阅删除事件
    async fn handle_subscription_deleted(
        &self,
        event_data: &Value,
    ) -> Result<Option<StripeSubscriptionStatusUpdate>> {
        let subscription = &event_data["data"]["object"];
        let stripe_subscription_id = subscription["id"]
            .as_str()
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

        self.db
            .query_with_params(
                query,
                json!({
                    "status": StripeSubscriptionStatus::Canceled,
                    "canceled_at": Utc::now(),
                    "stripe_subscription_id": stripe_subscription_id,
                    "updated_at": Utc::now()
                }),
            )
            .await?;

        let update = self
            .update_internal_subscription_by_stripe_id(
                stripe_subscription_id,
                SubscriptionStatus::Canceled,
                None,
                Some(Utc::now()),
                Some(false),
            )
            .await?;

        Ok(update)
    }

    fn map_subscription_status(status: &str) -> SubscriptionStatus {
        match status {
            "active" | "trialing" => SubscriptionStatus::Active,
            "canceled" => SubscriptionStatus::Canceled,
            "past_due" | "unpaid" | "incomplete" | "incomplete_expired" => {
                SubscriptionStatus::PastDue
            }
            "expired" => SubscriptionStatus::Expired,
            _ => SubscriptionStatus::Active,
        }
    }

    async fn update_internal_subscription(
        &self,
        subscription_id: &str,
        status: SubscriptionStatus,
        current_period_end: Option<DateTime<Utc>>,
        cancel_at_period_end: Option<bool>,
        canceled_at: Option<DateTime<Utc>>,
    ) -> Result<StripeSubscriptionStatusUpdate> {
        let mut fetch_response = self
            .db
            .query_with_params(
                "SELECT subscriber_id, creator_id FROM subscription WHERE id = $subscription_id LIMIT 1",
                json!({
                    "subscription_id": subscription_id,
                }),
            )
            .await?;

        let records: Vec<Value> = fetch_response.take(0)?;
        let record = records
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("找不到对应的订阅记录".to_string()))?;

        let subscriber_id = record
            .get("subscriber_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let creator_id = record
            .get("creator_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let now = Utc::now();

        self.db
            .query_with_params(
                "UPDATE subscription SET status = $status, updated_at = $updated_at WHERE id = $subscription_id",
                json!({
                    "status": status.to_string(),
                    "updated_at": now,
                    "subscription_id": subscription_id,
                }),
            )
            .await?;

        if let Some(period_end) = current_period_end {
            self.db
                .query_with_params(
                    "UPDATE subscription SET current_period_end = $current_period_end, updated_at = $updated_at WHERE id = $subscription_id",
                    json!({
                        "current_period_end": period_end,
                        "updated_at": now,
                        "subscription_id": subscription_id,
                    }),
                )
                .await?;
        }

        if let Some(flag) = cancel_at_period_end {
            self.db
                .query_with_params(
                    "UPDATE subscription SET cancel_at_period_end = $flag, updated_at = $updated_at WHERE id = $subscription_id",
                    json!({
                        "flag": flag,
                        "updated_at": now,
                        "subscription_id": subscription_id,
                    }),
                )
                .await?;
        }

        if let Some(canceled) = canceled_at {
            self.db
                .query_with_params(
                    "UPDATE subscription SET canceled_at = $canceled_at, updated_at = $updated_at WHERE id = $subscription_id",
                    json!({
                        "canceled_at": canceled,
                        "updated_at": now,
                        "subscription_id": subscription_id,
                    }),
                )
                .await?;
        }

        Ok(StripeSubscriptionStatusUpdate {
            subscription_id: subscription_id.to_string(),
            creator_id,
            subscriber_id,
            status,
            current_period_end,
            cancel_at_period_end,
            canceled_at,
        })
    }

    async fn update_internal_subscription_by_stripe_id(
        &self,
        stripe_subscription_id: &str,
        status: SubscriptionStatus,
        current_period_end: Option<DateTime<Utc>>,
        canceled_at: Option<DateTime<Utc>>,
        cancel_at_period_end: Option<bool>,
    ) -> Result<Option<StripeSubscriptionStatusUpdate>> {
        let mut response = self
            .db
            .query_with_params(
                "SELECT id, subscriber_id, creator_id FROM subscription WHERE stripe_subscription_id = $stripe_subscription_id LIMIT 1",
                json!({
                    "stripe_subscription_id": stripe_subscription_id,
                }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        let Some(record) = records.into_iter().next() else {
            return Ok(None);
        };

        let subscription_id = record
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let subscriber_id = record
            .get("subscriber_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let creator_id = record
            .get("creator_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // update_internal_subscription will re-fetch basic fields, no need to reuse subscriber_id/creator_id from this scope
        let status_update = self
            .update_internal_subscription(
                &subscription_id,
                status,
                current_period_end,
                cancel_at_period_end,
                canceled_at,
            )
            .await?;

        Ok(Some(status_update))
    }

    // ============ Connect账户管理 (可选) ============

    /// 创建Connect账户
    pub async fn create_connect_account(
        &self,
        user_id: &str,
        request: CreateConnectAccountRequest,
    ) -> Result<ConnectAccountResponse> {
        request
            .validate()
            .map_err(|e| AppError::BadRequest(e.to_string()))?;

        if let Some(existing) = self.get_connect_account_for_user(user_id).await? {
            return Ok(existing);
        }

        debug!("Creating Connect account for user: {}", user_id);

        let stripe_account = self.create_stripe_connect_account(&request).await?;
        let account = self
            .upsert_connect_account_record(user_id, &stripe_account)
            .await?;

        self.update_user_profile_stripe_account(user_id, &account.stripe_account_id)
            .await?;

        self.build_connect_account_response(account).await
    }

    /// 获取用户的 Stripe Connect 账户信息
    pub async fn get_connect_account_for_user(
        &self,
        user_id: &str,
    ) -> Result<Option<ConnectAccountResponse>> {
        let Some(record) = self.get_connect_account_record_by_user(user_id).await? else {
            return Ok(None);
        };

        let account = self.refresh_connect_account(&record).await?;
        self.update_user_profile_stripe_account(user_id, &account.stripe_account_id)
            .await?;

        self.build_connect_account_response(account).await.map(Some)
    }

    pub async fn get_connect_account_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<ConnectAccountResponse>> {
        let Some(record) = self
            .get_connect_account_record_by_identifier(identifier)
            .await?
        else {
            return Ok(None);
        };

        let user_id = record.user_id.clone();
        let account = self.refresh_connect_account(&record).await?;
        self.update_user_profile_stripe_account(&user_id, &account.stripe_account_id)
            .await?;

        self.build_connect_account_response(account).await.map(Some)
    }

    /// 在Stripe创建Connect账户
    async fn create_stripe_connect_account(
        &self,
        request: &CreateConnectAccountRequest,
    ) -> Result<Value> {
        let account_type_str = match request.account_type {
            ConnectAccountType::Express => "express",
            ConnectAccountType::Standard => "standard",
            ConnectAccountType::Custom => "custom",
        };

        let mut params: Vec<(String, String)> = vec![
            ("type".to_string(), account_type_str.to_string()),
            ("country".to_string(), request.country.to_uppercase()),
            ("email".to_string(), request.email.clone()),
        ];

        if let Some(business_type) = &request.business_type {
            params.push(("business_type".to_string(), business_type.clone()));
        }

        if let Some(metadata) = request
            .metadata
            .as_ref()
            .and_then(|value| value.as_object())
        {
            for (key, value) in metadata {
                let meta_value = if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    value.to_string()
                };
                params.push((format!("metadata[{}]", key), meta_value));
            }
        }

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/accounts")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Stripe Connect account creation failed: {}",
                error_text
            )));
        }

        let account: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        Ok(account)
    }

    async fn get_connect_account_record_by_user(
        &self,
        user_id: &str,
    ) -> Result<Option<StripeConnectAccount>> {
        let query = r#"
            SELECT * FROM connect_account
            WHERE user_id = $user_id
            ORDER BY updated_at DESC
            LIMIT 1
        "#;

        let mut response = self
            .db
            .query_with_params(query, json!({ "user_id": user_id }))
            .await?;

        let records: Vec<Value> = response.take(0)?;
        if let Some(record) = records.into_iter().next() {
            self.parse_connect_account_record(record).map(Some)
        } else {
            Ok(None)
        }
    }

    async fn get_connect_account_record_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<StripeConnectAccount>> {
        let (query, params) = if identifier.starts_with("connect_account:") {
            (
                "SELECT * FROM connect_account WHERE id = type::thing($record_id) LIMIT 1",
                json!({ "record_id": identifier }),
            )
        } else {
            (
                "SELECT * FROM connect_account WHERE stripe_account_id = $stripe_account_id LIMIT 1",
                json!({ "stripe_account_id": identifier }),
            )
        };

        let mut response = self.db.query_with_params(query, params).await?;
        let records: Vec<Value> = response.take(0)?;

        if let Some(record) = records.into_iter().next() {
            self.parse_connect_account_record(record).map(Some)
        } else {
            Ok(None)
        }
    }

    async fn refresh_connect_account(
        &self,
        record: &StripeConnectAccount,
    ) -> Result<StripeConnectAccount> {
        match self
            .retrieve_stripe_connect_account(&record.stripe_account_id)
            .await
        {
            Ok(latest) => {
                self.upsert_connect_account_record(&record.user_id, &latest)
                    .await
            }
            Err(err) => {
                warn!(
                    "Failed to refresh Stripe Connect account {}: {}",
                    record.stripe_account_id, err
                );
                Ok(record.clone())
            }
        }
    }

    async fn retrieve_stripe_connect_account(&self, stripe_account_id: &str) -> Result<Value> {
        let url = format!("https://api.stripe.com/v1/accounts/{}", stripe_account_id);
        let response = self
            .http_client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Stripe Connect account retrieval failed: {}",
                error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))
    }

    async fn build_connect_account_response(
        &self,
        account: StripeConnectAccount,
    ) -> Result<ConnectAccountResponse> {
        let requires_onboarding =
            !account.details_submitted || !account.charges_enabled || !account.payouts_enabled;

        let onboarding_url = if requires_onboarding {
            match self
                .create_connect_account_link(&account.stripe_account_id)
                .await
            {
                Ok(url) => Some(url),
                Err(err) => {
                    warn!(
                        "Failed to create Stripe connect onboarding link for {}: {}",
                        account.stripe_account_id, err
                    );
                    None
                }
            }
        } else {
            None
        };

        Ok(ConnectAccountResponse {
            account,
            onboarding_url,
            requires_onboarding,
        })
    }

    async fn create_connect_account_link(&self, stripe_account_id: &str) -> Result<String> {
        let return_url =
            self.config.connect_return_url.clone().ok_or_else(|| {
                AppError::Internal("Stripe Connect return URL 未配置".to_string())
            })?;

        let refresh_url =
            self.config.connect_refresh_url.clone().ok_or_else(|| {
                AppError::Internal("Stripe Connect refresh URL 未配置".to_string())
            })?;

        let params = [
            ("account", stripe_account_id),
            ("refresh_url", refresh_url.as_str()),
            ("return_url", return_url.as_str()),
            ("type", "account_onboarding"),
        ];

        let response = self
            .http_client
            .post("https://api.stripe.com/v1/account_links")
            .headers(self.get_headers())
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalService(format!(
                "Stripe account link creation failed: {}",
                error_text
            )));
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Stripe response: {}", e)))?;

        body.get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Internal("Stripe response missing onboarding url".to_string()))
    }

    async fn upsert_connect_account_record(
        &self,
        user_id: &str,
        stripe_account: &Value,
    ) -> Result<StripeConnectAccount> {
        let stripe_account_id = stripe_account["id"]
            .as_str()
            .ok_or_else(|| AppError::Internal("Stripe Connect account缺少ID".to_string()))?;

        let account_type = Self::extract_account_type(stripe_account);
        let country = stripe_account
            .get("country")
            .and_then(|v| v.as_str())
            .unwrap_or("US")
            .to_string();
        let currency = stripe_account
            .get("default_currency")
            .and_then(|v| v.as_str())
            .unwrap_or("usd")
            .to_string();
        let details_submitted = stripe_account
            .get("details_submitted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let charges_enabled = stripe_account
            .get("charges_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let payouts_enabled = stripe_account
            .get("payouts_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let requirements = stripe_account
            .get("requirements")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));

        let mut response = self
            .db
            .query_with_params(
                r#"
            UPDATE connect_account SET
                account_type = $account_type,
                country = $country,
                currency = $currency,
                details_submitted = $details_submitted,
                charges_enabled = $charges_enabled,
                payouts_enabled = $payouts_enabled,
                requirements = $requirements,
                updated_at = time::now()
            WHERE stripe_account_id = $stripe_account_id
            RETURN AFTER
        "#,
                json!({
                    "account_type": account_type,
                    "country": country,
                    "currency": currency,
                    "details_submitted": details_submitted,
                    "charges_enabled": charges_enabled,
                    "payouts_enabled": payouts_enabled,
                    "requirements": requirements,
                    "stripe_account_id": stripe_account_id,
                }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        if let Some(record) = records.into_iter().next() {
            return self.parse_connect_account_record(record);
        }

        let account_id = format!("connect_account:{}", uuid::Uuid::new_v4());
        let mut create_response = self
            .db
            .query_with_params(
                r#"
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
                created_at: time::now(),
                updated_at: time::now()
            }
        "#,
                json!({
                    "account_id": account_id,
                    "user_id": user_id,
                    "stripe_account_id": stripe_account_id,
                    "account_type": account_type,
                    "country": country,
                    "currency": currency,
                    "details_submitted": details_submitted,
                    "charges_enabled": charges_enabled,
                    "payouts_enabled": payouts_enabled,
                    "requirements": requirements,
                }),
            )
            .await?;

        let records: Vec<Value> = create_response.take(0)?;
        let record = records.into_iter().next().ok_or_else(|| {
            AppError::Internal("Failed to create Connect account record".to_string())
        })?;

        self.parse_connect_account_record(record)
    }

    async fn update_user_profile_stripe_account(
        &self,
        user_id: &str,
        stripe_account_id: &str,
    ) -> Result<()> {
        let query = r#"
            UPDATE user_profile SET
                stripe_account_id = $stripe_account_id,
                updated_at = time::now()
            WHERE user_id = $user_id
        "#;

        self.db
            .query_with_params(
                query,
                json!({
                    "stripe_account_id": stripe_account_id,
                    "user_id": user_id,
                }),
            )
            .await?;

        Ok(())
    }

    fn extract_account_type(account: &Value) -> ConnectAccountType {
        match account
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("express")
        {
            "standard" => ConnectAccountType::Standard,
            "custom" => ConnectAccountType::Custom,
            _ => ConnectAccountType::Express,
        }
    }

    async fn get_article_creator_id(&self, article_id: &str) -> Result<Option<String>> {
        let mut response = self
            .db
            .query_with_params(
                "SELECT author_id FROM article WHERE id = $article_id LIMIT 1",
                json!({ "article_id": article_id }),
            )
            .await?;

        let records: Vec<Value> = response.take(0)?;
        Ok(records.into_iter().next().and_then(|record| {
            record
                .get("author_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }))
    }

    fn extract_record_id(value: &Value) -> Option<String> {
        if let Some(s) = value.as_str() {
            return Some(s.to_string());
        }

        if let Some(obj) = value.as_object() {
            if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                if let Some(tb) = obj.get("tb").and_then(|v| v.as_str()) {
                    return Some(format!("{}:{}", tb, id));
                }
                return Some(id.to_string());
            }
        }

        None
    }

    fn parse_connect_account_record(&self, record: Value) -> Result<StripeConnectAccount> {
        let account_type = match record
            .get("account_type")
            .and_then(|v| v.as_str())
            .unwrap_or("express")
        {
            "standard" => ConnectAccountType::Standard,
            "custom" => ConnectAccountType::Custom,
            _ => ConnectAccountType::Express,
        };

        Ok(StripeConnectAccount {
            id: record
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            user_id: record
                .get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            stripe_account_id: record
                .get("stripe_account_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            account_type,
            country: record
                .get("country")
                .and_then(|v| v.as_str())
                .unwrap_or("US")
                .to_string(),
            currency: record
                .get("currency")
                .and_then(|v| v.as_str())
                .unwrap_or("USD")
                .to_string(),
            details_submitted: record
                .get("details_submitted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            charges_enabled: record
                .get("charges_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            payouts_enabled: record
                .get("payouts_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            requirements: record
                .get("requirements")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
            created_at: Self::parse_datetime_field(record.get("created_at")),
            updated_at: Self::parse_datetime_field(record.get("updated_at")),
        })
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
