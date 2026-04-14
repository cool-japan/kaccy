//! Bonding Curve Optimization
//!
//! This module implements multi-segment bonding curves, curve switching mechanisms,
//! and parameter evolution over time for optimized price discovery.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Multi-segment curve configuration
///
/// Allows different curve types or parameters at different supply ranges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSegmentCurve {
    /// Ordered list of contiguous curve segments
    pub segments: Vec<CurveSegment>,
}

/// A single supply-range segment within a multi-segment curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveSegment {
    /// Start of supply range for this segment (inclusive)
    pub supply_start: Decimal,
    /// End of supply range for this segment (exclusive)
    pub supply_end: Decimal,
    /// Curve type for this segment
    pub curve_type: SegmentCurveType,
    /// Base price at the start of this segment
    pub base_price: Decimal,
}

/// Bonding curve formula for a single segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SegmentCurveType {
    /// P = base_price + slope × supply
    Linear {
        /// Price increase per unit of supply
        slope: Decimal,
    },
    /// P = base_price × e^(rate × supply)
    Exponential {
        /// Exponential growth rate
        rate: Decimal,
    },
    /// P = base_price + scale × ln(1 + supply)
    Logarithmic {
        /// Logarithmic scale factor
        scale: Decimal,
    },
    /// P = base_price + coefficient × sqrt(supply)
    SquareRoot {
        /// Square-root scale coefficient
        coefficient: Decimal,
    },
}

impl MultiSegmentCurve {
    /// Create a new multi-segment curve, validating that segments are contiguous
    pub fn new(segments: Vec<CurveSegment>) -> Result<Self, CoreError> {
        if segments.is_empty() {
            return Err(CoreError::Validation(
                "Multi-segment curve must have at least one segment".to_string(),
            ));
        }

        // Validate segments are contiguous
        for i in 1..segments.len() {
            if segments[i].supply_start != segments[i - 1].supply_end {
                return Err(CoreError::Validation(
                    "Curve segments must be contiguous".to_string(),
                ));
            }
        }

        Ok(Self { segments })
    }

    /// Finds the appropriate segment for a given supply
    fn find_segment(&self, supply: Decimal) -> Option<&CurveSegment> {
        self.segments
            .iter()
            .find(|seg| supply >= seg.supply_start && supply < seg.supply_end)
    }

    /// Calculates price at a specific supply level
    pub fn price_at_supply(&self, supply: Decimal) -> Result<Decimal, CoreError> {
        let segment = self
            .find_segment(supply)
            .ok_or_else(|| CoreError::Validation("Supply out of curve range".to_string()))?;

        let supply_in_segment = supply - segment.supply_start;

        let price = match &segment.curve_type {
            SegmentCurveType::Linear { slope } => segment.base_price + (*slope * supply_in_segment),
            SegmentCurveType::Exponential { rate } => {
                // P = base_price * e^(rate * supply_in_segment)
                let exp_factor = (*rate * supply_in_segment)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    .exp();
                segment.base_price * Decimal::try_from(exp_factor).unwrap_or(Decimal::ONE)
            }
            SegmentCurveType::Logarithmic { scale } => {
                // P = base_price + scale * ln(1 + supply_in_segment)
                let ln_factor =
                    (1.0 + supply_in_segment.to_string().parse::<f64>().unwrap_or(0.0)).ln();
                segment.base_price
                    + (*scale * Decimal::try_from(ln_factor).unwrap_or(Decimal::ZERO))
            }
            SegmentCurveType::SquareRoot { coefficient } => {
                // P = base_price + coefficient * sqrt(supply_in_segment)
                let sqrt_factor = supply_in_segment
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    .sqrt();
                segment.base_price
                    + (*coefficient * Decimal::try_from(sqrt_factor).unwrap_or(Decimal::ZERO))
            }
        };

        Ok(price)
    }

