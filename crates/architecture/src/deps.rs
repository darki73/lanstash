//! Dependency rules: every crate is covered by the policy, depends only on the
//! workspace crates its row allows, the graph is acyclic, platforms build only
//! on core, and no third-party crate is in the lock file.

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::{DependencyKind, lock_packages};
use crate::policy::{Layer, Policy};
use crate::violation::Violation;
use crate::workspace::Workspace;

/// Every member and every manifest directory under `crates/` has a policy row,
/// at the directory the row names; every row is a member.
#[must_use]
pub fn check_policy_coverage(ws: &Workspace, policy: &Policy) -> Vec<Violation> {
    let mut violations = Vec::new();
    for c in &ws.crates {
        match policy.crate_policy(c.name()) {
            None => violations.push(Violation::new(c.name(), "policy-coverage", "crate has no policy entry")),
            Some(p) if p.dir != c.dir => violations.push(Violation::new(
                c.name(),
                "policy-coverage",
                format!("crate is at {}, its policy says {}", c.dir, p.dir),
            )),
            Some(_) => {}
        }
    }
    for dir in &ws.manifest_dirs {
        if !ws.members.contains(dir) {
            violations.push(Violation::new(dir, "policy-coverage", "manifest is not a workspace member"));
        }
    }
    for p in policy.crates {
        if !ws.is_member(p.package) {
            violations.push(Violation::new(p.package, "policy-coverage", "policy entry is not a workspace member"));
        }
    }
    violations
}

/// Dependencies on workspace crates are allowed by the row (`internal`, and
/// `dev_internal` for dev dependencies); any other dependency is third-party.
#[must_use]
pub fn check_internal_dependencies(ws: &Workspace, policy: &Policy) -> Vec<Violation> {
    let mut violations = Vec::new();
    for c in &ws.crates {
        let Some(rules) = policy.crate_policy(c.name()) else { continue };
        for d in &c.manifest.dependencies {
            if !ws.is_member(&d.name) {
                let direct = policy
                    .direct_external
                    .iter()
                    .any(|(pkg, crates)| *pkg == c.name() && crates.contains(&d.name.as_str()));
                if !(policy.external.contains(&d.name.as_str()) && direct) {
                    violations.push(Violation::new(
                        c.name(),
                        "external-dependency",
                        format!("{} is not an approved dependency", d.name),
                    ));
                }
                continue;
            }
            let allowed = rules.internal.contains(&d.name.as_str())
                || (d.kind == DependencyKind::Dev && rules.dev_internal.contains(&d.name.as_str()));
            if !allowed {
                violations.push(Violation::new(
                    c.name(),
                    "internal-dependency",
                    format!("{:?} dependency on {} is not allowed", d.kind, d.name),
                ));
            }
        }
    }
    violations
}

/// Core crates never depend on a platform or an adapter; a platform depends
/// only on core crates and its own crates. Checked on the policy rows, so the
/// table itself cannot allow it.
#[must_use]
pub fn check_layers(policy: &Policy) -> Vec<Violation> {
    let mut violations = Vec::new();
    let layer = |p: &str| policy.crate_policy(p).map(|c| c.layer);
    for c in policy.crates {
        for d in c.internal.iter().chain(c.dev_internal) {
            let Some(to) = layer(d) else {
                violations.push(Violation::new(c.package, "layers", format!("{d} has no policy entry")));
                continue;
            };
            let bad = match (c.layer, to) {
                (Layer::Platform(a), Layer::Platform(b)) => {
                    (a != b).then(|| format!("platform {a} names platform {b}"))
                }
                (Layer::Platform(a), to) if !to.is_core() => Some(format!("platform {a} names {d}, which is not core")),
                (from, Layer::Platform(b)) if from.is_core() || from == Layer::Adapter => {
                    Some(format!("{d} is platform {b}; only the composition root names platforms"))
                }
                (from, Layer::Adapter) if from.is_core() => {
                    Some(format!("{d} is an adapter; the core reaches adapters only through ports"))
                }
                _ => None,
            };
            if let Some(why) = bad {
                violations.push(Violation::new(c.package, "layers", why));
            }
        }
    }
    violations
}

/// The graph of workspace dependencies of every kind is acyclic. A crate's dev
/// dependency on itself (to turn on its own test features) is not an edge.
#[must_use]
pub fn check_acyclic(ws: &Workspace) -> Vec<Violation> {
    let graph: BTreeMap<&str, Vec<&str>> = ws
        .crates
        .iter()
        .map(|c| {
            let edges = c
                .manifest
                .dependencies
                .iter()
                .filter(|d| ws.is_member(&d.name) && d.name != c.name())
                .map(|d| d.name.as_str())
                .collect();
            (c.name(), edges)
        })
        .collect();
    let mut finished = BTreeSet::new();
    let mut violations = Vec::new();
    for start in graph.keys() {
        visit(start, &graph, &mut Vec::new(), &mut finished, &mut violations);
    }
    violations
}

fn visit<'a>(
    node: &'a str,
    graph: &BTreeMap<&'a str, Vec<&'a str>>,
    path: &mut Vec<&'a str>,
    finished: &mut BTreeSet<&'a str>,
    violations: &mut Vec<Violation>,
) {
    if finished.contains(node) {
        return;
    }
    if let Some(at) = path.iter().position(|v| *v == node) {
        let mut cycle = path[at..].to_vec();
        cycle.push(node);
        violations.push(Violation::new(node, "acyclic", cycle.join(" -> ")));
        return;
    }
    path.push(node);
    for next in graph.get(node).into_iter().flatten() {
        visit(next, graph, path, finished, violations);
    }
    path.pop();
    finished.insert(node);
}

/// `Cargo.lock` holds only workspace members and approved third-party crates.
#[must_use]
pub fn check_lock(ws: &Workspace, policy: &Policy) -> Vec<Violation> {
    lock_packages(&ws.lock)
        .into_iter()
        .filter(|p| !ws.is_member(p) && !policy.external.contains(&p.as_str()))
        .map(|p| Violation::new(&p, "third-party", "third-party crates need an explicit allow-list entry"))
        .collect()
}
