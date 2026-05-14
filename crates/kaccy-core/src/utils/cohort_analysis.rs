//! Cohort Analysis System
//!
//! Provides comprehensive cohort analysis for user behavior, including:
//! - User cohort definition based on registration or first trade date
//! - Retention rate calculation across different time periods
//! - Lifetime value (LTV) estimation
//! - Churn prediction using machine learning features

use chrono::{DateTime, Datelike, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Cohort definition type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CohortType {
    /// Cohort based on registration date
    Registration,
    /// Cohort based on first trade date
    FirstTrade,
    /// Cohort based on first deposit date
    FirstDeposit,
}

/// Time period for cohort analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CohortPeriod {
    /// Daily cohorts
    Daily,
    /// Weekly cohorts
    Weekly,
    /// Monthly cohorts
    Monthly,
    /// Quarterly cohorts
    Quarterly,
}

/// User cohort identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CohortId {
    /// Cohort type
    pub cohort_type: CohortType,
    /// Cohort period
    pub period: CohortPeriod,
    /// Cohort start date
    pub start_date: DateTime<Utc>,
}

impl CohortId {
    /// Create a new cohort ID from a date
    pub fn from_date(date: DateTime<Utc>, cohort_type: CohortType, period: CohortPeriod) -> Self {
        let start_date = match period {
            CohortPeriod::Daily => date.date_naive().and_hms_opt(0, 0, 0).expect("midnight is a valid time").and_utc(),
            CohortPeriod::Weekly => {
                let days_from_monday = date.weekday().num_days_from_monday();
                let monday = date - Duration::days(days_from_monday as i64);
                monday.date_naive().and_hms_opt(0, 0, 0).expect("midnight is a valid time").and_utc()
            }
            CohortPeriod::Monthly => {
                let year = date.year();
                let month = date.month();
                DateTime::from_timestamp(
                    chrono::NaiveDate::from_ymd_opt(year, month, 1)
                        .expect("year/month derived from existing timestamp are valid")
                        .and_hms_opt(0, 0, 0)
                        .expect("midnight is a valid time")
                        .and_utc()
                        .timestamp(),
                    0,
                )
                .expect("timestamp from valid NaiveDate is in range")
            }
            CohortPeriod::Quarterly => {
                let year = date.year();
                let quarter_month = ((date.month() - 1) / 3) * 3 + 1;
                DateTime::from_timestamp(
                    chrono::NaiveDate::from_ymd_opt(year, quarter_month, 1)
                        .expect("year/quarter_month derived from existing timestamp are valid")
                        .and_hms_opt(0, 0, 0)
                        .expect("midnight is a valid time")
                        .and_utc()
                        .timestamp(),
                    0,
                )
                .expect("timestamp from valid NaiveDate is in range")
            }
        };

        Self {
            cohort_type,
            period,
            start_date,
        }
    }

    /// Get the end date of this cohort period
    pub fn end_date(&self) -> DateTime<Utc> {
        match self.period {
            CohortPeriod::Daily => self.start_date + Duration::days(1),
            CohortPeriod::Weekly => self.start_date + Duration::weeks(1),
            CohortPeriod::Monthly => {
                let year = self.start_date.year();
                let month = self.start_date.month();
                let next_month = if month == 12 { 1 } else { month + 1 };
                let next_year = if month == 12 { year + 1 } else { year };
                DateTime::from_timestamp(
                    chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
                        .expect("year/month derived from existing timestamp are valid")
                        .and_hms_opt(0, 0, 0)
                        .expect("midnight is a valid time")
                        .and_utc()
                        .timestamp(),
                    0,
                )
                .expect("timestamp from valid NaiveDate is in range")
            }
            CohortPeriod::Quarterly => self.start_date + Duration::days(90),
        }
    }
}

/// User activity data for cohort analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivity {
    /// User ID
    pub user_id: String,
    /// Registration date
    pub registration_date: DateTime<Utc>,
    /// First trade date (if any)
    pub first_trade_date: Option<DateTime<Utc>>,
    /// First deposit date (if any)
    pub first_deposit_date: Option<DateTime<Utc>>,
    /// Activity dates (dates when user was active)
    pub activity_dates: Vec<DateTime<Utc>>,
    /// Total revenue generated by this user
    pub total_revenue: Decimal,
    /// Number of trades
    pub trade_count: u32,
    /// Last activity date
    pub last_activity_date: Option<DateTime<Utc>>,
}

