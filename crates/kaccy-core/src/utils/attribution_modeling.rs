//! Attribution Modeling System
//!
//! Provides comprehensive attribution modeling for marketing and user acquisition, including:
//! - Multi-touch attribution models (first-touch, last-touch, linear, time-decay, U-shaped, W-shaped)
//! - User acquisition channel analysis
//! - Conversion funnel optimization
//! - Revenue attribution

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Marketing channel type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    /// Organic search (SEO)
    OrganicSearch,
    /// Paid search (Google Ads, Bing Ads)
    PaidSearch,
    /// Social media (organic)
    SocialMedia,
    /// Paid social (Facebook Ads, Twitter Ads, etc.)
    PaidSocial,
    /// Email marketing
    Email,
    /// Direct traffic
    Direct,
    /// Referral traffic
    Referral,
    /// Display advertising
    Display,
    /// Affiliate marketing
    Affiliate,
    /// Content marketing
    Content,
    /// Other/Unknown
    Other,
}

impl Channel {
    /// Get channel name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::OrganicSearch => "organic_search",
            Channel::PaidSearch => "paid_search",
            Channel::SocialMedia => "social_media",
            Channel::PaidSocial => "paid_social",
            Channel::Email => "email",
            Channel::Direct => "direct",
            Channel::Referral => "referral",
            Channel::Display => "display",
            Channel::Affiliate => "affiliate",
            Channel::Content => "content",
            Channel::Other => "other",
        }
    }
}

/// Touchpoint in the user journey
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Touchpoint {
    /// Touchpoint ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Channel
    pub channel: Channel,
    /// Campaign ID (if applicable)
    pub campaign_id: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Cost (if applicable)
    pub cost: Option<Decimal>,
}

/// Conversion event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    /// Conversion ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Revenue generated
    pub revenue: Decimal,
    /// Touchpoints leading to this conversion
    pub touchpoints: Vec<Touchpoint>,
}

/// Attribution model type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributionModel {
    /// All credit to first touchpoint
    FirstTouch,
    /// All credit to last touchpoint
    LastTouch,
    /// Equal credit to all touchpoints
    Linear,
    /// Time-decayed credit (more recent = more credit)
    TimeDecay,
    /// U-shaped (40% first, 40% last, 20% middle)
    UShaped,
    /// W-shaped (30% first, 30% middle, 30% last, 10% others)
    WShaped,
    /// Position-based (custom weights per position)
    PositionBased,
}

/// Attribution result for a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAttribution {
    /// Channel
    pub channel: Channel,
    /// Attributed conversions
    pub conversions: f64,
    /// Attributed revenue
    pub revenue: Decimal,
    /// Total cost
    pub cost: Decimal,
    /// Return on ad spend (ROAS)
    pub roas: f64,
    /// Cost per acquisition (CPA)
    pub cpa: Decimal,
}

impl ChannelAttribution {
    /// Calculate ROAS
    pub fn calculate_roas(&mut self) {
        if self.cost > Decimal::ZERO {
            self.roas = (self.revenue / self.cost)
                .to_string()
                .parse()
                .unwrap_or(0.0);
        } else {
            self.roas = 0.0;
        }
    }

    /// Calculate CPA
    pub fn calculate_cpa(&mut self) {
        if self.conversions > 0.0 {
            self.cpa =
                self.cost / Decimal::from_f64_retain(self.conversions).unwrap_or(Decimal::ONE);
        } else {
            self.cpa = Decimal::ZERO;
        }
    }
}

/// Attribution engine
pub struct AttributionEngine {
    /// Attribution model to use
    model: AttributionModel,
    /// Lookback window in days
    lookback_days: i64,
}

impl AttributionEngine {
    /// Create a new attribution engine
    pub fn new(model: AttributionModel, lookback_days: i64) -> Self {
        Self {
            model,
            lookback_days,
        }
    }

