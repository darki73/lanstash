//! How Steam depots are named.

/// Steam depots are numbered from their app's id upwards (Deadlock: app
/// 1422450, depots 1422450 to 1422456). A depot without a name of its own
/// takes the name of the nearest app id at most this far below it. A
/// heuristic: exact only with Steam's own app info.
pub const DEPOT_WINDOW: u32 = 100;
