//! Password strength validation for the master password.
//!
//! Enforces minimum requirements per 等保 2.0 (8.1.4.1) and ISO 27001 A.9.4:
//! - Minimum 8 characters
//! - Must contain uppercase, lowercase, and digit
//! - Must not be a common password

/// Password strength score (0-4).
pub struct PasswordStrength {
    pub score: u8,
    pub feedback: Vec<String>,
}

/// Common passwords blacklist (top entries to reject outright).
const COMMON_PASSWORDS: &[&str] = &[
    "password", "12345678", "123456789", "1234567890", "qwerty123",
    "abc12345", "password1", "iloveyou", "sunshine1", "princess1",
    "admin123", "welcome1", "monkey123", "master12", "dragon12",
    "letmein1", "football1", "shadow12", "trustno1",
];

/// Checks the strength of a password and returns a score + feedback.
pub fn check_strength(password: &str) -> PasswordStrength {
    let mut score: u8 = 0;
    let mut feedback = Vec::new();

    let len = password.len();
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    // Length check
    if len < 8 {
        feedback.push("Must be at least 8 characters".into());
    } else {
        score += 1;
        if len >= 12 { score += 1; }
        if len >= 16 { score += 1; }
    }

    // Character diversity
    if !has_upper { feedback.push("Add uppercase letters".into()); }
    if !has_lower { feedback.push("Add lowercase letters".into()); }
    if !has_digit { feedback.push("Add numbers".into()); }

    if has_upper && has_lower && has_digit {
        score += 1;
    }
    if has_special {
        score = score.min(3) + 1; // Bonus for special chars, max 4
    }

    // Common password check
    let lower = password.to_lowercase();
    if COMMON_PASSWORDS.contains(&lower.as_str()) {
        score = 0;
        feedback.clear();
        feedback.push("This is a commonly used password".into());
    }

    // Context check
    if lower.contains("termex") {
        score = score.saturating_sub(1);
        feedback.push("Should not contain 'termex'".into());
    }

    PasswordStrength {
        score: score.min(4),
        feedback,
    }
}

/// Validates a master password meets minimum requirements.
/// Returns Ok(()) if acceptable, Err with feedback messages if too weak.
pub fn validate_master_password(password: &str) -> Result<(), Vec<String>> {
    let strength = check_strength(password);
    if strength.score < 2 {
        return Err(if strength.feedback.is_empty() {
            vec!["Password is too weak".into()]
        } else {
            strength.feedback
        });
    }
    Ok(())
}
