//! [`GameNames`] sources: a table (which the composition root starts with the
//! platforms' grouping rules) plus an optional names file (`service/game =
//! Name` lines), and a chain that asks several sources in order (e.g. a
//! platform login first, then the file, then Battle.net's catalog).

use std::path::Path;
use std::sync::Arc;

use lanstash_domain::names::NameTable;
use lanstash_ports::names::GameNames;

/// Names from a table, overridden by a names file.
#[derive(Debug, Clone, Default)]
pub struct TableNames {
    table: NameTable,
}

impl TableNames {
    /// The names of `table` only.
    #[must_use]
    pub fn new(table: NameTable) -> Self {
        Self { table }
    }

    /// The names of `table` plus the lines of `text` (named `source` in errors).
    ///
    /// # Errors
    /// A malformed line.
    pub fn with_text(mut table: NameTable, source: &str, text: &str) -> Result<Self, String> {
        table.add_text(text).map_err(|(line, e)| format!("{source}:{line}: {e}"))?;
        Ok(Self { table })
    }

    /// The names of `table` plus the file at `path`.
    ///
    /// # Errors
    /// The file can't be read or has a malformed line.
    pub fn open(table: NameTable, path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::with_text(table, &path.display().to_string(), &text)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

impl GameNames for TableNames {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        self.table.get(service, game).map(str::to_owned)
    }

    fn group(&self, service: &str, game: &str) -> Option<String> {
        self.table.group(service, game).map(str::to_owned)
    }
}

/// Asks each source in order; the first name wins.
#[derive(Default)]
pub struct ChainNames(pub Vec<Arc<dyn GameNames>>);

impl std::fmt::Debug for ChainNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChainNames({} sources)", self.0.len())
    }
}

impl GameNames for ChainNames {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        self.0.iter().find_map(|s| s.name(service, game))
    }

    fn group(&self, service: &str, game: &str) -> Option<String> {
        self.0.iter().find_map(|s| s.group(service, game))
    }
}
