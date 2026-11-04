//! Opaque, expiring pagination markers scoped to an operator-selected relation.
//! Only a server-generated token is accepted by the browser.
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash, Hasher, RandomState},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use crate::db::types::{DbError, DbErrorKind, RelationId, RowOptions, SortSpec};
use super::row_query::SeekDirection;
const TOKEN_LIFETIME: Duration = Duration::from_secs(300);
const MAX_TOKENS: usize = 1024;
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    relation: RelationId,
    columns: Vec<String>,
    sort: Option<SortSpec>,
    limit: u32,
}
impl Scope {
    pub fn new(relation: &RelationId, options: &RowOptions) -> Self {
        Self { relation: relation.clone(), columns: options.selected_columns.clone(),
            sort: options.sort.clone(), limit: options.limit }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub value: i64,
    pub direction: SeekDirection,
    pub page: u32,
}
struct Entry { scope: Scope, anchor: Anchor, expires: Instant }
#[derive(Default)]
pub struct CursorStore { entries: HashMap<String, Entry> }
fn invalid() -> DbError { DbError::new(DbErrorKind::InvalidIdentifier) }
impl CursorStore {
    pub fn issue(&mut self, scope: Scope, anchor: Anchor) -> String {
        let now=Instant::now();
        self.entries.retain(|_,e|e.expires>now);
        if self.entries.len()>=MAX_TOKENS {self.entries.clear();}
        let index=NEXT_TOKEN.fetch_add(1,Ordering::Relaxed);
        let entropy=RandomState::new();
        let mut hasher=entropy.build_hasher();
        index.hash(&mut hasher);
        scope.relation.hash(&mut hasher);
        let token=format!("{index:016x}{:016x}",hasher.finish());
        self.entries.insert(token.clone(),Entry {
            scope,anchor,expires:now+TOKEN_LIFETIME,
        });
        token
    }
    pub fn resolve(&self,token:&str,scope:&Scope) -> Result<Anchor,DbError> {
        if token.len()!=32 || !token.bytes().all(|b|b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let Some(entry)=self.entries.get(token) else {return Err(invalid());};
        if entry.expires<=Instant::now() || &entry.scope!=scope {
            return Err(invalid());
        }
        Ok(entry.anchor)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_validation_checks_scope() {
        let id=RelationId {namespace:"public".into(),name:"t".into()};
        let options=RowOptions::default();
        let scope=Scope::new(&id,&options);
        let mut store=CursorStore::default();
        let token=store.issue(scope.clone(),Anchor {
            value:8,direction:SeekDirection::Forward,page:1,
        });
        assert_eq!(store.resolve(&token,&scope).unwrap().value,8);
        let other=Scope::new(&RelationId {
            namespace:"public".into(),name:"other".into(),
        },&options);
        assert!(store.resolve(&token,&other).is_err());
    }
}
