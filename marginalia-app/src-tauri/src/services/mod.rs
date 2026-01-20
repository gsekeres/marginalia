//! Services module for business logic
//!
//! This module contains service implementations that coordinate
//! between adapters, storage, and commands.

pub mod job_manager;
pub mod summarizer_service;

pub use job_manager::JobManager;
pub use summarizer_service::{SummarizerService, SummarizationResult, ClaudeSummaryOutput};
