//! Storage module for SQLite database operations
//!
//! This module provides:
//! - Database connection management
//! - Schema migrations
//! - Repository pattern implementations for all entities

pub mod db;
pub mod paper_repo;
pub mod citation_repo;
pub mod connection_repo;
pub mod notes_repo;
pub mod job_repo;
pub mod project_repo;

pub use db::{Database, open_database, DatabaseError};
pub use paper_repo::PaperRepo;
pub use citation_repo::CitationRepo;
pub use connection_repo::ConnectionRepo;
pub use notes_repo::NotesRepo;
pub use job_repo::JobRepo;
pub use project_repo::ProjectRepo;