impl UserActivity {
    /// Get the cohort date for this user
    pub fn cohort_date(&self, cohort_type: CohortType) -> Option<DateTime<Utc>> {
        match cohort_type {
            CohortType::Registration => Some(self.registration_date),
            CohortType::FirstTrade => self.first_trade_date,
            CohortType::FirstDeposit => self.first_deposit_date,
        }
    }

    /// Check if user was active on a given date
    pub fn was_active_on(&self, date: DateTime<Utc>) -> bool {
        self.activity_dates
            .iter()
            .any(|d| d.date_naive() == date.date_naive())
    }

    /// Check if user was active in a date range
    pub fn was_active_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
        self.activity_dates.iter().any(|d| *d >= start && *d < end)
    }
}

/// Cohort retention data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohortRetention {
    /// Cohort ID
    pub cohort_id: CohortId,
    /// Total users in cohort
    pub total_users: usize,
    /// Retention rates by period (period -> retention rate)
    pub retention_rates: HashMap<u32, f64>,
    /// User counts by period (period -> active user count)
    pub user_counts: HashMap<u32, usize>,
}

impl CohortRetention {
    /// Calculate retention rate for a specific period
    pub fn retention_at_period(&self, period: u32) -> Option<f64> {
        self.retention_rates.get(&period).copied()
    }

    /// Get average retention rate across all periods
    pub fn average_retention(&self) -> f64 {
        if self.retention_rates.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.retention_rates.values().sum();
        sum / self.retention_rates.len() as f64
    }
}

/// Lifetime value (LTV) metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtvMetrics {
    /// Cohort ID
    pub cohort_id: CohortId,
    /// Average LTV per user
    pub average_ltv: Decimal,
    /// Median LTV
    pub median_ltv: Decimal,
    /// Predicted LTV (based on current trajectory)
    pub predicted_ltv: Decimal,
    /// Total revenue from cohort
    pub total_revenue: Decimal,
    /// Number of users
    pub user_count: usize,
    /// Average revenue per paying user (ARPPU)
    pub arppu: Decimal,
    /// Paying user percentage
    pub paying_user_percentage: f64,
}

/// Churn prediction features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnFeatures {
    /// User ID
    pub user_id: String,
    /// Days since last activity
    pub days_since_last_activity: i64,
    /// Days since registration
    pub days_since_registration: i64,
    /// Activity frequency (activities per day)
    pub activity_frequency: f64,
    /// Revenue per day
    pub revenue_per_day: Decimal,
    /// Trade frequency (trades per day)
    pub trade_frequency: f64,
    /// Trend in activity (increasing/decreasing)
    pub activity_trend: f64,
    /// Recent activity ratio (last 7 days / previous 30 days)
    pub recent_activity_ratio: f64,
}

impl ChurnFeatures {
    /// Calculate churn probability (simple heuristic model)
    pub fn churn_probability(&self) -> f64 {
        let mut score: f64 = 0.0;

        // Days since last activity (heavily weighted)
        score += match self.days_since_last_activity {
            0..=7 => 0.0,
            8..=14 => 0.1,
            15..=30 => 0.3,
            31..=60 => 0.5,
            _ => 0.8,
        };

        // Activity frequency
        if self.activity_frequency < 0.1 {
            score += 0.3;
        } else if self.activity_frequency < 0.5 {
            score += 0.1;
        }

        // Activity trend
        if self.activity_trend < -0.5 {
            score += 0.3;
        } else if self.activity_trend < 0.0 {
            score += 0.1;
        }

        // Recent activity ratio
        if self.recent_activity_ratio < 0.3 {
            score += 0.2;
        } else if self.recent_activity_ratio < 0.7 {
            score += 0.1;
        }

        // Revenue consideration (paying users less likely to churn)
        if self.revenue_per_day > Decimal::ZERO {
            score *= 0.7;
        }

        score.min(1.0)
    }

