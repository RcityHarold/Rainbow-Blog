use crate::{
    error::{AppError, Result},
    models::revenue::*,
    services::Database,
};
use chrono::{DateTime, Utc, Duration, Datelike};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn, error};
use validator::Validate;

#[derive(Clone)]
pub struct RevenueService {
    db: Arc<Database>,
    revenue_share: RevenueShare,
    minimum_payout_amount: i64, // 最低提现金额（美分）
}

impl RevenueService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self {
            db,
            revenue_share: RevenueShare::default(),
            minimum_payout_amount: 5000, // $50最低提现
        })
    }

    /// 记录收益
    pub async fn record_revenue(
        &self,
        creator_id: &str,
        source_type: RevenueSourceType,
        source_id: &str,
        gross_amount: i64,
        currency: &str,
    ) -> Result<RevenueRecord> {
        debug!("Recording revenue for creator: {}", creator_id);

        // 计算实际收益
        let creator_amount = calculate_creator_revenue(gross_amount, &self.revenue_share);
        let now = Utc::now();
        
        // 计算收益周期（当月）
        let period_start = chrono::TimeZone::from_utc_datetime(
            &Utc,
            &chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        );
        let period_end = period_start + Duration::days(31); // 简化处理

        let revenue_id = format!("revenue:{}", uuid::Uuid::new_v4());

        let query = r#"
            CREATE revenue CONTENT {
                id: $revenue_id,
                creator_id: $creator_id,
                source_type: $source_type,
                source_id: $source_id,
                gross_amount: $gross_amount,
                amount: $amount,
                platform_fee: $platform_fee,
                processing_fee: $processing_fee,
                currency: $currency,
                status: $status,
                period_start: $period_start,
                period_end: $period_end,
                created_at: $created_at,
                processed_at: NULL
            }
        "#;

        let platform_fee = calculate_platform_fee(gross_amount, &self.revenue_share);
        let processing_fee = calculate_processing_fee(gross_amount, &self.revenue_share);

        let mut response = self.db.query_with_params(query, json!({
            "revenue_id": revenue_id,
            "creator_id": creator_id,
            "source_type": source_type,
            "source_id": source_id,
            "gross_amount": gross_amount,
            "amount": creator_amount,
            "platform_fee": platform_fee,
            "processing_fee": processing_fee,
            "currency": currency,
            "status": RevenueStatus::Pending,
            "period_start": period_start,
            "period_end": period_end,
            "created_at": now
        })).await?;

        let revenues: Vec<Value> = response.take(0)?;
        let revenue = revenues.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create revenue record".to_string()))?;

        // 更新创作者收益汇总
        self.update_creator_earnings(creator_id, creator_amount).await?;

        Ok(RevenueRecord {
            id: revenue["id"].as_str().unwrap().to_string(),
            creator_id: creator_id.to_string(),
            source_type,
            source_id: source_id.to_string(),
            amount: creator_amount,
            currency: currency.to_string(),
            status: RevenueStatus::Pending,
            period_start,
            period_end,
            created_at: now,
            processed_at: None,
        })
    }

    /// 更新创作者收益汇总
    async fn update_creator_earnings(
        &self,
        creator_id: &str,
        amount: i64,
    ) -> Result<()> {
        let query = r#"
            UPDATE creator_earnings 
            SET 
                total_earnings += $amount,
                pending_balance += $amount,
                lifetime_earnings += $amount,
                updated_at = $now
            WHERE creator_id = $creator_id
        "#;

        self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "amount": amount,
            "now": Utc::now()
        })).await?;

        // 如果不存在则创建
        let create_query = r#"
            CREATE creator_earnings CONTENT {
                id: $id,
                creator_id: $creator_id,
                total_earnings: $amount,
                available_balance: 0,
                pending_balance: $amount,
                lifetime_earnings: $amount,
                currency: 'USD',
                last_payout_at: NULL,
                updated_at: $now
            } WHERE NOT EXISTS (
                SELECT * FROM creator_earnings WHERE creator_id = $creator_id
            )
        "#;

        self.db.query_with_params(create_query, json!({
            "id": format!("creator_earnings:{}", creator_id),
            "creator_id": creator_id,
            "amount": amount,
            "now": Utc::now()
        })).await?;

        Ok(())
    }

    /// 获取创作者收益汇总
    pub async fn get_creator_earnings(
        &self,
        creator_id: &str,
    ) -> Result<CreatorEarnings> {
        let query = "SELECT * FROM creator_earnings WHERE creator_id = $creator_id";
        
        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id
        })).await?;

        let earnings: Vec<Value> = response.take(0)?;
        
        if let Some(earning) = earnings.into_iter().next() {
            Ok(self.parse_creator_earnings(earning)?)
        } else {
            // 返回默认值
            Ok(CreatorEarnings {
                creator_id: creator_id.to_string(),
                total_earnings: 0,
                available_balance: 0,
                pending_balance: 0,
                lifetime_earnings: 0,
                currency: "USD".to_string(),
                last_payout_at: None,
                updated_at: Utc::now(),
            })
        }
    }

    /// 获取收益统计
    pub async fn get_revenue_stats(
        &self,
        creator_id: &str,
        period: RevenuePeriod,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<RevenueStats> {
        debug!("Getting revenue stats for creator: {} from {} to {}", 
            creator_id, start_date, end_date);

        // 获取各类型收益
        let query = r#"
            SELECT 
                source_type,
                SUM(amount) as total_amount,
                count() as count
            FROM revenue
            WHERE 
                creator_id = $creator_id AND
                created_at >= $start_date AND
                created_at < $end_date AND
                status IN ['completed', 'pending']
            GROUP BY source_type
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "start_date": start_date,
            "end_date": end_date
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        
        let mut subscription_revenue = 0;
        let mut purchase_revenue = 0;
        let mut tip_revenue = 0;
        let mut ad_revenue = 0;
        let mut transaction_count = 0;

        for result in results {
            let source_type = result["source_type"].as_str().unwrap_or("");
            let amount = result["total_amount"].as_i64().unwrap_or(0);
            let count = result["count"].as_i64().unwrap_or(0) as i32;
            
            transaction_count += count;
            
            match source_type {
                "subscription" => subscription_revenue = amount,
                "article_purchase" => purchase_revenue = amount,
                "tip" => tip_revenue = amount,
                "advertisement" => ad_revenue = amount,
                _ => {}
            }
        }

        // 获取订阅变化
        let (new_subscribers, cancelled_subscribers) = 
            self.get_subscription_changes(creator_id, start_date, end_date).await?;

        // 获取热门内容收益
        let top_earning_content = 
            self.get_top_earning_content(creator_id, start_date, end_date, 10).await?;

        Ok(RevenueStats {
            period,
            start_date,
            end_date,
            subscription_revenue,
            purchase_revenue,
            tip_revenue,
            ad_revenue,
            total_revenue: subscription_revenue + purchase_revenue + tip_revenue + ad_revenue,
            transaction_count,
            new_subscribers,
            cancelled_subscribers,
            top_earning_content,
        })
    }

    /// 获取订阅变化统计
    async fn get_subscription_changes(
        &self,
        creator_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<(i32, i32)> {
        // 新增订阅数
        let new_query = r#"
            SELECT count() as count
            FROM subscription
            WHERE 
                creator_id = $creator_id AND
                created_at >= $start_date AND
                created_at < $end_date
            GROUP ALL
        "#;

        let mut new_response = self.db.query_with_params(new_query, json!({
            "creator_id": creator_id,
            "start_date": start_date,
            "end_date": end_date
        })).await?;

        let new_results: Vec<Value> = new_response.take(0)?;
        let new_subscribers = new_results.first()
            .and_then(|r| r["count"].as_i64())
            .unwrap_or(0) as i32;

        // 取消订阅数
        let cancelled_query = r#"
            SELECT count() as count
            FROM subscription
            WHERE 
                creator_id = $creator_id AND
                status = 'canceled' AND
                updated_at >= $start_date AND
                updated_at < $end_date
            GROUP ALL
        "#;

        let mut cancelled_response = self.db.query_with_params(cancelled_query, json!({
            "creator_id": creator_id,
            "start_date": start_date,
            "end_date": end_date
        })).await?;

        let cancelled_results: Vec<Value> = cancelled_response.take(0)?;
        let cancelled_subscribers = cancelled_results.first()
            .and_then(|r| r["count"].as_i64())
            .unwrap_or(0) as i32;

        Ok((new_subscribers, cancelled_subscribers))
    }

    /// 获取热门内容收益
    async fn get_top_earning_content(
        &self,
        creator_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<ContentEarning>> {
        let query = r#"
            SELECT 
                article.id as content_id,
                article.title,
                'article' as content_type,
                (
                    SELECT SUM(amount) 
                    FROM revenue 
                    WHERE 
                        source_id IN (
                            SELECT id FROM subscription 
                            WHERE article_id = article.id
                        ) AND
                        source_type = 'subscription' AND
                        created_at >= $start_date AND
                        created_at < $end_date
                ) as subscription_revenue,
                (
                    SELECT SUM(amount) 
                    FROM revenue 
                    WHERE 
                        source_id IN (
                            SELECT id FROM article_purchase 
                            WHERE article_id = article.id
                        ) AND
                        source_type = 'article_purchase' AND
                        created_at >= $start_date AND
                        created_at < $end_date
                ) as purchase_revenue,
                article.view_count
            FROM article
            WHERE 
                author_id = $creator_id AND
                is_paid_content = true
            ORDER BY (subscription_revenue + purchase_revenue) DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "start_date": start_date,
            "end_date": end_date,
            "limit": limit
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let mut content_earnings = Vec::new();

        for result in results {
            let subscription_revenue = result["subscription_revenue"].as_i64().unwrap_or(0);
            let purchase_revenue = result["purchase_revenue"].as_i64().unwrap_or(0);
            let total_revenue = subscription_revenue + purchase_revenue;
            let view_count = result["view_count"].as_i64().unwrap_or(1);
            
            content_earnings.push(ContentEarning {
                content_id: result["content_id"].as_str().unwrap().to_string(),
                content_type: result["content_type"].as_str().unwrap().to_string(),
                title: result["title"].as_str().unwrap().to_string(),
                subscription_revenue,
                purchase_revenue,
                total_revenue,
                view_count,
                conversion_rate: if view_count > 0 {
                    (total_revenue as f64 / view_count as f64) * 100.0
                } else {
                    0.0
                },
            });
        }

        Ok(content_earnings)
    }

    /// 创建支付
    pub async fn create_payout(
        &self,
        creator_id: &str,
        request: CreatePayoutRequest,
    ) -> Result<Payout> {
        debug!("Creating payout for creator: {}", creator_id);

        // 验证请求
        request.validate().map_err(|e| {
            AppError::Validation(format!("支付请求验证失败: {}", e))
        })?;

        // 获取创作者收益
        let earnings = self.get_creator_earnings(creator_id).await?;

        // 检查余额
        if earnings.available_balance < request.amount {
            return Err(AppError::BadRequest(
                format!("可用余额不足。可用余额: ${:.2}, 请求金额: ${:.2}", 
                    earnings.available_balance as f64 / 100.0,
                    request.amount as f64 / 100.0
                )
            ));
        }

        // 检查最低提现金额
        if request.amount < self.minimum_payout_amount {
            return Err(AppError::BadRequest(
                format!("提现金额必须至少为 ${:.2}", 
                    self.minimum_payout_amount as f64 / 100.0
                )
            ));
        }

        let payout_id = format!("payout:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE payout CONTENT {
                id: $payout_id,
                creator_id: $creator_id,
                amount: $amount,
                currency: $currency,
                method: $method,
                status: $status,
                bank_account_id: $bank_account_id,
                description: $description,
                created_at: $created_at,
                processed_at: NULL,
                failed_at: NULL,
                failure_reason: NULL
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "payout_id": payout_id,
            "creator_id": creator_id,
            "amount": request.amount,
            "currency": earnings.currency,
            "method": PayoutMethod::Stripe,
            "status": PayoutStatus::Pending,
            "bank_account_id": request.bank_account_id,
            "description": request.description,
            "created_at": now
        })).await?;

        let payouts: Vec<Value> = response.take(0)?;
        let payout = payouts.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create payout".to_string()))?;

        // 更新创作者余额
        self.update_balance_for_payout(creator_id, request.amount).await?;

        Ok(self.parse_payout(payout)?)
    }

    /// 更新余额（支付时）
    async fn update_balance_for_payout(
        &self,
        creator_id: &str,
        amount: i64,
    ) -> Result<()> {
        let query = r#"
            UPDATE creator_earnings 
            SET 
                available_balance -= $amount,
                updated_at = $now
            WHERE 
                creator_id = $creator_id AND
                available_balance >= $amount
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "amount": amount,
            "now": Utc::now()
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        if results.is_empty() {
            return Err(AppError::BadRequest("余额更新失败".to_string()));
        }

        Ok(())
    }

    /// 处理支付完成
    pub async fn complete_payout(
        &self,
        payout_id: &str,
        stripe_payout_id: Option<String>,
    ) -> Result<Payout> {
        debug!("Completing payout: {}", payout_id);

        let now = Utc::now();
        let query = r#"
            UPDATE payout 
            SET 
                status = 'completed',
                stripe_payout_id = $stripe_payout_id,
                processed_at = $now
            WHERE 
                id = $payout_id AND
                status = 'pending'
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "payout_id": payout_id,
            "stripe_payout_id": stripe_payout_id,
            "now": now
        })).await?;

        let payouts: Vec<Value> = response.take(0)?;
        let payout = payouts.into_iter().next()
            .ok_or_else(|| AppError::NotFound("Payout not found".to_string()))?;

        let parsed_payout = self.parse_payout(payout)?;

        // 更新最后支付时间
        self.update_last_payout_time(&parsed_payout.creator_id).await?;

        // 将待结算余额转为可用余额
        self.process_pending_revenues(&parsed_payout.creator_id).await?;

        Ok(parsed_payout)
    }

    /// 更新最后支付时间
    async fn update_last_payout_time(
        &self,
        creator_id: &str,
    ) -> Result<()> {
        let query = r#"
            UPDATE creator_earnings 
            SET 
                last_payout_at = $now,
                updated_at = $now
            WHERE creator_id = $creator_id
        "#;

        self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "now": Utc::now()
        })).await?;

        Ok(())
    }

    /// 处理待结算收益
    async fn process_pending_revenues(
        &self,
        creator_id: &str,
    ) -> Result<()> {
        let now = Utc::now();
        
        // 将30天前的待结算收益转为可用余额
        let cutoff_date = now - Duration::days(30);
        
        let query = r#"
            UPDATE revenue 
            SET 
                status = 'completed',
                processed_at = $now
            WHERE 
                creator_id = $creator_id AND
                status = 'pending' AND
                created_at <= $cutoff_date
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "now": now,
            "cutoff_date": cutoff_date
        })).await?;

        let revenues: Vec<Value> = response.take(0)?;
        
        // 计算总金额
        let total_amount: i64 = revenues.iter()
            .map(|r| r["amount"].as_i64().unwrap_or(0))
            .sum();

        if total_amount > 0 {
            // 更新余额
            let update_query = r#"
                UPDATE creator_earnings 
                SET 
                    pending_balance -= $amount,
                    available_balance += $amount,
                    updated_at = $now
                WHERE creator_id = $creator_id
            "#;

            self.db.query_with_params(update_query, json!({
                "creator_id": creator_id,
                "amount": total_amount,
                "now": now
            })).await?;
        }

        Ok(())
    }

    /// 获取收益仪表板
    pub async fn get_revenue_dashboard(
        &self,
        creator_id: &str,
    ) -> Result<RevenueDashboard> {
        // 获取收益汇总
        let earnings = self.get_creator_earnings(creator_id).await?;

        // 获取当月统计
        let now = Utc::now();
        let current_month_start = chrono::TimeZone::from_utc_datetime(
            &Utc,
            &chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        );
        let next_month_start = if now.month() == 12 {
            chrono::TimeZone::from_utc_datetime(
                &Utc,
                &chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        } else {
            chrono::TimeZone::from_utc_datetime(
                &Utc,
                &chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        };

        let current_month_stats = self.get_revenue_stats(
            creator_id,
            RevenuePeriod::Monthly,
            current_month_start,
            next_month_start,
        ).await?;

        // 获取上月统计
        let last_month_end = current_month_start;
        let last_month_start = if current_month_start.month() == 1 {
            chrono::TimeZone::from_utc_datetime(
                &Utc,
                &chrono::NaiveDate::from_ymd_opt(current_month_start.year() - 1, 12, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        } else {
            chrono::TimeZone::from_utc_datetime(
                &Utc,
                &chrono::NaiveDate::from_ymd_opt(current_month_start.year(), current_month_start.month() - 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        };

        let last_month_stats = self.get_revenue_stats(
            creator_id,
            RevenuePeriod::Monthly,
            last_month_start,
            last_month_end,
        ).await?;

        // 获取最近交易
        let recent_transactions = self.get_recent_transactions(creator_id, 10).await?;

        // 获取待处理支付
        let pending_payouts = self.get_pending_payouts(creator_id).await?;

        // 获取银行账户
        let bank_accounts = self.get_bank_accounts(creator_id).await?;

        // 计算下次支付日期（每月1日）
        let next_payout_date = if now.day() >= 1 {
            Some(next_month_start)
        } else {
            Some(current_month_start)
        };

        Ok(RevenueDashboard {
            earnings,
            current_month_stats,
            last_month_stats,
            recent_transactions,
            pending_payouts,
            bank_accounts,
            minimum_payout_amount: self.minimum_payout_amount,
            next_payout_date,
        })
    }

    /// 获取最近交易记录
    async fn get_recent_transactions(
        &self,
        creator_id: &str,
        limit: i32,
    ) -> Result<Vec<RevenueRecord>> {
        let query = r#"
            SELECT * FROM revenue
            WHERE creator_id = $creator_id
            ORDER BY created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id,
            "limit": limit
        })).await?;

        let revenues: Vec<Value> = response.take(0)?;
        revenues.into_iter()
            .map(|r| self.parse_revenue_record(r))
            .collect()
    }

    /// 获取待处理支付
    async fn get_pending_payouts(
        &self,
        creator_id: &str,
    ) -> Result<Vec<Payout>> {
        let query = r#"
            SELECT * FROM payout
            WHERE 
                creator_id = $creator_id AND
                status IN ['pending', 'processing']
            ORDER BY created_at DESC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id
        })).await?;

        let payouts: Vec<Value> = response.take(0)?;
        payouts.into_iter()
            .map(|p| self.parse_payout(p))
            .collect()
    }

    /// 获取银行账户列表
    pub async fn get_bank_accounts(
        &self,
        creator_id: &str,
    ) -> Result<Vec<BankAccount>> {
        let query = r#"
            SELECT * FROM bank_account
            WHERE 
                creator_id = $creator_id AND
                is_verified = true
            ORDER BY is_default DESC, created_at DESC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id
        })).await?;

        let accounts: Vec<Value> = response.take(0)?;
        accounts.into_iter()
            .map(|a| self.parse_bank_account(a))
            .collect()
    }

    /// 解析收益记录
    fn parse_revenue_record(&self, value: Value) -> Result<RevenueRecord> {
        Ok(RevenueRecord {
            id: value["id"].as_str().unwrap().to_string(),
            creator_id: value["creator_id"].as_str().unwrap().to_string(),
            source_type: serde_json::from_value(value["source_type"].clone())?,
            source_id: value["source_id"].as_str().unwrap().to_string(),
            amount: value["amount"].as_i64().unwrap(),
            currency: value["currency"].as_str().unwrap().to_string(),
            status: serde_json::from_value(value["status"].clone())?,
            period_start: DateTime::parse_from_rfc3339(value["period_start"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            period_end: DateTime::parse_from_rfc3339(value["period_end"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            created_at: DateTime::parse_from_rfc3339(value["created_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            processed_at: value["processed_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }

    /// 解析创作者收益
    fn parse_creator_earnings(&self, value: Value) -> Result<CreatorEarnings> {
        Ok(CreatorEarnings {
            creator_id: value["creator_id"].as_str().unwrap().to_string(),
            total_earnings: value["total_earnings"].as_i64().unwrap_or(0),
            available_balance: value["available_balance"].as_i64().unwrap_or(0),
            pending_balance: value["pending_balance"].as_i64().unwrap_or(0),
            lifetime_earnings: value["lifetime_earnings"].as_i64().unwrap_or(0),
            currency: value["currency"].as_str().unwrap_or("USD").to_string(),
            last_payout_at: value["last_payout_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            updated_at: DateTime::parse_from_rfc3339(value["updated_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
        })
    }

    /// 解析支付记录
    fn parse_payout(&self, value: Value) -> Result<Payout> {
        Ok(Payout {
            id: value["id"].as_str().unwrap().to_string(),
            creator_id: value["creator_id"].as_str().unwrap().to_string(),
            amount: value["amount"].as_i64().unwrap(),
            currency: value["currency"].as_str().unwrap().to_string(),
            method: serde_json::from_value(value["method"].clone())?,
            status: serde_json::from_value(value["status"].clone())?,
            stripe_payout_id: value["stripe_payout_id"].as_str().map(String::from),
            bank_account_id: value["bank_account_id"].as_str().map(String::from),
            description: value["description"].as_str().map(String::from),
            created_at: DateTime::parse_from_rfc3339(value["created_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            processed_at: value["processed_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            failed_at: value["failed_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            failure_reason: value["failure_reason"].as_str().map(String::from),
        })
    }

    /// 解析银行账户
    fn parse_bank_account(&self, value: Value) -> Result<BankAccount> {
        Ok(BankAccount {
            id: value["id"].as_str().unwrap().to_string(),
            creator_id: value["creator_id"].as_str().unwrap().to_string(),
            account_holder_name: value["account_holder_name"].as_str().unwrap().to_string(),
            account_number_last4: value["account_number_last4"].as_str().unwrap().to_string(),
            bank_name: value["bank_name"].as_str().unwrap().to_string(),
            country: value["country"].as_str().unwrap().to_string(),
            currency: value["currency"].as_str().unwrap().to_string(),
            is_default: value["is_default"].as_bool().unwrap_or(false),
            is_verified: value["is_verified"].as_bool().unwrap_or(false),
            stripe_bank_account_id: value["stripe_bank_account_id"].as_str().map(String::from),
            created_at: DateTime::parse_from_rfc3339(value["created_at"].as_str().unwrap())
                .unwrap().with_timezone(&Utc),
            verified_at: value["verified_at"].as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }

    /// 获取数据库连接（用于路由中的直接查询）
    pub fn get_db(&self) -> &Arc<Database> {
        &self.db
    }

    /// 查询收益交易记录
    pub async fn query_revenue_transactions(
        &self,
        creator_id: &str,
        source_type: Option<&str>,
        status: Option<&str>,
        offset: i32,
        limit: i32,
    ) -> Result<(Vec<Value>, i64)> {
        let mut where_conditions = vec!["creator_id = $creator_id".to_string()];
        let mut query_params = json!({
            "creator_id": creator_id,
            "offset": offset,
            "limit": limit
        });

        if let Some(source_type) = source_type {
            where_conditions.push("source_type = $source_type".to_string());
            query_params["source_type"] = json!(source_type);
        }

        if let Some(status) = status {
            where_conditions.push("status = $status".to_string());
            query_params["status"] = json!(status);
        }

        let where_clause = where_conditions.join(" AND ");
        
        // 获取数据
        let query_str = format!(r#"
            SELECT * FROM revenue
            WHERE {}
            ORDER BY created_at DESC
            START {}
            LIMIT {}
        "#, where_clause, offset, limit);

        let mut response = self.db.query_with_params(&query_str, query_params.clone()).await?;
        let transactions: Vec<Value> = response.take(0)?;

        // 获取总数
        let count_query = format!(r#"
            SELECT count() as total FROM revenue
            WHERE {}
            GROUP ALL
        "#, where_clause);

        let mut count_response = self.db.query_with_params(&count_query, json!({
            "creator_id": creator_id
        })).await?;

        let count_results: Vec<Value> = count_response.take(0)?;
        let total = count_results.first()
            .and_then(|r| r["total"].as_i64())
            .unwrap_or(0);

        Ok((transactions, total))
    }

    /// 查询支付列表
    pub async fn query_payouts(&self, creator_id: &str) -> Result<Vec<Value>> {
        let query = r#"
            SELECT * FROM payout
            WHERE creator_id = $creator_id
            ORDER BY created_at DESC
            LIMIT 50
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "creator_id": creator_id
        })).await?;

        let payouts: Vec<Value> = response.take(0)?;
        Ok(payouts)
    }

    /// 查询支付详情
    pub async fn query_payout_details(&self, payout_id: &str, creator_id: &str) -> Result<Option<Value>> {
        let query = r#"
            SELECT * FROM payout
            WHERE id = $payout_id AND creator_id = $creator_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "payout_id": payout_id,
            "creator_id": creator_id
        })).await?;

        let payouts: Vec<Value> = response.take(0)?;
        Ok(payouts.into_iter().next())
    }

    /// 添加银行账户
    pub async fn add_bank_account(
        &self,
        creator_id: &str,
        account_holder_name: &str,
        bank_name: &str,
        country: &str,
        currency: &str,
    ) -> Result<Value> {
        let account_id = format!("bank_account:{}", uuid::Uuid::new_v4());
        let now = Utc::now();

        let query = r#"
            CREATE bank_account CONTENT {
                id: $account_id,
                creator_id: $creator_id,
                account_holder_name: $account_holder_name,
                account_number_last4: "****",
                bank_name: $bank_name,
                country: $country,
                currency: $currency,
                is_default: false,
                is_verified: false,
                stripe_bank_account_id: NULL,
                created_at: $created_at,
                verified_at: NULL
            }
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "account_id": account_id,
            "creator_id": creator_id,
            "account_holder_name": account_holder_name,
            "bank_name": bank_name,
            "country": country,
            "currency": currency,
            "created_at": now
        })).await?;

        let accounts: Vec<Value> = response.take(0)?;
        accounts.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create bank account".to_string()))
    }

    /// 验证银行账户
    pub async fn verify_bank_account(&self, account_id: &str, creator_id: &str) -> Result<bool> {
        let now = Utc::now();
        let query = r#"
            UPDATE bank_account 
            SET 
                is_verified = true,
                verified_at = $now
            WHERE 
                id = $account_id AND 
                creator_id = $creator_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "account_id": account_id,
            "creator_id": creator_id,
            "now": now
        })).await?;

        let accounts: Vec<Value> = response.take(0)?;
        Ok(!accounts.is_empty())
    }

    /// 设置默认银行账户
    pub async fn set_default_bank_account(&self, account_id: &str, creator_id: &str) -> Result<bool> {
        // 先取消其他默认账户
        let clear_query = r#"
            UPDATE bank_account 
            SET is_default = false 
            WHERE creator_id = $creator_id
        "#;

        self.db.query_with_params(clear_query, json!({
            "creator_id": creator_id
        })).await?;

        // 设置新的默认账户
        let set_query = r#"
            UPDATE bank_account 
            SET is_default = true 
            WHERE 
                id = $account_id AND 
                creator_id = $creator_id AND
                is_verified = true
        "#;

        let mut response = self.db.query_with_params(set_query, json!({
            "account_id": account_id,
            "creator_id": creator_id
        })).await?;

        let accounts: Vec<Value> = response.take(0)?;
        Ok(!accounts.is_empty())
    }
}