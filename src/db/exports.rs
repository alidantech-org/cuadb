//! Export bounds shared by future CSV/JSON/linked-table streaming paths.
//! The current milestone defines policy only; it does not expose exports.
use super::types::{DbError, DbErrorKind, RelationId};

pub const MAX_EXPORT_ROWS: u64 = 10_000;
pub const MAX_EXPORT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_EXPORT_RELATIONS: usize = 8;
pub const MAX_EXPORT_DEPTH: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    RelatedZip,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    CurrentPage,
    BoundedRelation,
    RelatedRelations,
}

#[derive(Debug, Clone)]
pub struct ExportPlan {
    /// Identifiers require the same live catalog/SELECT validation as row pages.
    pub relations: Vec<RelationId>,
    pub format: ExportFormat,
    pub scope: ExportScope,
    pub max_rows: u64,
    pub max_bytes: u64,
    pub max_depth: u8,
}
impl ExportPlan {
    pub fn bounded(self) -> Result<Self, DbError> {
        let invalid = || DbError::new(DbErrorKind::InvalidIdentifier);
        if self.relations.is_empty()
            || self.relations.len() > MAX_EXPORT_RELATIONS
            || self.max_rows == 0
            || self.max_rows > MAX_EXPORT_ROWS
            || self.max_bytes == 0
            || self.max_bytes > MAX_EXPORT_BYTES
            || self.max_depth > MAX_EXPORT_DEPTH
        {
            return Err(invalid());
        }
        if self.scope != ExportScope::RelatedRelations
            && (self.relations.len() != 1
                || self.max_depth != 0
                || self.format == ExportFormat::RelatedZip)
        {
            return Err(invalid());
        }
        if self.scope == ExportScope::RelatedRelations && self.format != ExportFormat::RelatedZip {
            return Err(invalid());
        }
        Ok(self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_or_unscoped_export_requests() {
        let plan = ExportPlan {
            relations: vec![RelationId {
                namespace: "public".into(),
                name: "t".into(),
            }],
            format: ExportFormat::Csv,
            scope: ExportScope::CurrentPage,
            max_rows: 100,
            max_bytes: 1024,
            max_depth: 0,
        };
        assert!(plan.clone().bounded().is_ok());
        assert!(
            ExportPlan {
                max_rows: u64::MAX,
                ..plan.clone()
            }
            .bounded()
            .is_err()
        );
        assert!(
            ExportPlan {
                max_depth: 2,
                ..plan
            }
            .bounded()
            .is_err()
        );
    }
}