    /// Get churn risk level
    pub fn churn_risk_level(&self) -> ChurnRiskLevel {
        let probability = self.churn_probability();
        if probability < 0.3 {
            ChurnRiskLevel::Low
        } else if probability < 0.6 {
            ChurnRiskLevel::Medium
        } else {
            ChurnRiskLevel::High
        }
    }
}

/// Churn risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChurnRiskLevel {
    /// Low probability of churn.
    Low,
    /// Moderate probability of churn.
    Medium,
    /// High probability of churn requiring intervention.
    High,
}

/// Cohort analyzer
pub struct CohortAnalyzer {
    users: Vec<UserActivity>,
}

impl CohortAnalyzer {
    /// Create a new cohort analyzer
    pub fn new(users: Vec<UserActivity>) -> Self {
        Self { users }
    }

    /// Get all cohorts of a specific type and period
    pub fn get_cohorts(&self, cohort_type: CohortType, period: CohortPeriod) -> Vec<CohortId> {
        let mut cohorts = HashSet::new();

        for user in &self.users {
            if let Some(date) = user.cohort_date(cohort_type) {
                let cohort_id = CohortId::from_date(date, cohort_type, period);
                cohorts.insert(cohort_id);
            }
        }

        let mut cohort_vec: Vec<_> = cohorts.into_iter().collect();
        cohort_vec.sort_by_key(|c| c.start_date);
        cohort_vec
    }

    /// Calculate retention rates for a cohort
    pub fn calculate_retention(&self, cohort_id: &CohortId, max_periods: u32) -> CohortRetention {
        let cohort_users: Vec<_> = self
            .users
            .iter()
            .filter(|u| {
                if let Some(date) = u.cohort_date(cohort_id.cohort_type) {
                    let user_cohort =
                        CohortId::from_date(date, cohort_id.cohort_type, cohort_id.period);
                    user_cohort == *cohort_id
                } else {
                    false
                }
            })
            .collect();

        let total_users = cohort_users.len();
        let mut retention_rates = HashMap::new();
        let mut user_counts = HashMap::new();

        for period in 0..=max_periods {
            let period_start = match cohort_id.period {
                CohortPeriod::Daily => cohort_id.start_date + Duration::days(period as i64),
                CohortPeriod::Weekly => cohort_id.start_date + Duration::weeks(period as i64),
                CohortPeriod::Monthly => {
                    let months = period as i64;
                    let year = cohort_id.start_date.year() as i64;
                    let month = cohort_id.start_date.month() as i64 + months;
                    let adjusted_year = year + (month - 1) / 12;
                    let adjusted_month = ((month - 1) % 12 + 1) as u32;
                    DateTime::from_timestamp(
                        chrono::NaiveDate::from_ymd_opt(adjusted_year as i32, adjusted_month, 1)
                            .expect("adjusted year/month from valid cohort timestamp are valid")
                            .and_hms_opt(0, 0, 0)
                            .expect("midnight is a valid time")
                            .and_utc()
                            .timestamp(),
                        0,
                    )
                    .expect("timestamp from valid NaiveDate is in range")
                }
                CohortPeriod::Quarterly => {
                    cohort_id.start_date + Duration::days(90 * period as i64)
                }
            };

            let period_end = match cohort_id.period {
                CohortPeriod::Daily => period_start + Duration::days(1),
                CohortPeriod::Weekly => period_start + Duration::weeks(1),
                CohortPeriod::Monthly => {
                    let year = period_start.year();
                    let month = period_start.month();
                    let next_month = if month == 12 { 1 } else { month + 1 };
                    let next_year = if month == 12 { year + 1 } else { year };
                    DateTime::from_timestamp(
                        chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
                            .expect("year/month derived from existing timestamp are valid")
                            .and_hms_opt(0, 0, 0)
                            .expect("midnight is a valid time")
                            .and_utc()
                            .timestamp(),
                        0,
                    )
                    .expect("timestamp from valid NaiveDate is in range")
                }
                CohortPeriod::Quarterly => period_start + Duration::days(90),
            };

            let active_users = cohort_users
                .iter()
                .filter(|u| u.was_active_in_range(period_start, period_end))
                .count();

            user_counts.insert(period, active_users);
            if total_users > 0 {
                retention_rates.insert(period, active_users as f64 / total_users as f64);
            }
        }

        CohortRetention {
            cohort_id: cohort_id.clone(),
            total_users,
            retention_rates,
            user_counts,
        }
    }

