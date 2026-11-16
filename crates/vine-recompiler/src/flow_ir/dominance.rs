//! Immediate dominators over the actual continuation CFG, followed by tree
//! intervals for constant-time dominance queries. No all-block ingress shortcut.
use super::{NodeId, ROOT, v};
pub(super) fn ranges(
    successors: &[Vec<NodeId>],
    predecessors: &[Vec<NodeId>],
) -> Result<Vec<(usize, usize)>, v::Error> {
    let bad = || v::Error("invalid or unreachable continuation CFG".into());
    let n = successors.len();
    if n == 0
        || predecessors.len() != n
        || successors
            .iter()
            .flatten()
            .chain(predecessors.iter().flatten())
            .any(|id| id.0 >= n)
    {
        return Err(bad());
    }
    let mut seen = vec![false; n];
    let mut post = vec![];
    let mut stack = vec![(ROOT.0, false)];
    while let Some((node, leaving)) = stack.pop() {
        if leaving {
            post.push(node);
            continue;
        }
        if std::mem::replace(&mut seen[node], true) {
            continue;
        }
        stack.push((node, true));
        for next in successors[node].iter().rev() {
            if !seen[next.0] {
                stack.push((next.0, false));
            }
        }
    }
    if post.len() != n {
        return Err(bad());
    }
    post.reverse();
    let mut rank = vec![0; n];
    for (r, &id) in post.iter().enumerate() {
        rank[id] = r;
    }
    let mut parent = vec![usize::MAX; n];
    parent[ROOT.0] = ROOT.0;
    loop {
        let mut changed = false;
        for &node in post.iter().skip(1) {
            let mut known = predecessors[node]
                .iter()
                .map(|n| n.0)
                .filter(|n| parent[*n] != usize::MAX);
            let Some(mut common) = known.next() else {
                continue;
            };
            for mut other in known {
                while common != other {
                    while rank[common] > rank[other] {
                        common = parent[common];
                    }
                    while rank[other] > rank[common] {
                        other = parent[other];
                    }
                }
            }
            if parent[node] != common {
                parent[node] = common;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut children = vec![vec![]; n];
    for node in 1..n {
        if parent[node] == usize::MAX {
            return Err(bad());
        }
        children[parent[node]].push(node);
    }
    let mut result = vec![(0, 0); n];
    let mut time = 0;
    let mut stack = vec![(ROOT.0, false)];
    while let Some((node, leaving)) = stack.pop() {
        if leaving {
            result[node].1 = time;
            time += 1;
            continue;
        }
        result[node].0 = time;
        time += 1;
        stack.push((node, true));
        for &next in children[node].iter().rev() {
            stack.push((next, false));
        }
    }
    Ok(result)
}
