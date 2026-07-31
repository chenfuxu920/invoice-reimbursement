pub mod engine;
pub mod batch;
pub mod manual;
pub mod scoring;
pub mod strategy_selector;
pub mod batch_optimizer;
pub mod benchmarks;
pub mod segment;

pub use engine::MatchEngine;
pub use batch::{batch_match, BatchMatchResult};
pub use manual::{create_manual_match, unmatch_invoice};
pub use scoring::{MatchScore, ScoreBreakdown, ScoringWeights, MultiDimensionalScorer};
pub use strategy_selector::{MatchingStrategy, StrategySelector, TimeAccuracy};
pub use batch_optimizer::{BatchMatchOptimizer, BatchMatchResult as OptimizedBatchMatchResult};