    /// Calculate LTV metrics for a cohort
    pub fn calculate_ltv(&self, cohort_id: &CohortId) -> LtvMetrics {
        let cohort_users: Vec<_> = self
            .users
            .iter()
            .filter(|u| {
                if let Some(date) = u.cohort_date(cohort_id.cohort_type) {
                    let user_cohort =
                        CohortId::from_date(date, cohort_id.cohort_type, cohort_id.period);
                    user_cohort == *cohort_id
                } else {
                    false
                }
            })
            .collect();

        let user_count = cohort_users.len();
        if user_count == 0 {
            return LtvMetrics {
                cohort_id: cohort_id.clone(),
                average_ltv: Decimal::ZERO,
                median_ltv: Decimal::ZERO,
                predicted_ltv: Decimal::ZERO,
                total_revenue: Decimal::ZERO,
                user_count: 0,
                arppu: Decimal::ZERO,
                paying_user_percentage: 0.0,
            };
        }

        let total_revenue: Decimal = cohort_users.iter().map(|u| u.total_revenue).sum();
        let average_ltv = total_revenue / Decimal::from(user_count);

        // Calculate median LTV
        let mut revenues: Vec<Decimal> = cohort_users.iter().map(|u| u.total_revenue).collect();
        revenues.sort();
        let median_ltv = if revenues.len() % 2 == 0 {
            (revenues[revenues.len() / 2 - 1] + revenues[revenues.len() / 2]) / Decimal::TWO
        } else {
            revenues[revenues.len() / 2]
        };

        // Calculate ARPPU (average revenue per paying user)
        let paying_users = cohort_users
            .iter()
            .filter(|u| u.total_revenue > Decimal::ZERO)
            .count();
        let arppu = if paying_users > 0 {
            total_revenue / Decimal::from(paying_users)
        } else {
            Decimal::ZERO
        };
        let paying_user_percentage = if user_count > 0 {
            paying_users as f64 / user_count as f64
        } else {
            0.0
        };

        // Simple LTV prediction: current LTV * growth factor based on cohort age
        let cohort_age_days = (Utc::now() - cohort_id.start_date).num_days();
        let growth_factor = if cohort_age_days < 30 {
            Decimal::from(4)
        } else if cohort_age_days < 90 {
            Decimal::from(2)
        } else {
            Decimal::new(12, 1) // 1.2
        };
        let predicted_ltv = average_ltv * growth_factor;

        LtvMetrics {
            cohort_id: cohort_id.clone(),
            average_ltv,
            median_ltv,
            predicted_ltv,
            total_revenue,
            user_count,
            arppu,
            paying_user_percentage,
        }
    }

