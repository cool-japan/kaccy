//! KYC (Know Your Customer) submission and verification models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::{KycStatus, ValidationError};

/// KYC submission with detailed information
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KycSubmission {
    pub submission_id: Uuid,
    pub user_id: Uuid,
    /// Full legal name
    pub full_name: String,
    /// Date of birth
    pub date_of_birth: chrono::NaiveDate,
    /// Nationality/citizenship
    pub nationality: String,
    /// Country of residence
    pub country_of_residence: String,
    /// Street address
    pub address_line1: String,
    /// Additional address information
    pub address_line2: Option<String>,
    /// City
    pub city: String,
    /// State/province
    pub state_province: Option<String>,
    /// Postal/ZIP code
    pub postal_code: String,
    /// Type of ID document
    pub id_document_type: IdDocumentType,
    /// ID document number
    pub id_document_number: String,
    /// ID document issuing country
    pub id_issuing_country: String,
    /// ID document expiry date
    pub id_expiry_date: Option<chrono::NaiveDate>,
    /// URL to ID document front image
    pub id_document_front_url: String,
    /// URL to ID document back image (if applicable)
    pub id_document_back_url: Option<String>,
    /// URL to selfie with ID
    pub selfie_url: String,
    /// URL to proof of address document
    pub proof_of_address_url: Option<String>,
    /// Current status of the submission
    pub status: KycStatus,
    /// Admin who reviewed (if reviewed)
    pub reviewed_by_user_id: Option<Uuid>,
    /// Review timestamp
    pub reviewed_at: Option<DateTime<Utc>>,
    /// Rejection reason or review notes
    pub review_notes: Option<String>,
    /// Risk level assessment
    pub risk_level: Option<RiskLevel>,
    /// Whether enhanced due diligence is required
    pub requires_edd: bool,
    /// IP address at time of submission
    pub submission_ip: Option<String>,
    /// User agent at time of submission
    pub submission_user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Type of identification document
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum IdDocumentType {
    /// National ID card
    NationalId,
    /// Passport
    #[default]
    Passport,
    /// Driver's license
    DriversLicense,
    /// Residence permit
    ResidencePermit,
}

impl fmt::Display for IdDocumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdDocumentType::NationalId => write!(f, "National ID"),
            IdDocumentType::Passport => write!(f, "Passport"),
            IdDocumentType::DriversLicense => write!(f, "Driver's License"),
            IdDocumentType::ResidencePermit => write!(f, "Residence Permit"),
        }
    }
}

/// Risk level assessment for KYC
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum RiskLevel {
    /// Low risk user
    Low,
    /// Medium risk user
    #[default]
    Medium,
    /// High risk user (requires enhanced monitoring)
    High,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
        }
    }
}

impl fmt::Display for KycSubmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KycSubmission({}, user={}, status={}, name={})",
            self.submission_id, self.user_id, self.status, self.full_name
        )
    }
}

impl KycSubmission {
    /// Check if ID document is expired
    pub fn is_id_expired(&self) -> bool {
        if let Some(expiry) = self.id_expiry_date {
            expiry < chrono::Utc::now().date_naive()
        } else {
            false
        }
    }

    /// Check if user is under minimum age (18)
    pub fn is_underage(&self) -> bool {
        let today = chrono::Utc::now().date_naive();
        let age_years = (today - self.date_of_birth).num_days() / 365;
        age_years < 18
    }

    /// Check if submission is complete and ready for review
    pub fn is_complete(&self) -> bool {
        !self.id_document_front_url.is_empty()
            && !self.selfie_url.is_empty()
            && !self.is_id_expired()
            && !self.is_underage()
    }

    /// Check if submission can be reviewed
    pub fn can_review(&self) -> bool {
        self.status == KycStatus::Pending && self.is_complete()
    }
}

/// Request to submit KYC information
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitKycRequest {
    pub full_name: String,
    pub date_of_birth: chrono::NaiveDate,
    pub nationality: String,
    pub country_of_residence: String,
    pub address_line1: String,
    pub address_line2: Option<String>,
    pub city: String,
    pub state_province: Option<String>,
    pub postal_code: String,
    pub id_document_type: IdDocumentType,
    pub id_document_number: String,
    pub id_issuing_country: String,
    pub id_expiry_date: Option<chrono::NaiveDate>,
    pub id_document_front_url: String,
    pub id_document_back_url: Option<String>,
    pub selfie_url: String,
    pub proof_of_address_url: Option<String>,
}

