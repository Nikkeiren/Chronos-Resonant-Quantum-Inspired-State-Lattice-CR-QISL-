//! Graph topology generation and partition-group assignment.

use crate::config::TopologyConfig;
use crate::rng::Rng;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub n: usize,
    /// Sorted adjacency lists (undirected, no self loops, no duplicates).
    pub adj: Vec<Vec<usize>>,
}

impl Graph {
    pub fn edge_count(&self) -> usize {
        self.adj.iter().map(|a| a.len()).sum::<usize>() / 2
    }

    pub fn edges(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, a) in self.adj.iter().enumerate() {
            for &j in a {
                if i < j {
                    out.push((i, j));
                }
            }
        }
        out
    }

    /// Connected components as a component id per node.
    pub fn components(&self) -> (Vec<usize>, usize) {
        let mut comp = vec![usize::MAX; self.n];
        let mut count = 0;
        for s in 0..self.n {
            if comp[s] != usize::MAX {
                continue;
            }
            let mut q = VecDeque::new();
            q.push_back(s);
            comp[s] = count;
            while let Some(u) = q.pop_front() {
                for &v in &self.adj[u] {
                    if comp[v] == usize::MAX {
                        comp[v] = count;
                        q.push_back(v);
                    }
                }
            }
            count += 1;
        }
        (comp, count)
    }

    pub fn is_connected(&self) -> bool {
        self.components().1 <= 1
    }

    /// BFS visiting order starting at `start`; unreachable nodes are appended
    /// by continuing from the lowest unvisited id.
    pub fn bfs_order(&self, start: usize) -> Vec<usize> {
        let mut seen = vec![false; self.n];
        let mut order = Vec::with_capacity(self.n);
        let mut starts: Vec<usize> = vec![start];
        starts.extend(0..self.n);
        for s in starts {
            if seen[s] {
                continue;
            }
            let mut q = VecDeque::new();
            q.push_back(s);
            seen[s] = true;
            while let Some(u) = q.pop_front() {
                order.push(u);
                for &v in &self.adj[u] {
                    if !seen[v] {
                        seen[v] = true;
                        q.push_back(v);
                    }
                }
            }
        }
        order
    }
}

fn to_graph(n: usize, sets: Vec<BTreeSet<usize>>) -> Graph {
    Graph {
        n,
        adj: sets.into_iter().map(|s| s.into_iter().collect()).collect(),
    }
}

fn add_edge(sets: &mut [BTreeSet<usize>], a: usize, b: usize) {
    if a != b {
        sets[a].insert(b);
        sets[b].insert(a);
    }
}

fn ring_lattice(n: usize, k: usize) -> Vec<BTreeSet<usize>> {
    let mut sets = vec![BTreeSet::new(); n];
    for i in 0..n {
        for d in 1..=(k / 2) {
            add_edge(&mut sets, i, (i + d) % n);
        }
    }
    sets
}