    /// Calculate churn features for a user
    pub fn calculate_churn_features(
        &self,
        user_id: &str,
        current_date: DateTime<Utc>,
    ) -> Option<ChurnFeatures> {
        let user = self.users.iter().find(|u| u.user_id == user_id)?;

        let days_since_registration = (current_date - user.registration_date).num_days();
        let days_since_last_activity = user
            .last_activity_date
            .map(|d| (current_date - d).num_days())
            .unwrap_or(days_since_registration);

        let activity_frequency = if days_since_registration > 0 {
            user.activity_dates.len() as f64 / days_since_registration as f64
        } else {
            0.0
        };

        let revenue_per_day = if days_since_registration > 0 {
            user.total_revenue / Decimal::from(days_since_registration.max(1))
        } else {
            Decimal::ZERO
        };

        let trade_frequency = if days_since_registration > 0 {
            user.trade_count as f64 / days_since_registration as f64
        } else {
            0.0
        };

        // Calculate activity trend (last 7 days vs previous 7 days)
        let last_7_days_start = current_date - Duration::days(7);
        let previous_7_days_start = current_date - Duration::days(14);
        let last_7_days_activity = user
            .activity_dates
            .iter()
            .filter(|d| **d >= last_7_days_start && **d < current_date)
            .count() as f64;
        let previous_7_days_activity = user
            .activity_dates
            .iter()
            .filter(|d| **d >= previous_7_days_start && **d < last_7_days_start)
            .count() as f64;

        let activity_trend = if previous_7_days_activity > 0.0 {
            (last_7_days_activity - previous_7_days_activity) / previous_7_days_activity
        } else if last_7_days_activity > 0.0 {
            1.0
        } else {
            -1.0
        };

        // Recent activity ratio (last 7 days / previous 30 days)
        let last_30_days_start = current_date - Duration::days(30);
        let last_30_days_activity = user
            .activity_dates
            .iter()
            .filter(|d| **d >= last_30_days_start && **d < current_date)
            .count() as f64;

        let recent_activity_ratio = if last_30_days_activity > 0.0 {
            last_7_days_activity / last_30_days_activity
        } else {
            0.0
        };

        Some(ChurnFeatures {
            user_id: user_id.to_string(),
            days_since_last_activity,
            days_since_registration,
            activity_frequency,
            revenue_per_day,
            trade_frequency,
            activity_trend,
            recent_activity_ratio,
        })
    }

