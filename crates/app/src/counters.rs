//! Counter groups: a struct of `AtomicU64` totals plus `values()` listing
//! them by name, so reports and the status page need no field lists of their own.

/// Defines a counter struct (`Debug`, `Default`) and `values()` on it.
macro_rules! counters {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident: AtomicU64,)* }) => {
        $(#[$m])*
        #[derive(Debug, Default)]
        pub struct $name {
            $($(#[$fm])* pub $f: std::sync::atomic::AtomicU64,)*
        }

        impl $name {
            /// Every counter with its field name, in declaration order.
            #[must_use]
            pub fn values(&self) -> Vec<(&'static str, u64)> {
                vec![$((stringify!($f), self.$f.load(std::sync::atomic::Ordering::Relaxed)),)*]
            }
        }
    };
}