    /// Calculates cost to buy a certain amount
    pub fn buy_cost(&self, from_supply: Decimal, amount: Decimal) -> Result<Decimal, CoreError> {
        let to_supply = from_supply + amount;

        // Check if crosses multiple segments
        let from_segment_idx = self
            .segments
            .iter()
            .position(|seg| from_supply >= seg.supply_start && from_supply < seg.supply_end)
            .ok_or_else(|| CoreError::Validation("From supply out of range".to_string()))?;

        let to_segment_idx = self
            .segments
            .iter()
            .position(|seg| to_supply > seg.supply_start && to_supply <= seg.supply_end)
            .ok_or_else(|| CoreError::Validation("To supply out of range".to_string()))?;

        if from_segment_idx == to_segment_idx {
            // Within single segment - use average price
            let avg_price = (self.price_at_supply(from_supply)?
                + self.price_at_supply(to_supply)?)
                / Decimal::TWO;
            Ok(avg_price * amount)
        } else {
            // Crosses multiple segments - sum costs
            let mut total_cost = Decimal::ZERO;
            let mut current_supply = from_supply;

            for idx in from_segment_idx..=to_segment_idx {
                let segment = &self.segments[idx];
                let segment_end = if idx == to_segment_idx {
                    to_supply
                } else {
                    segment.supply_end
                };

                let segment_amount = segment_end - current_supply;
                let avg_price = (self.price_at_supply(current_supply)?
                    + self.price_at_supply(segment_end)?)
                    / Decimal::TWO;
                total_cost += avg_price * segment_amount;

                current_supply = segment_end;
            }

            Ok(total_cost)
        }
    }
}

/// Curve switching mechanism
///
/// Allows transitioning between different curve types based on conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveSwitchingManager {
    /// ID of the currently active curve
    pub current_curve_id: String,
    /// All curves that can be activated
    pub available_curves: Vec<NamedCurve>,
    /// Ordered rules that trigger automatic curve switches
    pub switching_rules: Vec<SwitchingRule>,
    /// Chronological log of past curve switches
    pub switch_history: Vec<SwitchEvent>,
}

/// A bonding curve with a human-readable identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedCurve {
    /// Unique string identifier for this curve
    pub curve_id: String,
    /// Mathematical configuration of the curve
    pub curve_config: CurveConfig,
}

/// Bonding curve configuration variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CurveConfig {
    /// Linear bonding curve
    Linear {
        /// Price slope per unit supply
        slope: Decimal,
        /// Price at zero supply
        base_price: Decimal,
    },
    /// Exponential bonding curve
    Exponential {
        /// Growth rate exponent
        rate: Decimal,
        /// Price at zero supply
        base_price: Decimal,
    },
    /// S-shaped sigmoid bonding curve
    Sigmoid {
        /// Supply at which the curve is steepest
        midpoint: Decimal,
        /// Steepness of the S-curve transition
        steepness: Decimal,
    },
}

/// A rule that triggers a curve switch when a condition is met
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchingRule {
    /// Unique identifier of the rule
    pub rule_id: String,
    /// Curve that must be active for this rule to fire
    pub from_curve_id: String,
    /// Curve to switch to when the condition is met
    pub to_curve_id: String,
    /// Condition that triggers the switch
    pub condition: SwitchCondition,
}

/// Condition that triggers an automated curve switch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwitchCondition {
    /// Switch when supply reaches threshold
    SupplyThreshold {
        /// Supply level that triggers the switch.
        threshold: Decimal,
    },
    /// Switch when price reaches threshold
    PriceThreshold {
        /// Price level that triggers the switch.
        threshold: Decimal,
    },
    /// Switch when volatility exceeds threshold
    VolatilityThreshold {
        /// Volatility level that triggers the switch.
        threshold: Decimal,
    },
    /// Switch after time period
    TimeElapsed {
        /// Duration to wait before switching.
        duration: Duration,
    },
}

/// Record of a completed curve switch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchEvent {
    /// When the switch occurred
    pub timestamp: SystemTime,
    /// Curve that was active before the switch
    pub from_curve_id: String,
    /// Curve that became active after the switch
    pub to_curve_id: String,
    /// Description of the condition that triggered the switch
    pub trigger_reason: String,
}

impl CurveSwitchingManager {
    /// Create a new curve switching manager
    pub fn new(
        current_curve_id: String,
        available_curves: Vec<NamedCurve>,
        switching_rules: Vec<SwitchingRule>,
    ) -> Self {
        Self {
            current_curve_id,
            available_curves,
            switching_rules,
            switch_history: Vec::new(),
        }
    }

