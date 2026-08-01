//! Diagnostic rules.
//!
//! Given a [`ScanReport`] and its [`DependencyGraph`], produce a list of
//! [`Diagnostic`]s the GUI can render as rows. Each diagnostic carries enough
//! context (severity, related paths, dependents) for the user to decide on a repair.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::graph::DependencyGraph;
use crate::scan::ScanReport;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    MissingDll,
    OrphanDll,
    BrokenPeParse,
    CircularDependency,
    ParseFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    /// DLL file stem (without extension) or full path. Used by the repair queue.
    pub subject: String,
    pub related_paths: Vec<PathBuf>,
    pub dependents: Vec<String>,
}

pub fn run_diagnostics(report: &ScanReport, graph: &DependencyGraph) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let stats = graph.stats();

    for missing in &stats.missing_dlls {
        let dependents = graph.dependents_of(missing);
        let severity = if dependents.iter().any(|d| {
            d.contains("system") || d.contains("kernel") || d.contains("ntdll")
        }) {
            Severity::Critical
        } else if dependents.is_empty() {
            Severity::Info
        } else {
            Severity::Error
        };
        out.push(Diagnostic {
            kind: DiagnosticKind::MissingDll,
            severity,
            title: format!("Missing dependency: {missing}"),
            detail: format!(
                "{dependent_count} program(s) import {missing} but no copy was found on disk.",
                dependent_count = dependents.len()
            ),
            subject: missing.clone(),
            related_paths: Vec::new(),
            dependents,
        });
    }

    for file in &report.files {
        if let Some(err) = &file.error {
            out.push(Diagnostic {
                kind: DiagnosticKind::BrokenPeParse,
                severity: Severity::Warning,
                title: format!("Could not analyze: {}", file.path.display()),
                detail: err.clone(),
                subject: file.path.display().to_string(),
                related_paths: vec![file.path.clone()],
                dependents: Vec::new(),
            });
        }
    }

    for cycle in &stats.cyclic_paths {
        out.push(Diagnostic {
            kind: DiagnosticKind::CircularDependency,
            severity: Severity::Warning,
            title: format!("Circular dependency: {} modules", cycle.len()),
            detail: cycle.join(" -> "),
            subject: cycle.first().cloned().unwrap_or_default(),
            related_paths: Vec::new(),
            dependents: cycle.clone(),
        });
    }

    let orphan_stems = detect_orphans(report, graph);
    for (stem, path) in orphan_stems {
        out.push(Diagnostic {
            kind: DiagnosticKind::OrphanDll,
            severity: Severity::Info,
            title: format!("Orphan DLL: {stem}"),
            detail: "No scanned program imports this DLL; candidate for cleanup.".into(),
            subject: stem,
            related_paths: vec![PathBuf::from(path)],
            dependents: Vec::new(),
        });
    }

    out.sort_by(|a, b| b.severity.cmp(&a.severity));
    out
}

fn detect_orphans(report: &ScanReport, graph: &DependencyGraph) -> Vec<(String, String)> {
    let imported: BTreeSet<&String> = graph.reverse.keys().collect();
    let mut orphans = Vec::new();
    for file in &report.files {
        if file.pe.is_none() {
            continue;
        }
        let Some(stem) = file
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
        else {
            continue;
        };
        if !imported.contains(&stem) {
            orphans.push((stem, file.path.display().to_string()));
        }
    }
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::PeInfo;
    use crate::scan::ScannedFile;

    fn fake_report_with(dll: &str, imports: Vec<String>) -> ScanReport {
        ScanReport {
            targets: vec![],
            files: vec![ScannedFile {
                path: PathBuf::from(format!("C:/fake/{dll}.dll")),
                size: 1,
                pe: Some(PeInfo {
                    kind: crate::pe::PeKind::Pe32Plus,
                    machine: "x86_64".into(),
                    imports,
                    exports: vec![],
                    sha256: "deadbeef".into(),
                    file_size: 1,
                }),
                error: None,
            }],
            total_files: 1,
            parsed_files: 1,
            failed_files: 0,
            duration_ms: 0,
        }
    }

    #[test]
    fn missing_dependency_is_reported() {
        let report = fake_report_with("app", vec!["nonexistent.dll".into()]);
        let graph = DependencyGraph::from_scan(&report);
        let diags = run_diagnostics(&report, &graph);
        assert!(diags
            .iter()
            .any(|d| matches!(d.kind, DiagnosticKind::MissingDll) && d.subject == "nonexistent"));
    }
}
