//! Versioned package migrations.

use crate::package::Package;
use qf_core::{MigrationError, Version};
use std::sync::Arc;

/// Versioned package migration edge.
///
/// Method names `from_version` / `to_version` are part of the binding product
/// specification (docs/07-migration.md).
#[allow(clippy::wrong_self_convention)]
pub trait Migration: Send + Sync {
    fn from_version(&self) -> Version;
    fn to_version(&self) -> Version;
    fn migrate(&self, input: Package) -> Result<Package, MigrationError>;
}

pub struct MigrationRegistry {
    edges: Vec<Arc<dyn Migration>>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self { edges: vec![] }
    }

    pub fn register(&mut self, m: Arc<dyn Migration>) {
        self.edges.push(m);
    }

    pub fn plan(
        &self,
        from: Version,
        to: Version,
    ) -> Result<Vec<Arc<dyn Migration>>, MigrationError> {
        if from == to {
            return Ok(vec![]);
        }
        let mut path = Vec::new();
        let mut cur = from;
        while cur != to {
            let edge = self
                .edges
                .iter()
                .find(|e| e.from_version() == cur)
                .cloned()
                .ok_or_else(|| MigrationError::NoPath {
                    from: from.to_string(),
                    to: to.to_string(),
                })?;
            let next = edge.to_version();
            path.push(edge);
            cur = next;
            if path.len() > 64 {
                return Err(MigrationError::Failed("migration path too long".into()));
            }
        }
        Ok(path)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Migrator {
    pub registry: MigrationRegistry,
    pub target: Version,
}

impl Migrator {
    pub fn new(target: Version) -> Self {
        Self {
            registry: MigrationRegistry::new(),
            target,
        }
    }

    pub fn needs_migration(&self, from: Version) -> bool {
        from != self.target
    }

    pub fn migrate_package(&self, mut input: Package) -> Result<Package, MigrationError> {
        let plan = self.registry.plan(input.format_version, self.target)?;
        for m in plan {
            let unknown_before = count_unknown_payloads(&input);
            let other_before = input.other_files.len();
            input = m.migrate(input)?;
            if input.format_version != m.to_version() {
                return Err(MigrationError::Validation(
                    "migration did not set format_version to to_version".into(),
                ));
            }
            let unknown_after = count_unknown_payloads(&input);
            if unknown_after < unknown_before {
                return Err(MigrationError::Validation(
                    "migration dropped unknown block payloads".into(),
                ));
            }
            if input.other_files.len() < other_before {
                return Err(MigrationError::Validation(
                    "migration dropped other_files members".into(),
                ));
            }
        }
        Ok(input)
    }
}

fn count_unknown_payloads(pkg: &Package) -> usize {
    let mut n = 0usize;
    if let Some(cp) = &pkg.checkpoint {
        for block in cp.snapshot.state.document.blocks.values() {
            if matches!(block.body, qf_blocks::BlockBody::Unknown(_)) {
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_version_requires_no_identity_edge() {
        let registry = MigrationRegistry::new();
        assert!(registry
            .plan(Version::V1_0_0, Version::V1_0_0)
            .unwrap()
            .is_empty());
    }
}
