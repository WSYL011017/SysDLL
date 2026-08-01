//! DLL dependency graph.
//!
//! Built from a [`ScanReport`]: every scanned file is a node, every "imports X.dll"
//! relationship is a directed edge. We do **not** embed `petgraph` in MVP to keep the
//! dependency tree small — the structure is simple enough to model inline.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::scan::ScanReport;

/// Adjacency map keyed by lowercased DLL file stem (e.g. `kernel32`, `user32`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyGraph {
    /// Full path of the canonical copy we know about, per DLL stem.
    pub nodes: BTreeMap<String, String>,
    /// Edge list: dependents → dependencies (lowercased stems).
    pub edges: BTreeMap<String, BTreeSet<String>>,
    /// Reverse lookup: dependency → set of dependents (for "who needs this?").
    pub reverse: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub missing_dlls: Vec<String>,
    pub cyclic_paths: Vec<Vec<String>>,
}

impl DependencyGraph {
    pub fn from_scan(report: &ScanReport) -> Self {
        let mut graph = Self::default();
        for file in &report.files {
            let Some(pe) = &file.pe else { continue };
            let stem = stem_from_path(&file.path);
            graph
                .nodes
                .entry(stem.clone())
                .or_insert_with(|| file.path.display().to_string());
            let entry = graph.edges.entry(stem.clone()).or_default();
            for import in &pe.imports {
                // Strip a trailing ".dll" / ".sys" / ".exe" so the graph keys
                // are bare stems ("kernel32") rather than decorated names.
                let import_stem = stem_from_dll_name(import);
                entry.insert(import_stem.clone());
                graph
                    .reverse
                    .entry(import_stem)
                    .or_default()
                    .insert(stem.clone());
            }
        }
        graph
    }

    pub fn stats(&self) -> GraphStats {
        let node_count = self.nodes.len();
        let edge_count = self.edges.values().map(|s| s.len()).sum();

        // Missing: every edge target that has no node entry.
        let mut referenced: HashSet<&String> = HashSet::new();
        for deps in self.edges.values() {
            for d in deps {
                referenced.insert(d);
            }
        }
        let mut missing: Vec<String> = referenced
            .into_iter()
            .filter(|d| !self.nodes.contains_key(*d))
            .cloned()
            .collect();
        missing.sort();

        let cyclic_paths = detect_cycles(&self.edges);

        GraphStats {
            node_count,
            edge_count,
            missing_dlls: missing,
            cyclic_paths,
        }
    }

    /// Reverse BFS from a missing DLL name to find every program that transitively
    /// depends on it. Used to prioritise repair actions.
    pub fn dependents_of(&self, stem: &str) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut out: Vec<String> = Vec::new();
        if let Some(initial) = self.reverse.get(&stem.to_ascii_lowercase()) {
            for d in initial {
                queue.push_back(d.clone());
            }
        }
        while let Some(node) = queue.pop_front() {
            if !seen.insert(node.clone()) {
                continue;
            }
            out.push(node.clone());
            if let Some(deps) = self.edges.get(&node) {
                for d in deps {
                    queue.push_back(d.clone());
                }
            }
        }
        out.sort();
        out
    }
}

fn stem_from_path(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem_from_dll_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    // Trim known extensions; otherwise keep as-is.
    for ext in [".dll", ".sys", ".exe", ".ocx"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    lower
}

/// Naive cycle detection (DFS) — the graph is small enough that we don't need Tarjan.
fn detect_cycles(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<(String, Vec<String>)> = Vec::new();

    for start in edges.keys() {
        if visited.contains(start) {
            continue;
        }
        stack.push((start.clone(), vec![start.clone()]));
        while let Some((node, path)) = stack.pop() {
            if let Some(deps) = edges.get(&node) {
                for dep in deps {
                    if let Some(pos) = path.iter().position(|p| p == dep) {
                        let mut cycle = path[pos..].to_vec();
                        cycle.push(dep.clone());
                        cycles.push(cycle);
                    } else if !visited.contains(dep) {
                        let mut next_path = path.clone();
                        next_path.push(dep.clone());
                        stack.push((dep.clone(), next_path));
                    }
                }
            }
            visited.insert(node);
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_yields_empty_graph() {
        let report = ScanReport::default();
        let graph = DependencyGraph::from_scan(&report);
        assert_eq!(graph.stats().node_count, 0);
    }
}
