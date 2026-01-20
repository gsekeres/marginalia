//! External service adapters
//!
//! This module contains adapters for external services and APIs:
//! - Unpaywall: Open access PDF lookup by DOI
//! - Semantic Scholar: Academic paper search and PDF lookup
//! - arXiv: Open access preprint lookup
//! - Claude CLI: LLM-powered PDF search and summarization
//! - Filesystem: PDF download, save, and summary file operations

pub mod arxiv;
pub mod claude_cli;
pub mod filesystem;
pub mod semantic_scholar;
pub mod unpaywall;

// Re-export commonly used types
pub use arxiv::ArxivClient;
pub use claude_cli::ClaudeCliClient;
pub use filesystem::FileSystemAdapter;
pub use semantic_scholar::SemanticScholarClient;
pub use unpaywall::UnpaywallClient;