    /// Calculate attribution weights for touchpoints
    fn calculate_weights(
        &self,
        touchpoints: &[Touchpoint],
        conversion_time: DateTime<Utc>,
    ) -> Vec<f64> {
        if touchpoints.is_empty() {
            return vec![];
        }

        let n = touchpoints.len();
        match self.model {
            AttributionModel::FirstTouch => {
                let mut weights = vec![0.0; n];
                weights[0] = 1.0;
                weights
            }
            AttributionModel::LastTouch => {
                let mut weights = vec![0.0; n];
                weights[n - 1] = 1.0;
                weights
            }
            AttributionModel::Linear => vec![1.0 / n as f64; n],
            AttributionModel::TimeDecay => {
                // Half-life of 7 days
                let half_life_days = 7.0;
                let mut weights = Vec::with_capacity(n);
                let mut total_weight = 0.0;

                for touchpoint in touchpoints {
                    let days_before_conversion =
                        (conversion_time - touchpoint.timestamp).num_days() as f64;
                    let weight = 2.0_f64.powf(-days_before_conversion / half_life_days);
                    weights.push(weight);
                    total_weight += weight;
                }

                // Normalize
                if total_weight > 0.0 {
                    weights.iter_mut().for_each(|w| *w /= total_weight);
                }
                weights
            }
            AttributionModel::UShaped => {
                let mut weights = vec![0.0; n];
                if n == 1 {
                    weights[0] = 1.0;
                } else if n == 2 {
                    weights[0] = 0.5;
                    weights[1] = 0.5;
                } else {
                    weights[0] = 0.4;
                    weights[n - 1] = 0.4;
                    let middle_weight = 0.2 / (n - 2) as f64;
                    for weight in weights.iter_mut().take(n - 1).skip(1) {
                        *weight = middle_weight;
                    }
                }
                weights
            }
            AttributionModel::WShaped => {
                let mut weights = vec![0.0; n];
                if n == 1 {
                    weights[0] = 1.0;
                } else if n == 2 {
                    weights[0] = 0.5;
                    weights[1] = 0.5;
                } else if n == 3 {
                    weights[0] = 0.3;
                    weights[1] = 0.4;
                    weights[2] = 0.3;
                } else {
                    weights[0] = 0.3;
                    weights[n / 2] = 0.3;
                    weights[n - 1] = 0.3;
                    let remaining_weight = 0.1 / (n - 3) as f64;
                    for (i, weight) in weights.iter_mut().enumerate().take(n).skip(1) {
                        if i != n / 2 && i != n - 1 {
                            *weight = remaining_weight;
                        }
                    }
                }
                weights
            }
            AttributionModel::PositionBased => {
                // Similar to U-shaped for now
                let mut weights = vec![0.0; n];
                if n == 1 {
                    weights[0] = 1.0;
                } else {
                    weights[0] = 0.4;
                    weights[n - 1] = 0.4;
                    let middle_weight = 0.2 / (n - 2).max(1) as f64;
                    for weight in weights.iter_mut().take(n - 1).skip(1) {
                        *weight = middle_weight;
                    }
                }
                weights
            }
        }
    }

    /// Attribute a conversion
    pub fn attribute_conversion(
        &self,
        conversion: &Conversion,
    ) -> HashMap<Channel, ChannelAttribution> {
        // Filter touchpoints within lookback window
        let cutoff_time = conversion.timestamp - Duration::days(self.lookback_days);
        let relevant_touchpoints: Vec<Touchpoint> = conversion
            .touchpoints
            .iter()
            .filter(|tp| tp.timestamp >= cutoff_time)
            .cloned()
            .collect();

        if relevant_touchpoints.is_empty() {
            return HashMap::new();
        }

        // Calculate weights
        let weights = self.calculate_weights(&relevant_touchpoints, conversion.timestamp);

        // Attribute revenue and conversions
        let mut attributions: HashMap<Channel, ChannelAttribution> = HashMap::new();

        for (touchpoint, &weight) in relevant_touchpoints.iter().zip(weights.iter()) {
            let entry = attributions
                .entry(touchpoint.channel)
                .or_insert(ChannelAttribution {
                    channel: touchpoint.channel,
                    conversions: 0.0,
                    revenue: Decimal::ZERO,
                    cost: Decimal::ZERO,
                    roas: 0.0,
                    cpa: Decimal::ZERO,
                });

            entry.conversions += weight;
            entry.revenue +=
                conversion.revenue * Decimal::from_f64_retain(weight).unwrap_or(Decimal::ZERO);
            if let Some(cost) = touchpoint.cost {
                entry.cost += cost;
            }
        }

        // Calculate metrics
        for attribution in attributions.values_mut() {
            attribution.calculate_roas();
            attribution.calculate_cpa();
        }

        attributions
    }

