//! Machine Learning Integration Module
//!
//! This module provides comprehensive machine learning capabilities including:
//! - Feature engineering framework for extracting trading features
//! - Price prediction models with confidence intervals
//! - Anomaly detection for unusual patterns
//! - Strategy optimization and hyperparameter tuning
//!
//! # Examples
//!
//! ## Feature Extraction
//!
//! ```
//! use kaccy_core::ml::features::{TechnicalFeatureExtractor, FeatureExtractor, PricePoint};
//! use rust_decimal_macros::dec;
//! use chrono::Utc;
//!
//! let data = vec![
//!     PricePoint {
//!         timestamp: Utc::now(),
//!         open: dec!(100),
//!         high: dec!(105),
//!         low: dec!(95),
//!         close: dec!(102),
//!         volume: dec!(1000),
//!     },
//!     // ... more data points
//! ];
//!
//! let extractor = TechnicalFeatureExtractor::new();
//! let features = extractor.extract(&data).unwrap();
//! ```
//!
//! ## Price Prediction
//!
//! ```no_run
//! use kaccy_core::ml::prediction::{LinearRegressionModel, PredictionModel};
//! use kaccy_core::ml::features::PricePoint;
//! use rust_decimal_macros::dec;
//! use chrono::Utc;
//!
//! let data = vec![
//!     PricePoint {
//!         timestamp: Utc::now(),
//!         open: dec!(100),
//!         high: dec!(105),
//!         low: dec!(95),
//!         close: dec!(102),
//!         volume: dec!(1000),
//!     },
//!     // ... more data points
//! ];
//!
//! let mut model = LinearRegressionModel::new();
//! model.train(&data).unwrap();
//! let predictions = model.predict(5).unwrap(); // Predict next 5 periods
//! ```
//!
//! ## Anomaly Detection
//!
//! ```
//! use kaccy_core::ml::anomaly::{ZScoreDetector, AnomalyDetector};
//! use kaccy_core::ml::features::PricePoint;
//! use rust_decimal_macros::dec;
//! use chrono::Utc;
//!
//! let data = vec![
//!     PricePoint {
//!         timestamp: Utc::now(),
//!         open: dec!(100),
//!         high: dec!(105),
//!         low: dec!(95),
//!         close: dec!(102),
//!         volume: dec!(1000),
//!     },
//!     // ... more data points
//! ];
//!
//! let mut detector = ZScoreDetector::default();
//! let anomalies = detector.detect(&data).unwrap();
//! ```
//!
//! ## Strategy Optimization
//!
//! ```
//! use kaccy_core::ml::optimization::{GridSearchOptimizer, Parameter, ObjectiveFunction, ParameterSet};
//!
//! struct MyStrategy;
//!
//! impl ObjectiveFunction for MyStrategy {
//!     fn evaluate(&self, parameters: &ParameterSet) -> anyhow::Result<f64> {
//!         // Evaluate strategy with given parameters
//!         Ok(0.0)
//!     }
//! }
//!
//! let params = vec![
//!     Parameter::new("period", 5.0, 50.0).with_step(5.0).as_integer(),
//!     Parameter::new("threshold", 0.01, 0.1).with_step(0.01),
//! ];
//!
//! let optimizer = GridSearchOptimizer::new(params);
//! let result = optimizer.optimize(&MyStrategy).unwrap();
//! ```

pub mod advanced_forecasting;
pub mod anomaly;
pub mod arima;
pub mod chart_patterns;
pub mod deep_learning;
pub mod ensemble;
pub mod feature_importance;
pub mod feature_transformations;
pub mod features;
pub mod garch;
pub mod gpu_acceleration;
pub mod liquidity_prediction;
pub mod model_persistence;
pub mod online_learning;
pub mod optimization;
pub mod order_flow_prediction;
pub mod prediction;
pub mod prophet;
pub mod reinforcement_learning;
pub mod sentiment_analysis;
pub mod volatility_forecasting;
pub mod volume_patterns;

