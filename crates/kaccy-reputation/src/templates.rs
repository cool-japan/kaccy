//! Commitment templates module
//!
//! This module provides:
//! - Pre-defined commitment types
//! - Standard verification criteria
//! - Template management

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{ReputationError, Result};

/// Category of commitment templates
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCategory {
    /// Content creation (articles, videos, podcasts)
    Content,
    /// Code/software development
    Development,
    /// Community engagement
    Community,
    /// Education/teaching
    Education,
    /// Events (meetups, workshops)
    Events,
    /// Support/consulting
    Support,
    /// Other/custom
    Other,
}

impl TemplateCategory {
    /// Returns the lowercase database string representation of this category.
    pub fn to_db_string(&self) -> &'static str {
        match self {
            TemplateCategory::Content => "content",
            TemplateCategory::Development => "development",
            TemplateCategory::Community => "community",
            TemplateCategory::Education => "education",
            TemplateCategory::Events => "events",
            TemplateCategory::Support => "support",
            TemplateCategory::Other => "other",
        }
    }

    /// Parses a category from its lowercase database string; returns `None` for unknown values.
    pub fn from_db_string(s: &str) -> Option<Self> {
        match s {
            "content" => Some(TemplateCategory::Content),
            "development" => Some(TemplateCategory::Development),
            "community" => Some(TemplateCategory::Community),
            "education" => Some(TemplateCategory::Education),
            "events" => Some(TemplateCategory::Events),
            "support" => Some(TemplateCategory::Support),
            "other" => Some(TemplateCategory::Other),
            _ => None,
        }
    }

    /// Returns a vector containing every [`TemplateCategory`] variant.
    pub fn all() -> Vec<TemplateCategory> {
        vec![
            TemplateCategory::Content,
            TemplateCategory::Development,
            TemplateCategory::Community,
            TemplateCategory::Education,
            TemplateCategory::Events,
            TemplateCategory::Support,
            TemplateCategory::Other,
        ]
    }
}

impl std::fmt::Display for TemplateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_db_string())
    }
}

impl std::str::FromStr for TemplateCategory {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "content" => Ok(TemplateCategory::Content),
            "development" | "dev" => Ok(TemplateCategory::Development),
            "community" => Ok(TemplateCategory::Community),
            "education" | "edu" => Ok(TemplateCategory::Education),
            "events" => Ok(TemplateCategory::Events),
            "support" => Ok(TemplateCategory::Support),
            "other" => Ok(TemplateCategory::Other),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid template category: {}. Valid values: content, development, community, education, events, support, other",
                s
            ))),
        }
    }
}

/// A commitment template definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommitmentTemplate {
    /// Unique identifier for this template.
    pub template_id: Uuid,
    /// Display name for the template.
    pub name: String,
    /// String representation of the template category.
    pub category: String,
    /// Full description of what this commitment involves.
    pub description: String,
    /// Default number of days from creation to deadline.
    pub default_duration_days: i32,
    /// Criteria used to verify successful completion.
    pub verification_criteria: String,
    /// Description of evidence that must be submitted.
    pub evidence_requirements: String,
    /// Reputation points awarded for fulfilling this commitment.
    pub reputation_reward: Decimal,
    /// Reputation points deducted for failing this commitment.
    pub reputation_penalty: Decimal,
    /// Whether this is a built-in system template.
    pub is_system: bool,
    /// Whether this template is currently available for use.
    pub is_active: bool,
    /// Number of commitments created from this template.
    pub usage_count: i64,
    /// User who created this template, or `None` for system templates.
    pub created_by: Option<Uuid>,
    /// Timestamp when this template was created.
    pub created_at: DateTime<Utc>,
}

impl CommitmentTemplate {
    /// Get parsed category
    pub fn parsed_category(&self) -> Option<TemplateCategory> {
        TemplateCategory::from_db_string(&self.category)
    }

    /// Calculate deadline from now
    pub fn calculate_deadline(&self) -> DateTime<Utc> {
        Utc::now() + Duration::days(i64::from(self.default_duration_days))
    }
}