    /// Attribute multiple conversions
    pub fn attribute_conversions(
        &self,
        conversions: &[Conversion],
    ) -> HashMap<Channel, ChannelAttribution> {
        let mut combined: HashMap<Channel, ChannelAttribution> = HashMap::new();

        for conversion in conversions {
            let attribution = self.attribute_conversion(conversion);
            for (channel, attr) in attribution {
                let entry = combined.entry(channel).or_insert(ChannelAttribution {
                    channel,
                    conversions: 0.0,
                    revenue: Decimal::ZERO,
                    cost: Decimal::ZERO,
                    roas: 0.0,
                    cpa: Decimal::ZERO,
                });

                entry.conversions += attr.conversions;
                entry.revenue += attr.revenue;
                entry.cost += attr.cost;
            }
        }

        // Recalculate metrics
        for attribution in combined.values_mut() {
            attribution.calculate_roas();
            attribution.calculate_cpa();
        }

        combined
    }
}

/// Funnel stage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FunnelStage {
    /// User visited site
    Visit,
    /// User signed up
    Signup,
    /// User made first deposit
    FirstDeposit,
    /// User made first trade
    FirstTrade,
    /// User is active (regular trader)
    Active,
    /// User retained (still active after 30 days)
    Retained,
}

impl FunnelStage {
    /// Get stage name
    pub fn as_str(&self) -> &'static str {
        match self {
            FunnelStage::Visit => "visit",
            FunnelStage::Signup => "signup",
            FunnelStage::FirstDeposit => "first_deposit",
            FunnelStage::FirstTrade => "first_trade",
            FunnelStage::Active => "active",
            FunnelStage::Retained => "retained",
        }
    }

    /// Get next stage
    pub fn next(&self) -> Option<FunnelStage> {
        match self {
            FunnelStage::Visit => Some(FunnelStage::Signup),
            FunnelStage::Signup => Some(FunnelStage::FirstDeposit),
            FunnelStage::FirstDeposit => Some(FunnelStage::FirstTrade),
            FunnelStage::FirstTrade => Some(FunnelStage::Active),
            FunnelStage::Active => Some(FunnelStage::Retained),
            FunnelStage::Retained => None,
        }
    }
}

/// User funnel event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelEvent {
    /// User ID
    pub user_id: String,
    /// Stage
    pub stage: FunnelStage,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Channel (how they reached this stage)
    pub channel: Option<Channel>,
}

/// Funnel metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelMetrics {
    /// Stage
    pub stage: FunnelStage,
    /// Number of users at this stage
    pub user_count: usize,
    /// Conversion rate to next stage
    pub conversion_rate: f64,
    /// Drop-off rate
    pub drop_off_rate: f64,
    /// Average time to next stage (in days)
    pub avg_time_to_next: Option<f64>,
}

/// Funnel analyzer
pub struct FunnelAnalyzer {
    events: Vec<FunnelEvent>,
}

impl FunnelAnalyzer {
    /// Create a new funnel analyzer
    pub fn new(events: Vec<FunnelEvent>) -> Self {
        Self { events }
    }

    /// Calculate funnel metrics
    pub fn calculate_metrics(&self) -> Vec<FunnelMetrics> {
        let stages = vec![
            FunnelStage::Visit,
            FunnelStage::Signup,
            FunnelStage::FirstDeposit,
            FunnelStage::FirstTrade,
            FunnelStage::Active,
            FunnelStage::Retained,
        ];

        let mut metrics = Vec::new();

        for stage in &stages {
            let users_at_stage: Vec<_> = self.events.iter().filter(|e| e.stage == *stage).collect();

            let user_count = users_at_stage.len();

            // Get next stage
            let (conversion_rate, drop_off_rate, avg_time_to_next) =
                if let Some(next_stage) = stage.next() {
                    let users_at_next: Vec<_> = self
                        .events
                        .iter()
                        .filter(|e| e.stage == next_stage)
                        .collect();

                    let next_count = users_at_next.len();
                    let conversion_rate = if user_count > 0 {
                        next_count as f64 / user_count as f64
                    } else {
                        0.0
                    };
                    let drop_off_rate = 1.0 - conversion_rate;

                    // Calculate average time to next stage
                    let mut time_deltas = Vec::new();
                    for user in &users_at_stage {
                        if let Some(next_event) = users_at_next
                            .iter()
                            .find(|e| e.user_id == user.user_id && e.timestamp > user.timestamp)
                        {
                            let delta = (next_event.timestamp - user.timestamp).num_days();
                            time_deltas.push(delta as f64);
                        }
                    }

                    let avg_time = if !time_deltas.is_empty() {
                        Some(time_deltas.iter().sum::<f64>() / time_deltas.len() as f64)
                    } else {
                        None
                    };

                    (conversion_rate, drop_off_rate, avg_time)
                } else {
                    (0.0, 0.0, None)
                };

            metrics.push(FunnelMetrics {
                stage: stage.clone(),
                user_count,
                conversion_rate,
                drop_off_rate,
                avg_time_to_next,
            });
        }

        metrics
    }