/// Build a graph from the topology configuration. Deterministic given `rng`.
pub fn build(cfg: &TopologyConfig, n: usize, rng: &mut Rng) -> Result<Graph, String> {
    let sets: Vec<BTreeSet<usize>> = match cfg.kind.as_str() {
        "ring" => ring_lattice(n, cfg.k),
        "complete" => {
            let mut sets = vec![BTreeSet::new(); n];
            for i in 0..n {
                for j in (i + 1)..n {
                    add_edge(&mut sets, i, j);
                }
            }
            sets
        }
        "grid" => {
            let side = (n as f64).sqrt().ceil() as usize;
            let mut sets = vec![BTreeSet::new(); n];
            for i in 0..n {
                let (r, c) = (i / side, i % side);
                if c + 1 < side && i + 1 < n {
                    add_edge(&mut sets, i, i + 1);
                }
                if r + 1 < side && i + side < n {
                    add_edge(&mut sets, i, i + side);
                }
            }
            sets
        }
        "erdos_renyi" => {
            let mut sets = vec![BTreeSet::new(); n];
            for i in 0..n {
                for j in (i + 1)..n {
                    if rng.next_f64() < cfg.p {
                        add_edge(&mut sets, i, j);
                    }
                }
            }
            sets
        }
        "small_world" => {
            // Watts-Strogatz: ring lattice, then rewire the "forward" edge of each node with prob p.
            let mut sets = ring_lattice(n, cfg.k);
            for i in 0..n {
                for d in 1..=(cfg.k / 2) {
                    let j = (i + d) % n;
                    if rng.next_f64() < cfg.p && sets[i].contains(&j) {
                        let c = rng.below(n);
                        if c != i && !sets[i].contains(&c) {
                            sets[i].remove(&j);
                            sets[j].remove(&i);
                            add_edge(&mut sets, i, c);
                        }
                    }
                }
            }
            sets
        }
        "scale_free" => {
            // Barabasi-Albert preferential attachment with an initial clique of m + 1 nodes.
            let m = cfg.m;
            let mut sets = vec![BTreeSet::new(); n];
            let mut repeated: Vec<usize> = Vec::new();
            for i in 0..=m {
                for j in (i + 1)..=m {
                    add_edge(&mut sets, i, j);
                    repeated.push(i);
                    repeated.push(j);
                }
            }
            for v in (m + 1)..n {
                let mut targets: BTreeSet<usize> = BTreeSet::new();
                let mut guard = 0;
                while targets.len() < m && guard < 10_000 {
                    guard += 1;
                    let t = repeated[rng.below(repeated.len())];
                    if t != v {
                        targets.insert(t);
                    }
                }
                for t in targets {
                    add_edge(&mut sets, v, t);
                    repeated.push(v);
                    repeated.push(t);
                }
            }
            sets
        }
        other => return Err(format!("unknown topology kind '{}'", other)),
    };
    let mut g = to_graph(n, sets);
    if cfg.ensure_connected && !g.is_connected() {
        let (comp, count) = g.components();
        let mut sets: Vec<BTreeSet<usize>> = g.adj.iter().map(|a| a.iter().cloned().collect()).collect();
        // Join every other component to component 0 through random members.
        for c in 1..count {
            let members_c: Vec<usize> = (0..n).filter(|&i| comp[i] == c).collect();
            let members_0: Vec<usize> = (0..n).filter(|&i| comp[i] == 0).collect();
            let a = members_c[rng.below(members_c.len())];
            let b = members_0[rng.below(members_0.len())];
            add_edge(&mut sets, a, b);
        }
        g = to_graph(n, sets);
    }
    Ok(g)
}

/// Partition group (0/1) per node. Group 0 contains `ceil(fraction * n)` nodes.
pub fn partition_groups(g: &Graph, mode: &str, fraction: f64, rng: &mut Rng) -> Vec<u8> {
    let n = g.n;
    let size0 = ((fraction * n as f64).ceil() as usize).clamp(1, n.saturating_sub(1).max(1));
    let order: Vec<usize> = match mode {
        "random" => {
            let mut v: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut v);
            v
        }
        "index" => (0..n).collect(),
        _ => g.bfs_order(0),
    };
    let mut groups = vec![1u8; n];
    for &i in order.iter().take(size0) {
        groups[i] = 0;
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(kind: &str) -> TopologyConfig {
        TopologyConfig {
            kind: kind.into(),
            ..TopologyConfig::default()
        }
    }

    fn check_valid(g: &Graph) {
        for (i, a) in g.adj.iter().enumerate() {
            let mut sorted = a.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(&sorted, a, "adjacency must be sorted and unique");
            assert!(!a.contains(&i), "no self loops");
            for &j in a {
                assert!(g.adj[j].contains(&i), "symmetric");
            }
        }
    }

    #[test]
    fn all_topologies_are_valid_and_connected() {
        for kind in ["ring", "erdos_renyi", "small_world", "scale_free", "grid", "complete"] {
            let mut c = cfg(kind);
            c.p = if kind == "erdos_renyi" { 0.03 } else { 0.1 };
            let mut rng = Rng::new(5);
            let g = build(&c, 60, &mut rng).unwrap();
            check_valid(&g);
            assert!(g.is_connected(), "{} must be connected", kind);
        }
    }

    #[test]
    fn deterministic_for_same_seed() {
        let c = cfg("small_world");
        let a = build(&c, 80, &mut Rng::new(11)).unwrap();
        let b = build(&c, 80, &mut Rng::new(11)).unwrap();
        let d = build(&c, 80, &mut Rng::new(12)).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, d);
    }

    #[test]
    fn ring_degree() {
        let mut c = cfg("ring");
        c.k = 4;
        let g = build(&c, 20, &mut Rng::new(1)).unwrap();
        assert!(g.adj.iter().all(|a| a.len() == 4));
    }

    #[test]
    fn partition_group_sizes() {
        let g = build(&cfg("small_world"), 100, &mut Rng::new(2)).unwrap();
        for mode in ["bfs", "random", "index"] {
            let groups = partition_groups(&g, mode, 0.65, &mut Rng::new(3));
            assert_eq!(groups.iter().filter(|&&x| x == 0).count(), 65);
        }
    }
}