    /// Evaluates switching rules and returns new curve ID if switch should occur
    pub fn evaluate_switch(
        &mut self,
        current_supply: Decimal,
        current_price: Decimal,
        current_volatility: Decimal,
        elapsed_time: Duration,
    ) -> Option<String> {
        for rule in &self.switching_rules {
            if rule.from_curve_id != self.current_curve_id {
                continue;
            }

            let should_switch = match &rule.condition {
                SwitchCondition::SupplyThreshold { threshold } => current_supply >= *threshold,
                SwitchCondition::PriceThreshold { threshold } => current_price >= *threshold,
                SwitchCondition::VolatilityThreshold { threshold } => {
                    current_volatility >= *threshold
                }
                SwitchCondition::TimeElapsed { duration } => elapsed_time >= *duration,
            };

            if should_switch {
                self.switch_history.push(SwitchEvent {
                    timestamp: SystemTime::now(),
                    from_curve_id: self.current_curve_id.clone(),
                    to_curve_id: rule.to_curve_id.clone(),
                    trigger_reason: format!("{:?}", rule.condition),
                });

                self.current_curve_id = rule.to_curve_id.clone();
                return Some(rule.to_curve_id.clone());
            }
        }

        None
    }

    /// Gets the current active curve configuration
    pub fn get_current_curve(&self) -> Option<&NamedCurve> {
        self.available_curves
            .iter()
            .find(|c| c.curve_id == self.current_curve_id)
    }
}

/// Parameter evolution
///
/// Allows curve parameters to evolve over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterEvolution {
    /// Name of the parameter being evolved
    pub parameter_name: String,
    /// Mathematical profile of the evolution
    pub evolution_type: EvolutionType,
    /// Parameter value at the start of the evolution
    pub start_value: Decimal,
    /// Parameter value at the end of the evolution
    pub end_value: Decimal,
    /// Wall-clock time when the evolution began
    pub start_time: SystemTime,
    /// Total duration of the evolution from start to end
    pub duration: Duration,
}

/// Mathematical curve describing how a parameter evolves over time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionType {
    /// Linear interpolation from start to end
    Linear,
    /// Exponential growth/decay from start to end
    Exponential,
    /// Logarithmic progression from start to end
    Logarithmic,
    /// S-shaped sigmoid transition from start to end
    Sigmoid,
}

impl ParameterEvolution {
    /// Create a new parameter evolution starting now
    pub fn new(
        parameter_name: String,
        evolution_type: EvolutionType,
        start_value: Decimal,
        end_value: Decimal,
        duration: Duration,
    ) -> Self {
        Self {
            parameter_name,
            evolution_type,
            start_value,
            end_value,
            start_time: SystemTime::now(),
            duration,
        }
    }

    /// Calculates the current parameter value based on elapsed time
    pub fn current_value(&self) -> Decimal {
        let elapsed = SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or(Duration::ZERO);

        if elapsed >= self.duration {
            return self.end_value;
        }

        let progress = Decimal::try_from(elapsed.as_secs_f64() / self.duration.as_secs_f64())
            .unwrap_or(Decimal::ZERO);

        let value_range = self.end_value - self.start_value;

        match self.evolution_type {
            EvolutionType::Linear => self.start_value + (value_range * progress),
            EvolutionType::Exponential => {
                // Exponential growth: start * (end/start)^progress
                let ratio = (self.end_value / self.start_value)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(1.0);
                let exp_factor = ratio.powf(progress.to_string().parse::<f64>().unwrap_or(0.0));
                self.start_value * Decimal::try_from(exp_factor).unwrap_or(Decimal::ONE)
            }
            EvolutionType::Logarithmic => {
                // Logarithmic: start + range * log(1 + progress) / log(2)
                let log_progress =
                    (1.0 + progress.to_string().parse::<f64>().unwrap_or(0.0)).ln() / 2.0_f64.ln();
                self.start_value
                    + (value_range * Decimal::try_from(log_progress).unwrap_or(Decimal::ZERO))
            }
            EvolutionType::Sigmoid => {
                // Sigmoid: start + range / (1 + e^(-10*(progress - 0.5)))
                let x = progress.to_string().parse::<f64>().unwrap_or(0.0);
                let sigmoid = 1.0 / (1.0 + (-10.0 * (x - 0.5)).exp());
                self.start_value
                    + (value_range * Decimal::try_from(sigmoid).unwrap_or(Decimal::ZERO))
            }
        }
    }

