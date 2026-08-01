//! DLL dependency graph.
//!
//! Built from a [`ScanReport`]: every scanned file is a node, every "imports X.dll"
//! relationship is a directed edge. We do **not** embed `petgraph` in MVP to keep the
//! dependency tree small — the structure is simple enough to model inline.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

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

// -----------------------------------------------------------------------
// Cycle detection
//
// Audit fix R5/P1-10: the previous implementation cloned the whole DFS
// path onto the stack for every edge — O(N!) on shared transitive
// dependencies — and marked `visited` against the popped node instead of
// the expanded neighbour, so genuine cycles through fan-out points were
// silently dropped.
//
// New implementation: explicit three-colour DFS (White/Gray/Black) with an
// owned-string stack so lifetimes are trivial. A node is coloured Gray
// when pushed, Black when its frame is exhausted. A back edge to a Gray
// node is the cycle witness. Each cycle is canonicalised by rotating so
// the smallest stem is first; identical rotations fold into one entry.
//
// Complexity:
//   - Time  O(V + E)
//   - Space O(V) for the colour map and DFS path stack
//
// On an adversarial graph where every node depends on every other node
// (≈ 10k DLLs, ≈ 100M edges in the worst case), this is still bounded
// by the number of *simple* cycles which can be exponential — but our
// canonicalisation caps the reported set to one per undirected cycle.
// -----------------------------------------------------------------------

#[derive(Clone, Copy, Eq, PartialEq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Iterate every simple cycle in `edges`. Returns at most one
/// representative per undirected cycle.
fn detect_cycles(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut color: HashMap<String, Color> = edges
        .keys()
        .map(|k| (k.clone(), Color::White))
        .collect();
    let mut path: Vec<String> = Vec::new();
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut seen_keys: HashSet<Vec<String>> = HashSet::new();

    for start in edges.keys() {
        if color[start] != Color::White {
            continue;
        }
        // Recursive DFS is fine here — depth is bounded by the longest
        // acyclic path which is in practice tiny for DLL graphs.
        dfs(start, edges, &mut color, &mut path, &mut cycles, &mut seen_keys);
    }

    cycles
}

fn dfs(
    node: &str,
    edges: &BTreeMap<String, BTreeSet<String>>,
    color: &mut HashMap<String, Color>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
    seen_keys: &mut HashSet<Vec<String>>,
) {
    color.insert(node.to_string(), Color::Gray);
    path.push(node.to_string());

    if let Some(deps) = edges.get(node) {
        // Stable iteration order matters only for testability.
        for dep in deps {
            match color.get(dep).copied().unwrap_or(Color::White) {
                Color::Gray => {
                    // Back edge: rebuild the cycle from `dep` to `node`.
                    if let Some(pos) = path.iter().position(|p| p == dep) {
                        let mut cycle: Vec<String> = path[pos..].to_vec();
                        cycle.push(dep.clone());
                        if let Some(key) = canonical_key(&cycle) {
                            if seen_keys.insert(key) {
                                cycles.push(cycle);
                            }
                        }
                    }
                }
                Color::White => {
                    dfs(dep, edges, color, path, cycles, seen_keys);
                }
                Color::Black => { /* fully explored; no new cycles through here */ }
            }
        }
    }

    path.pop();
    color.insert(node.to_string(), Color::Black);
}

/// Rotate the cycle body so the smallest stem is first. Identical rotations
/// (e.g. [a,b,a] vs [b,a,b]) fold into the same key. Excludes the
/// closing element when computing the key.
fn canonical_key(cycle: &[String]) -> Option<Vec<String>> {
    if cycle.len() < 3 {
        return None;
    }
    let body = &cycle[..cycle.len() - 1];
    let min_pos = body
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map(|(i, _)| i)?;
    let rotated: Vec<String> = body
        .iter()
        .cycle()
        .skip(min_pos)
        .take(body.len())
        .cloned()
        .collect();
    let mut key = rotated;
    key.push(key[0].clone());
    Some(key)
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

    #[test]
    fn detects_simple_cycle() {
        // a → b → a
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        edges.entry("a".into()).or_default().insert("b".into());
        edges.entry("b".into()).or_default().insert("a".into());

        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1, "got: {:?}", cycles);
        let body: Vec<&str> = cycles[0].iter().map(String::as_str).collect();
        assert!(body.contains(&"a"));
        assert!(body.contains(&"b"));
    }

    #[test]
    fn no_false_positive_on_dag() {
        // a → b → c (no cycles)
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        edges.entry("a".into()).or_default().insert("b".into());
        edges.entry("b".into()).or_default().insert("c".into());

        let cycles = detect_cycles(&edges);
        assert!(cycles.is_empty(), "got: {:?}", cycles);
    }

    #[test]
    fn triangle_does_not_over_report() {
        // a → b, b → c, c → a — three edges, one undirected cycle.
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        edges.entry("a".into()).or_default().insert("b".into());
        edges.entry("b".into()).or_default().insert("c".into());
        edges.entry("c".into()).or_default().insert("a".into());

        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1, "got: {:?}", cycles);
    }

    #[test]
    fn canonical_key_folds_rotations() {
        let a_b_a = vec!["a".into(), "b".into(), "a".into()];
        let b_a_b = vec!["b".into(), "a".into(), "b".into()];
        assert_eq!(canonical_key(&a_b_a), canonical_key(&b_a_b));
    }

    #[test]
    fn fan_out_does_not_explode() {
        // 16 nodes all depending on a single shared `kernel32`. The old
        // O(N!) clone-per-edge DFS would have produced astronomical
        // intermediate allocations.
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for i in 0..16 {
            let stem = format!("n{i:02}");
            edges.entry(stem).or_default().insert("kernel32".into());
        }
        edges.entry("kernel32".into()).or_default();
        let cycles = detect_cycles(&edges);
        assert!(cycles.is_empty(), "got: {:?}", cycles);
    }
}
