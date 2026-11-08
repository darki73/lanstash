//! Runs every rule over a loaded workspace.

use crate::comments::check_em_dashes;
use crate::deps::{check_acyclic, check_internal_dependencies, check_layers, check_lock, check_policy_coverage};
use crate::file_size::check_file_sizes;
use crate::files::{check_crate_files, check_lints};
use crate::mirror::check_test_mirror;
use crate::module_imports::check_module_imports;
use crate::policy::Policy;
use crate::sources::check_sources;
use crate::violation::Violation;
use crate::workspace::Workspace;

/// Every violation of `policy` in `ws`.
#[must_use]
pub fn audit(ws: &Workspace, policy: &Policy) -> Vec<Violation> {
    let mut v = check_policy_coverage(ws, policy);
    v.extend(check_internal_dependencies(ws, policy));
    v.extend(check_layers(policy));
    v.extend(check_acyclic(ws));
    v.extend(check_lock(ws, policy));
    for c in &ws.crates {
        v.extend(check_crate_files(c, policy.undocumented));
        v.extend(check_lints(c, policy.allows_unsafe(c.name())));
        v.extend(check_file_sizes(c, policy.max_lines, policy.oversized));
        v.extend(check_module_imports(c.name(), &c.sources, policy.module_bans));
        let untested: Vec<&str> =
            policy.untested.iter().filter(|(p, _, _)| *p == c.name()).map(|(_, m, _)| *m).collect();
        v.extend(check_test_mirror(c, &untested));
        if let Some(rules) = policy.crate_policy(c.name()) {
            v.extend(check_sources(c, rules, policy));
        }
    }
    for (path, text) in &ws.texts {
        v.extend(check_em_dashes(path, text));
    }
    v
}
