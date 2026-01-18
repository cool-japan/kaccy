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
    pub template_id: Uuid,
    pub name: String,
    pub category: String,
    pub description: String,
    pub default_duration_days: i32,
    pub verification_criteria: String,
    pub evidence_requirements: String,
    pub reputation_reward: Decimal,
    pub reputation_penalty: Decimal,
    pub is_system: bool,
    pub is_active: bool,
    pub usage_count: i64,
    pub created_by: Option<Uuid>,
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
    pub name: String,
    pub category: TemplateCategory,
    pub description: String,
    pub default_duration_days: i32,
    pub verification_criteria: String,
    pub evidence_requirements: String,
    pub reputation_reward: Option<Decimal>,
    pub reputation_penalty: Option<Decimal>,
}

impl CreateTemplateRequest {
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
    pub template_id: Uuid,
    pub token_id: Uuid,
    pub title: Option<String>,
    pub custom_description: Option<String>,
    pub custom_deadline: Option<DateTime<Utc>>,
}

/// Summary of templates by category
#[derive(Debug, Clone, Serialize, Default)]
pub struct TemplateSummary {
    pub total_templates: i64,
    pub templates_by_category: Vec<CategoryCount>,
    pub most_used: Vec<TemplateUsage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryCount {
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateUsage {
    pub template_id: Uuid,
    pub name: String,
    pub usage_count: i64,
}

/// Service for managing commitment templates
pub struct TemplateService {
    pool: PgPool,
}

impl TemplateService {
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
