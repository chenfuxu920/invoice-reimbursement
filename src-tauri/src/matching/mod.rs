pub mod batch;
pub mod batch_optimizer;
pub mod benchmarks;
pub mod engine;
pub mod manual;
pub mod scoring;
pub mod segment;
pub mod strategy_selector;

pub use batch::{batch_match, BatchMatchResult};
pub use batch_optimizer::{BatchMatchOptimizer, BatchMatchResult as OptimizedBatchMatchResult};
pub use engine::MatchEngine;
pub use manual::{create_manual_match, unmatch_invoice};
pub use scoring::{MatchScore, MultiDimensionalScorer, ScoreBreakdown, ScoringWeights};
pub use strategy_selector::{MatchingStrategy, StrategySelector, TimeAccuracy};