    /// Get funnel metrics by channel
    pub fn metrics_by_channel(&self, channel: Channel) -> Vec<FunnelMetrics> {
        let channel_events: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.channel == Some(channel))
            .cloned()
            .collect();

        let analyzer = FunnelAnalyzer::new(channel_events);
        analyzer.calculate_metrics()
    }

    /// Get conversion rate for a specific stage
    pub fn conversion_rate(&self, from: FunnelStage, to: FunnelStage) -> f64 {
        let from_users: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.stage == from)
            .map(|e| &e.user_id)
            .collect();

        let to_users: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.stage == to)
            .map(|e| &e.user_id)
            .collect();

        if from_users.is_empty() {
            return 0.0;
        }

        let converted = from_users
            .iter()
            .filter(|uid| to_users.contains(uid))
            .count();

        converted as f64 / from_users.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_touchpoint(
        id: &str,
        user_id: &str,
        channel: Channel,
        timestamp: DateTime<Utc>,
    ) -> Touchpoint {
        Touchpoint {
            id: id.to_string(),
            user_id: user_id.to_string(),
            channel,
            campaign_id: None,
            timestamp,
            cost: Some(Decimal::from(10)),
        }
    }

    #[test]
    fn test_first_touch_attribution() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let conversion = Conversion {
            id: "conv1".to_string(),
            user_id: "user1".to_string(),
            timestamp: base_time + Duration::days(10),
            revenue: Decimal::from(100),
            touchpoints: vec![
                create_test_touchpoint("tp1", "user1", Channel::OrganicSearch, base_time),
                create_test_touchpoint(
                    "tp2",
                    "user1",
                    Channel::PaidSearch,
                    base_time + Duration::days(5),
                ),
            ],
        };

        let engine = AttributionEngine::new(AttributionModel::FirstTouch, 30);
        let result = engine.attribute_conversion(&conversion);

        assert!(result.contains_key(&Channel::OrganicSearch));
        assert_eq!(result[&Channel::OrganicSearch].conversions, 1.0);
        assert_eq!(result[&Channel::OrganicSearch].revenue, Decimal::from(100));
    }

    #[test]
    fn test_last_touch_attribution() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let conversion = Conversion {
            id: "conv1".to_string(),
            user_id: "user1".to_string(),
            timestamp: base_time + Duration::days(10),
            revenue: Decimal::from(100),
            touchpoints: vec![
                create_test_touchpoint("tp1", "user1", Channel::OrganicSearch, base_time),
                create_test_touchpoint(
                    "tp2",
                    "user1",
                    Channel::PaidSearch,
                    base_time + Duration::days(5),
                ),
            ],
        };

        let engine = AttributionEngine::new(AttributionModel::LastTouch, 30);
        let result = engine.attribute_conversion(&conversion);

        assert!(result.contains_key(&Channel::PaidSearch));
        assert_eq!(result[&Channel::PaidSearch].conversions, 1.0);
        assert_eq!(result[&Channel::PaidSearch].revenue, Decimal::from(100));
    }

    #[test]
    fn test_linear_attribution() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let conversion = Conversion {
            id: "conv1".to_string(),
            user_id: "user1".to_string(),
            timestamp: base_time + Duration::days(10),
            revenue: Decimal::from(100),
            touchpoints: vec![
                create_test_touchpoint("tp1", "user1", Channel::OrganicSearch, base_time),
                create_test_touchpoint(
                    "tp2",
                    "user1",
                    Channel::PaidSearch,
                    base_time + Duration::days(5),
                ),
            ],
        };

        let engine = AttributionEngine::new(AttributionModel::Linear, 30);
        let result = engine.attribute_conversion(&conversion);

        assert_eq!(result[&Channel::OrganicSearch].conversions, 0.5);
        assert_eq!(result[&Channel::PaidSearch].conversions, 0.5);
        assert_eq!(result[&Channel::OrganicSearch].revenue, Decimal::from(50));
        assert_eq!(result[&Channel::PaidSearch].revenue, Decimal::from(50));
    }

    #[test]
    fn test_u_shaped_attribution() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let conversion = Conversion {
            id: "conv1".to_string(),
            user_id: "user1".to_string(),
            timestamp: base_time + Duration::days(10),
            revenue: Decimal::from(100),
            touchpoints: vec![
                create_test_touchpoint("tp1", "user1", Channel::OrganicSearch, base_time),
                create_test_touchpoint(
                    "tp2",
                    "user1",
                    Channel::Email,
                    base_time + Duration::days(3),
                ),
                create_test_touchpoint(
                    "tp3",
                    "user1",
                    Channel::PaidSearch,
                    base_time + Duration::days(5),
                ),
            ],
        };

        let engine = AttributionEngine::new(AttributionModel::UShaped, 30);
        let result = engine.attribute_conversion(&conversion);

        assert_eq!(result[&Channel::OrganicSearch].conversions, 0.4);
        assert_eq!(result[&Channel::PaidSearch].conversions, 0.4);
        assert_eq!(result[&Channel::Email].conversions, 0.2);
    }

    #[test]
    fn test_roas_calculation() {
        let mut attribution = ChannelAttribution {
            channel: Channel::PaidSearch,
            conversions: 10.0,
            revenue: Decimal::from(1000),
            cost: Decimal::from(100),
            roas: 0.0,
            cpa: Decimal::ZERO,
        };

        attribution.calculate_roas();
        assert_eq!(attribution.roas, 10.0);

        attribution.calculate_cpa();
        assert_eq!(attribution.cpa, Decimal::from(10));
    }

    #[test]
    fn test_funnel_metrics() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let events = vec![
            FunnelEvent {
                user_id: "user1".to_string(),
                stage: FunnelStage::Visit,
                timestamp: base_time,
                channel: Some(Channel::OrganicSearch),
            },
            FunnelEvent {
                user_id: "user1".to_string(),
                stage: FunnelStage::Signup,
                timestamp: base_time + Duration::days(1),
                channel: Some(Channel::OrganicSearch),
            },
            FunnelEvent {
                user_id: "user2".to_string(),
                stage: FunnelStage::Visit,
                timestamp: base_time,
                channel: Some(Channel::PaidSearch),
            },
        ];

        let analyzer = FunnelAnalyzer::new(events);
        let metrics = analyzer.calculate_metrics();

        let visit_metrics = metrics
            .iter()
            .find(|m| m.stage == FunnelStage::Visit)
            .unwrap();
        assert_eq!(visit_metrics.user_count, 2);
        assert_eq!(visit_metrics.conversion_rate, 0.5); // 1 out of 2 signed up
    }

    #[test]
    fn test_conversion_rate_between_stages() {
        let base_time = DateTime::from_timestamp(1609459200, 0).unwrap();
        let events = vec![
            FunnelEvent {
                user_id: "user1".to_string(),
                stage: FunnelStage::Visit,
                timestamp: base_time,
                channel: Some(Channel::OrganicSearch),
            },
            FunnelEvent {
                user_id: "user1".to_string(),
                stage: FunnelStage::Signup,
                timestamp: base_time + Duration::days(1),
                channel: Some(Channel::OrganicSearch),
            },
            FunnelEvent {
                user_id: "user1".to_string(),
                stage: FunnelStage::FirstTrade,
                timestamp: base_time + Duration::days(5),
                channel: Some(Channel::OrganicSearch),
            },
        ];

        let analyzer = FunnelAnalyzer::new(events);
        let rate = analyzer.conversion_rate(FunnelStage::Visit, FunnelStage::FirstTrade);
        assert_eq!(rate, 1.0);
    }
}
