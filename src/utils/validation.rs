use crate::error::{AppError, Result};
use regex::Regex;
use std::sync::OnceLock;

/// 邮箱验证工具函数
/// 使用标准的RFC 5322邮箱格式验证（与Rainbow-docs一致）
pub fn validate_email(email: &str) -> bool {
    validator::validate_email(email)
}

/// 验证邮箱并返回详细错误信息
pub fn validate_email_format(email: &str) -> Result<()> {
    if email.trim().is_empty() {
        return Err(AppError::Validation("邮箱不能为空".to_string()));
    }

    if !validator::validate_email(email) {
        return Err(AppError::Validation("邮箱格式不正确".to_string()));
    }

    // 检查邮箱长度
    if email.len() > 254 {
        return Err(AppError::Validation("邮箱地址过长".to_string()));
    }

    Ok(())
}

/// 增强的邮箱验证，包含业务规则检查
pub fn validate_email_enhanced(email: &str) -> Result<()> {
    // 基础格式验证
    validate_email_format(email)?;

    // 检查是否为一次性邮箱域名（可选的业务规则）
    if is_disposable_email_domain(email) {
        return Err(AppError::Validation("不支持使用一次性邮箱地址".to_string()));
    }

    Ok(())
}

/// 检查是否为一次性邮箱域名
/// 这是一个简化版本，实际项目中可以维护一个更完整的黑名单
fn is_disposable_email_domain(email: &str) -> bool {
    static DISPOSABLE_DOMAINS: OnceLock<Regex> = OnceLock::new();
    
    let pattern = DISPOSABLE_DOMAINS.get_or_init(|| {
        // 常见的一次性邮箱域名模式
        Regex::new(r"@(10minutemail|tempmail|guerrillamail|mailinator|yopmail)\.").unwrap()
    });
    
    pattern.is_match(&email.to_lowercase())
}

/// 验证用户名格式（用于博客系统）
pub fn validate_username(username: &str) -> Result<()> {
    if username.trim().is_empty() {
        return Err(AppError::Validation("用户名不能为空".to_string()));
    }

    if username.len() < 3 {
        return Err(AppError::Validation("用户名至少需要3个字符".to_string()));
    }

    if username.len() > 30 {
        return Err(AppError::Validation("用户名不能超过30个字符".to_string()));
    }

    // 用户名只能包含字母、数字、下划线和连字符
    let username_regex = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    if !username_regex.is_match(username) {
        return Err(AppError::Validation("用户名只能包含字母、数字、下划线和连字符".to_string()));
    }

    Ok(())
}

/// 验证显示名称格式
pub fn validate_display_name(display_name: &str) -> Result<()> {
    if display_name.trim().is_empty() {
        return Err(AppError::Validation("显示名称不能为空".to_string()));
    }

    if display_name.len() > 50 {
        return Err(AppError::Validation("显示名称不能超过50个字符".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email() {
        // 有效邮箱
        assert!(validate_email("user@example.com"));
        assert!(validate_email("test.email+tag@domain.co.uk"));
        assert!(validate_email("user123@test-domain.com"));

        // 无效邮箱
        assert!(!validate_email("invalid-email"));
        assert!(!validate_email("@domain.com"));
        assert!(!validate_email("user@"));
        assert!(!validate_email("user..name@domain.com"));
    }

    #[test]
    fn test_validate_email_format() {
        // 有效邮箱
        assert!(validate_email_format("user@example.com").is_ok());

        // 无效邮箱
        assert!(validate_email_format("").is_err());
        assert!(validate_email_format("invalid-email").is_err());
        assert!(validate_email_format(&"a".repeat(255)).is_err());
    }

    #[test]
    fn test_validate_email_enhanced() {
        // 有效邮箱
        assert!(validate_email_enhanced("user@example.com").is_ok());

        // 一次性邮箱应该被拒绝
        assert!(validate_email_enhanced("test@10minutemail.com").is_err());
        assert!(validate_email_enhanced("test@tempmail.org").is_err());
    }

    #[test]
    fn test_validate_username() {
        // 有效用户名
        assert!(validate_username("user123").is_ok());
        assert!(validate_username("test_user").is_ok());
        assert!(validate_username("user-name").is_ok());

        // 无效用户名
        assert!(validate_username("").is_err());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("user@name").is_err());
        assert!(validate_username(&"a".repeat(31)).is_err());
    }

    #[test]
    fn test_validate_display_name() {
        // 有效显示名称
        assert!(validate_display_name("John Doe").is_ok());
        assert!(validate_display_name("用户姓名").is_ok());

        // 无效显示名称
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name(&"a".repeat(51)).is_err());
    }
}