impl SubmitKycRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Name validation
        if self.full_name.is_empty() {
            return Err(ValidationError("Full name is required".to_string()));
        }
        if self.full_name.len() < 2 {
            return Err(ValidationError(
                "Full name must be at least 2 characters".to_string(),
            ));
        }
        if self.full_name.len() > 100 {
            return Err(ValidationError(
                "Full name must be at most 100 characters".to_string(),
            ));
        }

        // Age validation
        let today = chrono::Utc::now().date_naive();
        let age_years = (today - self.date_of_birth).num_days() / 365;
        if age_years < 18 {
            return Err(ValidationError(
                "You must be at least 18 years old".to_string(),
            ));
        }
        if age_years > 120 {
            return Err(ValidationError("Invalid date of birth".to_string()));
        }

        // Address validation
        if self.address_line1.is_empty() {
            return Err(ValidationError("Address is required".to_string()));
        }
        if self.city.is_empty() {
            return Err(ValidationError("City is required".to_string()));
        }
        if self.postal_code.is_empty() {
            return Err(ValidationError("Postal code is required".to_string()));
        }

        // ID validation
        if self.id_document_number.is_empty() {
            return Err(ValidationError(
                "ID document number is required".to_string(),
            ));
        }
        if self.id_document_number.len() > 50 {
            return Err(ValidationError(
                "ID document number is too long".to_string(),
            ));
        }

        // Check ID expiry
        if let Some(expiry) = self.id_expiry_date {
            if expiry < today {
                return Err(ValidationError("ID document has expired".to_string()));
            }
        }

        // Document URL validation
        if self.id_document_front_url.is_empty() {
            return Err(ValidationError(
                "ID document front image is required".to_string(),
            ));
        }
        if !self.id_document_front_url.starts_with("https://") {
            return Err(ValidationError(
                "ID document front URL must use HTTPS".to_string(),
            ));
        }

        if self.selfie_url.is_empty() {
            return Err(ValidationError("Selfie image is required".to_string()));
        }
        if !self.selfie_url.starts_with("https://") {
            return Err(ValidationError("Selfie URL must use HTTPS".to_string()));
        }

        // Validate optional back image URL if provided
        if let Some(ref url) = self.id_document_back_url {
            if !url.starts_with("https://") {
                return Err(ValidationError(
                    "ID document back URL must use HTTPS".to_string(),
                ));
            }
        }

        // Validate optional proof of address URL if provided
        if let Some(ref url) = self.proof_of_address_url {
            if !url.starts_with("https://") {
                return Err(ValidationError(
                    "Proof of address URL must use HTTPS".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Request to review KYC submission (admin only)
#[derive(Debug, Deserialize)]
pub struct ReviewKycRequest {
    pub status: KycStatus,
    pub notes: Option<String>,
    pub risk_level: Option<RiskLevel>,
    pub requires_edd: bool,
}

impl ReviewKycRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Only allow Verified or Rejected status in review
        if self.status == KycStatus::Pending {
            return Err(ValidationError(
                "Cannot set status to Pending in review".to_string(),
            ));
        }

        if let Some(ref notes) = self.notes {
            if notes.len() > 2000 {
                return Err(ValidationError(
                    "Review notes must be at most 2000 characters".to_string(),
                ));
            }
        }

        // If rejecting, notes should be provided
        if self.status == KycStatus::Rejected && self.notes.is_none() {
            return Err(ValidationError(
                "Rejection reason must be provided in notes".to_string(),
            ));
        }

        Ok(())
    }
}

/// Summary of KYC statistics for admin dashboard
#[derive(Debug, Serialize)]
pub struct KycStats {
    pub total_submissions: i64,
    pub pending_submissions: i64,
    pub verified_submissions: i64,
    pub rejected_submissions: i64,
    pub avg_review_time_hours: Option<f64>,
    pub submissions_last_7_days: i64,
    pub submissions_last_30_days: i64,
}

/// Restricted countries list for compliance
pub struct ComplianceChecker;

impl ComplianceChecker {
    /// Countries where service is not available (OFAC sanctions, etc.)
    /// This is a sample list - should be maintained based on legal requirements
    pub const RESTRICTED_COUNTRIES: &'static [&'static str] = &[
        "KP", // North Korea
        "IR", // Iran
        "SY", // Syria
        "CU", // Cuba
        "MM", // Myanmar
    ];

    /// Check if country is restricted
    pub fn is_country_restricted(country_code: &str) -> bool {
        Self::RESTRICTED_COUNTRIES
            .iter()
            .any(|&c| c.eq_ignore_ascii_case(country_code))
    }

    /// Check if user is from a high-risk jurisdiction
    /// This should be expanded based on FATF list and regulations
    pub fn is_high_risk_jurisdiction(country_code: &str) -> bool {
        // Sample high-risk countries (should be updated based on FATF grey/black lists)
        let high_risk = ["AF", "YE", "VE"];
        high_risk
            .iter()
            .any(|&c| c.eq_ignore_ascii_case(country_code))
    }

    /// Determine initial risk level based on submission
    pub fn assess_initial_risk(submission: &SubmitKycRequest) -> RiskLevel {
        if Self::is_high_risk_jurisdiction(&submission.country_of_residence) {
            RiskLevel::High
        } else if Self::is_high_risk_jurisdiction(&submission.nationality) {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_expiry_check() {
        let submission = KycSubmission {
            submission_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            full_name: "John Doe".to_string(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            nationality: "US".to_string(),
            country_of_residence: "US".to_string(),
            address_line1: "123 Main St".to_string(),
            address_line2: None,
            city: "New York".to_string(),
            state_province: Some("NY".to_string()),
            postal_code: "10001".to_string(),
            id_document_type: IdDocumentType::Passport,
            id_document_number: "123456789".to_string(),
            id_issuing_country: "US".to_string(),
            id_expiry_date: Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
            id_document_front_url: "https://example.com/front.jpg".to_string(),
            id_document_back_url: None,
            selfie_url: "https://example.com/selfie.jpg".to_string(),
            proof_of_address_url: None,
            status: KycStatus::Pending,
            reviewed_by_user_id: None,
            reviewed_at: None,
            review_notes: None,
            risk_level: None,
            requires_edd: false,
            submission_ip: None,
            submission_user_agent: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(submission.is_id_expired());
    }

    #[test]
    fn test_underage_check() {
        let submission = KycSubmission {
            submission_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            full_name: "Young Person".to_string(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
            nationality: "US".to_string(),
            country_of_residence: "US".to_string(),
            address_line1: "123 Main St".to_string(),
            address_line2: None,
            city: "New York".to_string(),
            state_province: Some("NY".to_string()),
            postal_code: "10001".to_string(),
            id_document_type: IdDocumentType::Passport,
            id_document_number: "123456789".to_string(),
            id_issuing_country: "US".to_string(),
            id_expiry_date: Some(chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap()),
            id_document_front_url: "https://example.com/front.jpg".to_string(),
            id_document_back_url: None,
            selfie_url: "https://example.com/selfie.jpg".to_string(),
            proof_of_address_url: None,
            status: KycStatus::Pending,
            reviewed_by_user_id: None,
            reviewed_at: None,
            review_notes: None,
            risk_level: None,
            requires_edd: false,
            submission_ip: None,
            submission_user_agent: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(submission.is_underage());
    }

    #[test]
    fn test_compliance_checker() {
        assert!(ComplianceChecker::is_country_restricted("KP"));
        assert!(ComplianceChecker::is_country_restricted("IR"));
        assert!(!ComplianceChecker::is_country_restricted("US"));

        assert!(ComplianceChecker::is_high_risk_jurisdiction("AF"));
        assert!(!ComplianceChecker::is_high_risk_jurisdiction("US"));
    }

    #[test]
    fn test_submit_kyc_validation() {
        let valid_request = SubmitKycRequest {
            full_name: "John Doe".to_string(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            nationality: "US".to_string(),
            country_of_residence: "US".to_string(),
            address_line1: "123 Main St".to_string(),
            address_line2: None,
            city: "New York".to_string(),
            state_province: Some("NY".to_string()),
            postal_code: "10001".to_string(),
            id_document_type: IdDocumentType::Passport,
            id_document_number: "123456789".to_string(),
            id_issuing_country: "US".to_string(),
            id_expiry_date: Some(chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap()),
            id_document_front_url: "https://example.com/front.jpg".to_string(),
            id_document_back_url: None,
            selfie_url: "https://example.com/selfie.jpg".to_string(),
            proof_of_address_url: None,
        };
        assert!(valid_request.validate().is_ok());

        // Test underage
        let underage_request = SubmitKycRequest {
            date_of_birth: chrono::NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
            ..valid_request.clone()
        };
        assert!(underage_request.validate().is_err());

        // Test expired ID
        let expired_id_request = SubmitKycRequest {
            id_expiry_date: Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
            ..valid_request.clone()
        };
        assert!(expired_id_request.validate().is_err());
    }

    #[test]
    fn test_review_kyc_validation() {
        let valid_review = ReviewKycRequest {
            status: KycStatus::Verified,
            notes: Some("All documents verified".to_string()),
            risk_level: Some(RiskLevel::Low),
            requires_edd: false,
        };
        assert!(valid_review.validate().is_ok());

        // Rejection without notes should fail
        let invalid_review = ReviewKycRequest {
            status: KycStatus::Rejected,
            notes: None,
            risk_level: Some(RiskLevel::High),
            requires_edd: false,
        };
        assert!(invalid_review.validate().is_err());
    }
}
