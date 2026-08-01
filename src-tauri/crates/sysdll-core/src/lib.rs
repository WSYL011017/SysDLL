//! SysDll core engine.
//!
//! Provides:
//! - PE binary analysis (import / export tables) via `pelite`
//! - Directory scanning for system DLLs
//! - Dependency graph construction
//! - Diagnostic rules (missing DLL detection, version mismatch, signature)
//!
//! All public types are `serde` friendly so the GUI / CLI can ship them over JSON.

pub mod diag;
pub mod error;
pub mod graph;
pub mod pe;
pub mod scan;

pub use diag::{Diagnostic, DiagnosticKind, Severity};
pub use error::{Error, Result};
pub use graph::{DependencyGraph, GraphStats};
pub use pe::{PeInfo, PeKind};
pub use scan::{ScanReport, ScanTarget, ScannedFile};