pub use advanced_forecasting::{
    CombinationWeights, CombinedForecast, ForecastCombiner, MultiHorizonForecast,
    MultiHorizonForecaster, ProbabilisticForecast, ProbabilisticForecaster,
};
pub use anomaly::{
    Anomaly, AnomalyDetector, AnomalySeverity, AnomalyType, CompositeAnomalyDetector, IQRDetector,
    VolatilityDetector, ZScoreDetector,
};
pub use arima::{
    ArimaModel, ArimaParams, AutoArima, Forecast, SarimaParams, SeasonalDecomposition,
    TimeSeriesObservation,
};
pub use chart_patterns::{Candle, ChartPattern, ChartPatternDetector, PatternMatch};
pub use deep_learning::{
    AttentionFeatureSelector, AttentionMechanism, AttentionType, DLModelType, GANConfig, GANModel,
    LSTMConfig, LSTMModel, TransformerConfig, TransformerModel,
};
pub use ensemble::{
    BoostingConfig, BoostingEnsemble, EnsembleWeightOptimizer, OptimizationMethod, StackingConfig,
    StackingEnsemble,
};
pub use feature_importance::{
    FeatureImportance, FeatureImportanceAnalyzer, FeatureSelector, ImportanceMethod,
    ImportanceVisualizer,
};
pub use feature_transformations::{
    BinningStrategy, FeatureBinner, InteractionFeatureGenerator, LagFeatureGenerator,
    PolynomialFeatures, RollingFeatureGenerator, RollingStatistic,
};
pub use features::{
    CompositeFeatureExtractor, FeatureExtractor, FeatureVector, PricePoint,
    TechnicalFeatureExtractor, VolumeFeatureExtractor,
};
pub use garch::{
    GarchModel, GarchParams, GjrGarchModel, GjrGarchParams, MultivariateGarch, VolatilityCluster,
    VolatilityClustering,
};
pub use gpu_acceleration::{
    BacktestResult, GpuBackend, GpuBacktester, GpuDeviceInfo, GpuDeviceManager,
    GpuFeatureExtractor, GpuMatrix,
};
pub use liquidity_prediction::{
    DepthPrediction, DepthPredictor, IntradayLiquidityForecaster, LiquidityForecast,
    LiquiditySnapshot, MarketMakerBehavior, MarketMakerModeler, SpreadPrediction, SpreadPredictor,
};
pub use model_persistence::{
    ModelComparison, ModelMetadata, ModelRegistry, ModelVersion, ModelVersionManager,
    PersistentModel,
};
pub use online_learning::{
    AdaptiveOnlineModel, DriftDetector, DriftStatus, IncrementalStats, OnlineLearner,
    OnlineLearningConfig, OnlineLinearRegression, OnlineMovingAverage,
};
pub use optimization::{
    BayesianOptimizer, GeneticOptimizer, GridSearchOptimizer, ObjectiveFunction,
    OptimizationResult, Parameter, ParameterSet, RandomSearchOptimizer,
};
pub use order_flow_prediction::{
    CancellationPrediction, CancellationPredictor, InterArrivalTimeAnalyzer, InterArrivalTimeModel,
    OrderDirection, OrderEvent, OrderFlowPredictor, OrderPrediction, OrderSizeAnalyzer,
    OrderSizeDistribution,
};
pub use prediction::{
    EnsembleModel, LinearRegressionModel, ModelPerformance, MovingAveragePredictionModel,
    PredictionModel, PricePrediction,
};
pub use prophet::{
    Changepoint, GrowthType, Holiday, ProphetModel, SeasonalComponent, SeasonalPeriod,
    SeasonalityDetection, TrendDetection, TrendType,
};
pub use reinforcement_learning::{
    AdversarialTraining, CooperationMode, Experience, MarketState, MultiAgentSystem, RLAlgorithm,
    RLTradingAgent, TradingAction, TransferLearning, TransferMethod,
};
pub use sentiment_analysis::{
    AggregatedSentiment, ContrarianIndicator, ContrarianSignal, ContrarianSignalType,
    SentimentAggregator, SentimentClassification, SentimentDivergence, SentimentDivergenceDetector,
    SentimentDivergenceType, SentimentReading, SentimentSource, SentimentTrend,
    SentimentTrendAnalyzer, TrendDirection,
};
pub use volatility_forecasting::{
    ImpliedVolatility, JumpDetectionResult, JumpDetector, JumpDirection, JumpEvent, JumpPrediction,
    RealizedVolatilityForecast, RealizedVolatilityForecaster, VolatilitySurface,
    VolatilitySurfaceModeler, VolatilitySurfacePoint,
};
pub use volume_patterns::{
    ADSignal, AccumulationDistribution, AccumulationDistributionAnalyzer, DivergenceType,
    VolumeDivergence, VolumeDivergenceDetector, VolumeProfile, VolumeProfileAnalyzer,
    WyckoffAnalysis, WyckoffAnalyzer, WyckoffPhase,
};