    /// Checks if evolution is complete
    pub fn is_complete(&self) -> bool {
        let elapsed = SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or(Duration::ZERO);

        elapsed >= self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_segment_curve() {
        let segments = vec![
            CurveSegment {
                supply_start: Decimal::ZERO,
                supply_end: Decimal::new(1000, 0),
                curve_type: SegmentCurveType::Linear {
                    slope: Decimal::new(1, 3), // 0.001
                },
                base_price: Decimal::new(1, 1), // 0.1
            },
            CurveSegment {
                supply_start: Decimal::new(1000, 0),
                supply_end: Decimal::new(10000, 0),
                curve_type: SegmentCurveType::Linear {
                    slope: Decimal::new(2, 3), // 0.002
                },
                base_price: Decimal::new(11, 1), // 1.1
            },
        ];

        let curve = MultiSegmentCurve::new(segments).unwrap();

        // Test price in first segment
        let price1 = curve.price_at_supply(Decimal::new(500, 0)).unwrap();
        assert!(price1 > Decimal::new(1, 1)); // Should be > 0.1

        // Test price in second segment
        let price2 = curve.price_at_supply(Decimal::new(5000, 0)).unwrap();
        assert!(price2 > Decimal::new(11, 1)); // Should be > 1.1
    }

    #[test]
    fn test_curve_switching() {
        let curves = vec![
            NamedCurve {
                curve_id: "early".to_string(),
                curve_config: CurveConfig::Linear {
                    slope: Decimal::new(1, 3),
                    base_price: Decimal::new(1, 1),
                },
            },
            NamedCurve {
                curve_id: "mature".to_string(),
                curve_config: CurveConfig::Linear {
                    slope: Decimal::new(5, 4),
                    base_price: Decimal::ONE,
                },
            },
        ];

        let rules = vec![SwitchingRule {
            rule_id: "supply_threshold".to_string(),
            from_curve_id: "early".to_string(),
            to_curve_id: "mature".to_string(),
            condition: SwitchCondition::SupplyThreshold {
                threshold: Decimal::new(10000, 0),
            },
        }];

        let mut manager = CurveSwitchingManager::new("early".to_string(), curves, rules);

        // Should not switch yet
        let result1 = manager.evaluate_switch(
            Decimal::new(5000, 0),
            Decimal::new(5, 0),
            Decimal::new(1, 1),
            Duration::from_secs(3600),
        );
        assert!(result1.is_none());
        assert_eq!(manager.current_curve_id, "early");

        // Should switch now
        let result2 = manager.evaluate_switch(
            Decimal::new(15000, 0),
            Decimal::new(5, 0),
            Decimal::new(1, 1),
            Duration::from_secs(3600),
        );
        assert_eq!(result2, Some("mature".to_string()));
        assert_eq!(manager.current_curve_id, "mature");
    }

    #[test]
    fn test_parameter_evolution_linear() {
        let evolution = ParameterEvolution::new(
            "slope".to_string(),
            EvolutionType::Linear,
            Decimal::new(1, 2), // 0.01
            Decimal::new(1, 1), // 0.1
            Duration::from_secs(100),
        );

        // At start
        let value = evolution.current_value();
        assert!(value >= Decimal::new(1, 2));
        assert!(value <= Decimal::new(1, 1));
    }

    #[test]
    fn test_multi_segment_buy_cost() {
        let segments = vec![CurveSegment {
            supply_start: Decimal::ZERO,
            supply_end: Decimal::new(100, 0),
            curve_type: SegmentCurveType::Linear {
                slope: Decimal::new(1, 2), // 0.01
            },
            base_price: Decimal::ONE,
        }];

        let curve = MultiSegmentCurve::new(segments).unwrap();

        let cost = curve
            .buy_cost(Decimal::new(10, 0), Decimal::new(10, 0))
            .unwrap();
        assert!(cost > Decimal::ZERO);
    }
}