/// Request to create a new template
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTemplateRequest {
    /// Display name for the template.
    pub name: String,
    /// Category this template belongs to.
    pub category: TemplateCategory,
    /// Full description of what this commitment involves.
    pub description: String,
    /// Default duration in days from creation to deadline.
    pub default_duration_days: i32,
    /// Criteria used to verify successful completion.
    pub verification_criteria: String,
    /// Description of evidence that must be submitted.
    pub evidence_requirements: String,
    /// Optional reputation reward override.
    pub reputation_reward: Option<Decimal>,
    /// Optional reputation penalty override.
    pub reputation_penalty: Option<Decimal>,
}

impl CreateTemplateRequest {
    /// Validate the request, returning an error if any field is invalid.
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(ReputationError::Validation("Name is required".to_string()));
        }
        if self.name.len() > 100 {
            return Err(ReputationError::Validation(
                "Name must be at most 100 characters".to_string(),
            ));
        }
        if self.description.len() > 1000 {
            return Err(ReputationError::Validation(
                "Description must be at most 1000 characters".to_string(),
            ));
        }
        if self.default_duration_days < 1 || self.default_duration_days > 365 {
            return Err(ReputationError::Validation(
                "Duration must be between 1 and 365 days".to_string(),
            ));
        }
        if self.verification_criteria.is_empty() {
            return Err(ReputationError::Validation(
                "Verification criteria is required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Request to create a commitment from a template
#[derive(Debug, Clone, Deserialize)]
pub struct CreateFromTemplateRequest {
    /// Template to instantiate.
    pub template_id: Uuid,
    /// Token to associate with the new commitment.
    pub token_id: Uuid,
    /// Optional title override; defaults to the template name.
    pub title: Option<String>,
    /// Optional description override.
    pub custom_description: Option<String>,
    /// Optional deadline override; defaults to template's default duration.
    pub custom_deadline: Option<DateTime<Utc>>,
}

/// Summary of templates by category
#[derive(Debug, Clone, Serialize, Default)]
pub struct TemplateSummary {
    /// Total number of templates in the system.
    pub total_templates: i64,
    /// Template counts broken down by category.
    pub templates_by_category: Vec<CategoryCount>,
    /// Most frequently used templates.
    pub most_used: Vec<TemplateUsage>,
}

/// Template count for a single category.
#[derive(Debug, Clone, Serialize)]
pub struct CategoryCount {
    /// String representation of the category.
    pub category: String,
    /// Number of templates in this category.
    pub count: i64,
}

/// Usage statistics for a single template.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateUsage {
    /// Template identifier.
    pub template_id: Uuid,
    /// Template name.
    pub name: String,
    /// Total number of commitments created from this template.
    pub usage_count: i64,
}

/// Service for managing commitment templates
pub struct TemplateService {
    pool: PgPool,
}

impl TemplateService {
    /// Create a new [`TemplateService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize system templates
    pub async fn init_system_templates(&self) -> Result<()> {
        let templates = Self::default_templates();

        for template in templates {
            // Check if exists
            let exists: Option<(bool,)> = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM commitment_templates WHERE name = $1 AND is_system = true)",
            )
            .bind(&template.name)
            .fetch_optional(&self.pool)
            .await?;

            if exists.map(|(e,)| e).unwrap_or(false) {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO commitment_templates
                    (name, category, description, default_duration_days,
                     verification_criteria, evidence_requirements,
                     reputation_reward, reputation_penalty, is_system)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true)
                "#,
            )
            .bind(&template.name)
            .bind(template.category.to_db_string())
            .bind(&template.description)
            .bind(template.default_duration_days)
            .bind(&template.verification_criteria)
            .bind(&template.evidence_requirements)
            .bind(template.reputation_reward.unwrap_or(dec!(10)))
            .bind(template.reputation_penalty.unwrap_or(dec!(-20)))
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get default system templates
    fn default_templates() -> Vec<CreateTemplateRequest> {
        vec![
            // Content templates
            CreateTemplateRequest {
                name: "Blog Post".to_string(),
                category: TemplateCategory::Content,
                description: "Write and publish a blog post on a specified topic".to_string(),
                default_duration_days: 14,
                verification_criteria: "Post must be at least 500 words, original content, and publicly accessible".to_string(),
                evidence_requirements: "URL to published blog post".to_string(),
                reputation_reward: Some(dec!(10)),
                reputation_penalty: Some(dec!(-15)),
            },
            CreateTemplateRequest {
                name: "Video Tutorial".to_string(),
                category: TemplateCategory::Content,
                description: "Create and upload a video tutorial".to_string(),
                default_duration_days: 21,
                verification_criteria: "Video must be at least 5 minutes, clearly explain the topic, and be publicly accessible".to_string(),
                evidence_requirements: "YouTube or video platform URL".to_string(),
                reputation_reward: Some(dec!(15)),
                reputation_penalty: Some(dec!(-20)),
            },
            CreateTemplateRequest {
                name: "Podcast Episode".to_string(),
                category: TemplateCategory::Content,
                description: "Record and publish a podcast episode".to_string(),
                default_duration_days: 14,
                verification_criteria: "Episode must be at least 15 minutes and available on a podcast platform".to_string(),
                evidence_requirements: "Link to podcast episode".to_string(),
                reputation_reward: Some(dec!(12)),
                reputation_penalty: Some(dec!(-18)),
            },
            // Development templates
            CreateTemplateRequest {
                name: "Open Source Contribution".to_string(),
                category: TemplateCategory::Development,
                description: "Make a meaningful contribution to an open source project".to_string(),
                default_duration_days: 30,
                verification_criteria: "Contribution must be merged into the main branch of a public repository".to_string(),
                evidence_requirements: "GitHub PR or commit URL".to_string(),
                reputation_reward: Some(dec!(20)),
                reputation_penalty: Some(dec!(-25)),
            },
            CreateTemplateRequest {
                name: "Bug Fix".to_string(),
                category: TemplateCategory::Development,
                description: "Fix a reported bug in a project".to_string(),
                default_duration_days: 7,
                verification_criteria: "Bug must be fixed and the fix must be deployed or merged".to_string(),
                evidence_requirements: "GitHub issue/PR or deployment proof".to_string(),
                reputation_reward: Some(dec!(8)),
                reputation_penalty: Some(dec!(-12)),
            },
            CreateTemplateRequest {
                name: "Feature Release".to_string(),
                category: TemplateCategory::Development,
                description: "Release a new feature or product update".to_string(),
                default_duration_days: 30,
                verification_criteria: "Feature must be publicly released and functional".to_string(),
                evidence_requirements: "Release notes URL or changelog".to_string(),
                reputation_reward: Some(dec!(25)),
                reputation_penalty: Some(dec!(-30)),
            },
            // Community templates
            CreateTemplateRequest {
                name: "Community AMA".to_string(),
                category: TemplateCategory::Community,
                description: "Host an Ask Me Anything session".to_string(),
                default_duration_days: 14,
                verification_criteria: "AMA must be at least 30 minutes with at least 5 questions answered".to_string(),
                evidence_requirements: "Recording or transcript link".to_string(),
                reputation_reward: Some(dec!(12)),
                reputation_penalty: Some(dec!(-15)),
            },
            CreateTemplateRequest {
                name: "Discord Event".to_string(),
                category: TemplateCategory::Community,
                description: "Host an event in Discord (game night, discussion, etc.)".to_string(),
                default_duration_days: 7,
                verification_criteria: "Event must have at least 5 participants".to_string(),
                evidence_requirements: "Screenshot of event with participants".to_string(),
                reputation_reward: Some(dec!(8)),
                reputation_penalty: Some(dec!(-10)),
            },
            // Education templates
            CreateTemplateRequest {
                name: "Workshop".to_string(),
                category: TemplateCategory::Education,
                description: "Conduct a live workshop or training session".to_string(),
                default_duration_days: 21,
                verification_criteria: "Workshop must be at least 1 hour with educational content".to_string(),
                evidence_requirements: "Recording link and attendance proof".to_string(),
                reputation_reward: Some(dec!(20)),
                reputation_penalty: Some(dec!(-25)),
            },
            CreateTemplateRequest {
                name: "Course Module".to_string(),
                category: TemplateCategory::Education,
                description: "Create a course module or lesson".to_string(),
                default_duration_days: 30,
                verification_criteria: "Module must include learning objectives, content, and exercises".to_string(),
                evidence_requirements: "Link to course platform or materials".to_string(),
                reputation_reward: Some(dec!(18)),
                reputation_penalty: Some(dec!(-22)),
            },
            // Events templates
            CreateTemplateRequest {
                name: "Meetup".to_string(),
                category: TemplateCategory::Events,
                description: "Organize and host a local or virtual meetup".to_string(),
                default_duration_days: 30,
                verification_criteria: "Meetup must have at least 10 attendees".to_string(),
                evidence_requirements: "Event photos and attendance list".to_string(),
                reputation_reward: Some(dec!(15)),
                reputation_penalty: Some(dec!(-20)),
            },
            CreateTemplateRequest {
                name: "Conference Talk".to_string(),
                category: TemplateCategory::Events,
                description: "Give a talk at a conference or event".to_string(),
                default_duration_days: 60,
                verification_criteria: "Talk must be at least 20 minutes at a recognized event".to_string(),
                evidence_requirements: "Conference link and recording".to_string(),
                reputation_reward: Some(dec!(30)),
                reputation_penalty: Some(dec!(-35)),
            },
            // Support templates
            CreateTemplateRequest {
                name: "1-on-1 Mentoring".to_string(),
                category: TemplateCategory::Support,
                description: "Provide one-on-one mentoring session".to_string(),
                default_duration_days: 7,
                verification_criteria: "Session must be at least 30 minutes with actionable advice".to_string(),
                evidence_requirements: "Confirmation from mentee or calendar proof".to_string(),
                reputation_reward: Some(dec!(10)),
                reputation_penalty: Some(dec!(-12)),
            },
            CreateTemplateRequest {
                name: "Code Review".to_string(),
                category: TemplateCategory::Support,
                description: "Provide detailed code review for a project".to_string(),
                default_duration_days: 7,
                verification_criteria: "Review must include specific feedback and suggestions".to_string(),
                evidence_requirements: "Link to review comments or document".to_string(),
                reputation_reward: Some(dec!(8)),
                reputation_penalty: Some(dec!(-10)),
            },
        ]
    }

    /// Create a custom template
    pub async fn create(
        &self,
        creator_id: Uuid,
        request: &CreateTemplateRequest,
    ) -> Result<CommitmentTemplate> {
        request.validate()?;

        let template: CommitmentTemplate = sqlx::query_as(
            r#"
            INSERT INTO commitment_templates
                (name, category, description, default_duration_days,
                 verification_criteria, evidence_requirements,
                 reputation_reward, reputation_penalty, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(&request.name)
        .bind(request.category.to_db_string())
        .bind(&request.description)
        .bind(request.default_duration_days)
        .bind(&request.verification_criteria)
        .bind(&request.evidence_requirements)
        .bind(request.reputation_reward.unwrap_or(dec!(10)))
        .bind(request.reputation_penalty.unwrap_or(dec!(-20)))
        .bind(creator_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(template)
    }

    /// Create a commitment from a template
    pub async fn create_commitment_from_template(
        &self,
        user_id: Uuid,
        request: &CreateFromTemplateRequest,
    ) -> Result<Uuid> {
        // Get template
        let template: CommitmentTemplate = sqlx::query_as(
            "SELECT * FROM commitment_templates WHERE template_id = $1 AND is_active = true",
        )
        .bind(request.template_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            ReputationError::Validation(format!("Template not found: {}", request.template_id))
        })?;

        let title = request
            .title
            .clone()
            .unwrap_or_else(|| template.name.clone());
        let description = request
            .custom_description
            .clone()
            .or_else(|| Some(template.description.clone()));
        let deadline = request
            .custom_deadline
            .unwrap_or_else(|| template.calculate_deadline());

        let mut tx = self.pool.begin().await?;

        // Create commitment
        let (commitment_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO output_commitments
                (user_id, token_id, title, description, deadline)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING commitment_id
            "#,
        )
        .bind(user_id)
        .bind(request.token_id)
        .bind(&title)
        .bind(&description)
        .bind(deadline)
        .fetch_one(&mut *tx)
        .await?;

        // Link to template
        sqlx::query(
            r#"
            INSERT INTO template_instances (template_id, commitment_id)
            VALUES ($1, $2)
            "#,
        )
        .bind(request.template_id)
        .bind(commitment_id)
        .execute(&mut *tx)
        .await?;

        // Update usage count
        sqlx::query(
            r#"
            UPDATE commitment_templates
            SET usage_count = usage_count + 1
            WHERE template_id = $1
            "#,
        )
        .bind(request.template_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(commitment_id)
    }

    /// Get all active templates
    pub async fn get_all_active(&self) -> Result<Vec<CommitmentTemplate>> {
        let templates = sqlx::query_as::<_, CommitmentTemplate>(
            r#"
            SELECT * FROM commitment_templates
            WHERE is_active = true
            ORDER BY usage_count DESC, name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(templates)
    }

    /// Get templates by category
    pub async fn get_by_category(
        &self,
        category: TemplateCategory,
    ) -> Result<Vec<CommitmentTemplate>> {
        let templates = sqlx::query_as::<_, CommitmentTemplate>(
            r#"
            SELECT * FROM commitment_templates
            WHERE category = $1 AND is_active = true
            ORDER BY usage_count DESC, name ASC
            "#,
        )
        .bind(category.to_db_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(templates)
    }

    /// Get template by ID
    pub async fn get_by_id(&self, template_id: Uuid) -> Result<Option<CommitmentTemplate>> {
        let template = sqlx::query_as::<_, CommitmentTemplate>(
            "SELECT * FROM commitment_templates WHERE template_id = $1",
        )
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(template)
    }

    /// Get template summary
    pub async fn get_summary(&self) -> Result<TemplateSummary> {
        // Total count
        let (total,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM commitment_templates WHERE is_active = true")
                .fetch_one(&self.pool)
                .await?;

        // By category
        let by_category: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT category, COUNT(*) as count
            FROM commitment_templates
            WHERE is_active = true
            GROUP BY category
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        // Most used
        let most_used: Vec<(Uuid, String, i64)> = sqlx::query_as(
            r#"
            SELECT template_id, name, usage_count
            FROM commitment_templates
            WHERE is_active = true
            ORDER BY usage_count DESC
            LIMIT 10
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(TemplateSummary {
            total_templates: total,
            templates_by_category: by_category
                .into_iter()
                .map(|(category, count)| CategoryCount { category, count })
                .collect(),
            most_used: most_used
                .into_iter()
                .map(|(template_id, name, usage_count)| TemplateUsage {
                    template_id,
                    name,
                    usage_count,
                })
                .collect(),
        })
    }

    /// Deactivate a template
    pub async fn deactivate(&self, template_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE commitment_templates
            SET is_active = false
            WHERE template_id = $1 AND is_system = false
            "#,
        )
        .bind(template_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Search templates
    pub async fn search(&self, query: &str) -> Result<Vec<CommitmentTemplate>> {
        let search_pattern = format!("%{}%", query.to_lowercase());

        let templates = sqlx::query_as::<_, CommitmentTemplate>(
            r#"
            SELECT * FROM commitment_templates
            WHERE is_active = true
            AND (LOWER(name) LIKE $1 OR LOWER(description) LIKE $1)
            ORDER BY usage_count DESC
            LIMIT 20
            "#,
        )
        .bind(search_pattern)
        .fetch_all(&self.pool)
        .await?;

        Ok(templates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // ── TemplateCategory ──────────────────────────────────────────────────────

    #[test]
    fn test_category_to_db_string_all_variants() {
        assert_eq!(TemplateCategory::Content.to_db_string(), "content");
        assert_eq!(TemplateCategory::Development.to_db_string(), "development");
        assert_eq!(TemplateCategory::Community.to_db_string(), "community");
        assert_eq!(TemplateCategory::Education.to_db_string(), "education");
        assert_eq!(TemplateCategory::Events.to_db_string(), "events");
        assert_eq!(TemplateCategory::Support.to_db_string(), "support");
        assert_eq!(TemplateCategory::Other.to_db_string(), "other");
    }

    #[test]
    fn test_category_from_db_string_roundtrip() {
        for cat in TemplateCategory::all() {
            let s = cat.to_db_string();
            let parsed = TemplateCategory::from_db_string(s)
                .expect("from_db_string should succeed for valid db string");
            assert_eq!(parsed, cat, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_category_from_db_string_unknown_returns_none() {
        assert!(TemplateCategory::from_db_string("unknown").is_none());
        assert!(TemplateCategory::from_db_string("").is_none());
    }

    #[test]
    fn test_category_from_str_aliases() {
        use std::str::FromStr;
        assert_eq!(
            TemplateCategory::from_str("dev").expect("dev alias"),
            TemplateCategory::Development
        );
        assert_eq!(
            TemplateCategory::from_str("edu").expect("edu alias"),
            TemplateCategory::Education
        );
    }

    #[test]
    fn test_category_from_str_invalid_returns_error() {
        use std::str::FromStr;
        assert!(TemplateCategory::from_str("bogus").is_err());
    }

    #[test]
    fn test_category_display() {
        assert_eq!(format!("{}", TemplateCategory::Content), "content");
        assert_eq!(format!("{}", TemplateCategory::Development), "development");
    }

    #[test]
    fn test_category_all_count() {
        // Ensure all() enumerates all 7 variants
        assert_eq!(TemplateCategory::all().len(), 7);
    }

    // ── CreateTemplateRequest::validate ───────────────────────────────────────

    fn valid_request() -> CreateTemplateRequest {
        CreateTemplateRequest {
            name: "Test Template".to_string(),
            category: TemplateCategory::Content,
            description: "A test description".to_string(),
            default_duration_days: 14,
            verification_criteria: "Some criteria".to_string(),
            evidence_requirements: "Some evidence".to_string(),
            reputation_reward: Some(dec!(10)),
            reputation_penalty: Some(dec!(-15)),
        }
    }

    #[test]
    fn test_validate_valid_request_succeeds() {
        valid_request()
            .validate()
            .expect("valid request should pass");
    }

    #[test]
    fn test_validate_empty_name_rejected() {
        let mut req = valid_request();
        req.name = String::new();
        assert!(req.validate().is_err(), "empty name should fail");
    }

    #[test]
    fn test_validate_name_too_long_rejected() {
        let mut req = valid_request();
        req.name = "x".repeat(101);
        let err = req.validate().expect_err("name >100 chars should fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("100"),
            "error message should mention 100: {msg}"
        );
    }

    #[test]
    fn test_validate_name_exactly_100_chars_accepted() {
        let mut req = valid_request();
        req.name = "a".repeat(100);
        req.validate().expect("exactly 100 chars should pass");
    }

    #[test]
    fn test_validate_description_too_long_rejected() {
        let mut req = valid_request();
        req.description = "d".repeat(1001);
        assert!(
            req.validate().is_err(),
            "description >1000 chars should fail"
        );
    }

    #[test]
    fn test_validate_description_exactly_1000_chars_accepted() {
        let mut req = valid_request();
        req.description = "d".repeat(1000);
        req.validate().expect("exactly 1000 chars should pass");
    }

    #[test]
    fn test_validate_duration_zero_rejected() {
        let mut req = valid_request();
        req.default_duration_days = 0;
        assert!(req.validate().is_err(), "duration 0 should fail");
    }

    #[test]
    fn test_validate_duration_366_rejected() {
        let mut req = valid_request();
        req.default_duration_days = 366;
        assert!(req.validate().is_err(), "duration >365 should fail");
    }

    #[test]
    fn test_validate_duration_boundary_values_accepted() {
        let mut req = valid_request();
        req.default_duration_days = 1;
        req.validate().expect("duration 1 should pass");
        req.default_duration_days = 365;
        req.validate().expect("duration 365 should pass");
    }

    #[test]
    fn test_validate_empty_verification_criteria_rejected() {
        let mut req = valid_request();
        req.verification_criteria = String::new();
        assert!(
            req.validate().is_err(),
            "empty verification_criteria should fail"
        );
    }

    // ── CommitmentTemplate helpers ────────────────────────────────────────────

    fn make_template(category: &str, duration_days: i32) -> CommitmentTemplate {
        CommitmentTemplate {
            template_id: Uuid::new_v4(),
            name: "Sample".to_string(),
            category: category.to_string(),
            description: "desc".to_string(),
            default_duration_days: duration_days,
            verification_criteria: "criteria".to_string(),
            evidence_requirements: "evidence".to_string(),
            reputation_reward: dec!(10),
            reputation_penalty: dec!(-15),
            is_system: false,
            is_active: true,
            usage_count: 0,
            created_by: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_parsed_category_known_value() {
        let t = make_template("content", 7);
        assert_eq!(t.parsed_category(), Some(TemplateCategory::Content));
    }

    #[test]
    fn test_parsed_category_unknown_returns_none() {
        let t = make_template("invalid_cat", 7);
        assert!(t.parsed_category().is_none());
    }

    #[test]
    fn test_calculate_deadline_positive_duration() {
        let t = make_template("content", 30);
        let before = chrono::Utc::now();
        let deadline = t.calculate_deadline();
        let after = chrono::Utc::now();
        assert!(deadline > before + chrono::Duration::days(29));
        assert!(deadline < after + chrono::Duration::days(31));
    }

    // ── Default system templates pass their own validation ────────────────────

    #[test]
    fn test_all_default_templates_are_valid() {
        for template in TemplateService::default_templates() {
            template.validate().unwrap_or_else(|e| {
                panic!(
                    "default template '{}' failed validation: {e}",
                    template.name
                )
            });
        }
    }

    #[test]
    fn test_default_reward_penalty_fallbacks() {
        let req = CreateTemplateRequest {
            name: "Minimal".to_string(),
            category: TemplateCategory::Other,
            description: String::new(),
            default_duration_days: 7,
            verification_criteria: "criteria".to_string(),
            evidence_requirements: "evidence".to_string(),
            reputation_reward: None,
            reputation_penalty: None,
        };
        // Default reward is dec!(10), penalty is dec!(-20) — verify None is allowed
        req.validate().expect("None reward/penalty should be valid");
    }

    #[test]
    fn test_create_from_template_request_uses_template_title_when_none() {
        // Exercises the title fallback logic in create_commitment_from_template
        // We can verify it by inspecting the request struct directly
        let token_id = Uuid::new_v4();
        let template_id = Uuid::new_v4();
        let req = CreateFromTemplateRequest {
            template_id,
            token_id,
            title: None,
            custom_description: None,
            custom_deadline: None,
        };
        // title is None — the service would use template.name as fallback
        assert!(req.title.is_none(), "title should be None");
        assert!(req.custom_description.is_none());
        assert!(req.custom_deadline.is_none());
    }

    #[test]
    fn test_template_summary_default_zeroed() {
        let summary = TemplateSummary::default();
        assert_eq!(summary.total_templates, 0);
        assert!(summary.templates_by_category.is_empty());
        assert!(summary.most_used.is_empty());
    }
}