    /// Get users at risk of churning
    pub fn get_churn_risk_users(
        &self,
        current_date: DateTime<Utc>,
        min_risk_level: ChurnRiskLevel,
    ) -> Vec<(String, ChurnFeatures)> {
        self.users
            .iter()
            .filter_map(|user| {
                let features = self.calculate_churn_features(&user.user_id, current_date)?;
                let risk_level = features.churn_risk_level();

                let include = matches!(
                    (min_risk_level, risk_level),
                    (ChurnRiskLevel::Low, _)
                        | (ChurnRiskLevel::Medium, ChurnRiskLevel::Medium)
                        | (ChurnRiskLevel::Medium, ChurnRiskLevel::High)
                        | (ChurnRiskLevel::High, ChurnRiskLevel::High)
                );

                if include {
                    Some((user.user_id.clone(), features))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_user(
        id: &str,
        registration_date: DateTime<Utc>,
        activity_dates: Vec<DateTime<Utc>>,
        revenue: Decimal,
        trades: u32,
    ) -> UserActivity {
        UserActivity {
            user_id: id.to_string(),
            registration_date,
            first_trade_date: activity_dates.first().copied(),
            first_deposit_date: activity_dates.first().copied(),
            activity_dates: activity_dates.clone(),
            total_revenue: revenue,
            trade_count: trades,
            last_activity_date: activity_dates.last().copied(),
        }
    }

    #[test]
    fn test_cohort_id_from_date() {
        let date = DateTime::from_timestamp(1609459200, 0).unwrap(); // 2021-01-01 00:00:00 UTC
        let cohort = CohortId::from_date(date, CohortType::Registration, CohortPeriod::Daily);
        assert_eq!(cohort.start_date, date);

        let cohort = CohortId::from_date(date, CohortType::Registration, CohortPeriod::Weekly);
        // 2021-01-01 was a Friday, so Monday was 2020-12-28
        assert_eq!(
            cohort.start_date,
            DateTime::from_timestamp(1609113600, 0).unwrap()
        );

        let cohort = CohortId::from_date(date, CohortType::Registration, CohortPeriod::Monthly);
        assert_eq!(cohort.start_date, date);
    }

    #[test]
    fn test_cohort_retention() {
        let base_date = DateTime::from_timestamp(1609459200, 0).unwrap(); // 2021-01-01
        let users = vec![
            create_test_user(
                "user1",
                base_date,
                vec![
                    base_date,
                    base_date + Duration::days(1),
                    base_date + Duration::days(2),
                ],
                Decimal::from(100),
                3,
            ),
            create_test_user(
                "user2",
                base_date,
                vec![base_date, base_date + Duration::days(1)],
                Decimal::from(50),
                2,
            ),
            create_test_user("user3", base_date, vec![base_date], Decimal::from(25), 1),
        ];

        let analyzer = CohortAnalyzer::new(users);
        let cohort_id =
            CohortId::from_date(base_date, CohortType::Registration, CohortPeriod::Daily);
        let retention = analyzer.calculate_retention(&cohort_id, 3);

        assert_eq!(retention.total_users, 3);
        assert_eq!(retention.retention_at_period(0), Some(1.0)); // Day 0: 100% active
        assert_eq!(retention.retention_at_period(1).unwrap(), 2.0 / 3.0); // Day 1: 2/3 active
    }

    #[test]
    fn test_ltv_calculation() {
        let base_date = DateTime::from_timestamp(1609459200, 0).unwrap();
        let users = vec![
            create_test_user("user1", base_date, vec![base_date], Decimal::from(100), 1),
            create_test_user("user2", base_date, vec![base_date], Decimal::from(200), 2),
            create_test_user("user3", base_date, vec![base_date], Decimal::from(50), 1),
        ];

        let analyzer = CohortAnalyzer::new(users);
        let cohort_id =
            CohortId::from_date(base_date, CohortType::Registration, CohortPeriod::Daily);
        let ltv = analyzer.calculate_ltv(&cohort_id);

        assert_eq!(ltv.total_revenue, Decimal::from(350));
        assert_eq!(ltv.user_count, 3);
        assert_eq!(ltv.paying_user_percentage, 1.0);
    }

    #[test]
    fn test_churn_features() {
        let base_date = DateTime::from_timestamp(1609459200, 0).unwrap();
        let current_date = base_date + Duration::days(30);

        let users = vec![create_test_user(
            "user1",
            base_date,
            vec![
                base_date,
                base_date + Duration::days(1),
                base_date + Duration::days(5),
                base_date + Duration::days(10),
            ],
            Decimal::from(100),
            4,
        )];

        let analyzer = CohortAnalyzer::new(users);
        let features = analyzer
            .calculate_churn_features("user1", current_date)
            .unwrap();

        assert_eq!(features.days_since_registration, 30);
        assert!(features.days_since_last_activity > 15); // Last activity was day 10
        assert!(features.activity_frequency > 0.0);
    }

    #[test]
    fn test_churn_probability() {
        let features = ChurnFeatures {
            user_id: "test".to_string(),
            days_since_last_activity: 5,
            days_since_registration: 100,
            activity_frequency: 0.5,
            revenue_per_day: Decimal::from(1),
            trade_frequency: 0.3,
            activity_trend: 0.1,
            recent_activity_ratio: 0.8,
        };

        let probability = features.churn_probability();
        assert!((0.0..=1.0).contains(&probability));
        assert_eq!(features.churn_risk_level(), ChurnRiskLevel::Low);
    }

    #[test]
    fn test_high_churn_risk() {
        let features = ChurnFeatures {
            user_id: "test".to_string(),
            days_since_last_activity: 45,
            days_since_registration: 100,
            activity_frequency: 0.05,
            revenue_per_day: Decimal::ZERO,
            trade_frequency: 0.01,
            activity_trend: -0.8,
            recent_activity_ratio: 0.1,
        };

        let probability = features.churn_probability();
        assert!(probability > 0.6);
        assert_eq!(features.churn_risk_level(), ChurnRiskLevel::High);
    }

    #[test]
    fn test_get_churn_risk_users() {
        let base_date = DateTime::from_timestamp(1609459200, 0).unwrap();
        let current_date = base_date + Duration::days(60);

        let users = vec![
            // Active user
            create_test_user(
                "user1",
                base_date,
                vec![
                    base_date,
                    current_date - Duration::days(2),
                    current_date - Duration::days(1),
                ],
                Decimal::from(100),
                3,
            ),
            // Inactive user (high churn risk)
            create_test_user(
                "user2",
                base_date,
                vec![base_date, base_date + Duration::days(1)],
                Decimal::ZERO,
                1,
            ),
        ];

        let analyzer = CohortAnalyzer::new(users);
        let at_risk = analyzer.get_churn_risk_users(current_date, ChurnRiskLevel::High);

        assert!(!at_risk.is_empty());
        assert!(at_risk.iter().any(|(id, _)| id == "user2"));
    }
}
