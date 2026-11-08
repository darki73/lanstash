//! How much of a plan the stash holds, read from the index without counting
//! as a use.

use std::collections::BTreeSet;

use lanstash_battlenet_domain::locate::held_bytes;
use lanstash_battlenet_domain::plan::{Coverage, Plan, PlanDetail};
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_store::index::Lookup;
use lanstash_store::store::Store;

/// Whole files count when stored complete; archive parts count by the bytes
/// of them the archive's stored pieces cover.
pub(super) fn detail(store: &Store, plan: &Plan) -> PlanDetail {
    let index = store.index();
    let mut files = Coverage::default();
    for (t, size) in &plan.files {
        match index.lookup(&object_key("blizzard", t)) {
            Lookup::Found(o) if o.total > 0 && o.covers(Span::new(0, o.total)) => {
                let size = if *size == 0 { o.total } else { *size };
                files.have += size;
                files.want += size;
            }
            _ => files.want += size,
        }
    }
    let mut archived = Coverage::default();
    let mut at = 0;
    while at < plan.parts.len() {
        let archive = plan.parts[at].0;
        let end = at + plan.parts[at..].iter().take_while(|p| p.0 == archive).count();
        let spans: Vec<Span> = plan.parts[at..end].iter().map(|p| p.1).collect();
        archived.want += spans.iter().map(|s| s.end - s.start).sum::<u64>();
        if let Lookup::Found(o) = index.lookup(&object_key("blizzard", &plan.archives[archive])) {
            let stored: Vec<Span> = o.pieces.iter().map(|p| p.0).collect();
            archived.have += held_bytes(&spans, &stored);
        }
        at = end;
    }
    let archives = plan.parts.iter().map(|p| p.0).collect::<BTreeSet<_>>().len();
    PlanDetail {
        files,
        archived,
        file_count: plan.files.len(),
        part_count: plan.parts.len(),
        archive_count: archives,
        requests: plan.files.len() + plan.requests.len(),
        absent: plan.absent,
    }
}
