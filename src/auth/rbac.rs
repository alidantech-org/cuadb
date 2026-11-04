//! Roles, actions and resource grants. Authorization is deny-by-default and is
//! evaluated on the server for every route; grants never carry database credentials.

use std::sync::Arc;

use super::AuthError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    View,
    Export,
    Edit,
    Sql,
}

impl Action {
    pub const fn bit(self) -> u8 {
        match self {
            Self::View => 1,
            Self::Export => 2,
            Self::Edit => 4,
            Self::Sql => 8,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Export => "export",
            Self::Edit => "edit",
            Self::Sql => "sql",
        }
    }
}

/// Parse `view,export` style lists into a bitmask; unknown names are rejected.
pub fn parse_actions(raw: &str) -> Result<u8, AuthError> {
    let mut mask = 0u8;
    for part in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        mask |= match part {
            "view" => Action::View.bit(),
            "export" => Action::Export.bit(),
            "edit" => Action::Edit.bit(),
            "sql" => Action::Sql.bit(),
            _ => return Err(AuthError::Invalid("unknown action")),
        };
    }
    if mask == 0 {
        return Err(AuthError::Invalid("at least one action is required"));
    }
    Ok(mask)
}

pub fn action_names(mask: u8) -> String {
    [Action::View, Action::Export, Action::Edit, Action::Sql]
        .into_iter()
        .filter(|a| mask & a.bit() != 0)
        .map(Action::name)
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl Role {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, AuthError> {
        match raw {
            "admin" => Ok(Self::Admin),
            "editor" => Ok(Self::Editor),
            "viewer" => Ok(Self::Viewer),
            _ => Err(AuthError::Invalid("role must be admin, editor or viewer")),
        }
    }

    /// The most a role can ever do, whatever grants say.
    pub const fn ceiling(self) -> u8 {
        match self {
            Self::Admin => 15,
            Self::Editor => 15,
            Self::Viewer => 3,
        }
    }
}

/// What an operation touches. `schema`/`relation` are `None` for profile-level checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resource<'a> {
    pub profile: &'a str,
    pub schema: Option<&'a str>,
    pub relation: Option<&'a str>,
}

impl<'a> Resource<'a> {
    pub fn profile(profile: &'a str) -> Self {
        Self {
            profile,
            schema: None,
            relation: None,
        }
    }

    pub fn schema(profile: &'a str, schema: &'a str) -> Self {
        Self {
            profile,
            schema: Some(schema),
            relation: None,
        }
    }

    pub fn relation(profile: &'a str, schema: &'a str, relation: &'a str) -> Self {
        Self {
            profile,
            schema: Some(schema),
            relation: Some(relation),
        }
    }

    pub fn describe(&self) -> String {
        match (self.schema, self.relation) {
            (Some(s), Some(r)) => format!("{}/{s}.{r}", self.profile),
            (Some(s), None) => format!("{}/{s}", self.profile),
            _ => self.profile.to_string(),
        }
    }
}

/// A stored grant. Empty `schema`/`relation` mean "any".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub id: i64,
    pub profile: String,
    pub schema: String,
    pub relation: String,
    pub actions: u8,
}

#[derive(Debug)]
pub struct PrincipalData {
    pub user_id: Option<i64>,
    pub login: String,
    pub role: Role,
    /// CSRF secret of the session; empty for the open (unauthenticated local) principal.
    pub csrf: String,
    pub session_hash: Vec<u8>,
    pub grants: Vec<Grant>,
    /// Auth is not configured: the local shell is open and read-only.
    pub open: bool,
}

#[derive(Debug, Clone)]
pub struct Principal(pub Arc<PrincipalData>);

impl Principal {
    /// The unauthenticated local principal: may view and export, never edit or run SQL.
    pub fn open() -> Self {
        Self(Arc::new(PrincipalData {
            user_id: None,
            login: "local".into(),
            role: Role::Viewer,
            csrf: String::new(),
            session_hash: Vec::new(),
            grants: Vec::new(),
            open: true,
        }))
    }

    pub fn is_open(&self) -> bool {
        self.0.open
    }

    pub fn is_admin(&self) -> bool {
        !self.0.open && self.0.role == Role::Admin
    }

    pub fn login(&self) -> &str {
        &self.0.login
    }

    pub fn user_id(&self) -> Option<i64> {
        self.0.user_id
    }

    pub fn csrf(&self) -> &str {
        &self.0.csrf
    }

    fn matches(grant: &Grant, resource: &Resource<'_>) -> bool {
        grant.profile == resource.profile
            && (grant.schema.is_empty() || Some(grant.schema.as_str()) == resource.schema)
            && (grant.relation.is_empty() || Some(grant.relation.as_str()) == resource.relation)
    }

    /// Deny by default. Admin may do anything; others need a matching grant, capped by role.
    pub fn allows(&self, action: Action, resource: &Resource<'_>) -> bool {
        let data = &self.0;
        if data.open {
            return matches!(action, Action::View | Action::Export);
        }
        if data.role == Role::Admin {
            return true;
        }
        if data.role.ceiling() & action.bit() == 0 {
            return false;
        }
        data.grants.iter().any(|grant| {
            grant.actions & action.bit() != 0
                && if resource.schema.is_none() {
                    // A profile-level question is true if any grant for the profile allows it.
                    grant.profile == resource.profile
                } else {
                    Self::matches(grant, resource)
                }
        })
    }

    /// Like `allows`, but only for grants that cover the whole profile (`schema` None) or the
    /// whole schema, never a single relation. Used for metadata that spans relations.
    pub fn allows_wide(&self, action: Action, profile: &str, schema: Option<&str>) -> bool {
        let data = &self.0;
        if data.open {
            return matches!(action, Action::View | Action::Export);
        }
        if data.role == Role::Admin {
            return true;
        }
        if data.role.ceiling() & action.bit() == 0 {
            return false;
        }
        data.grants.iter().any(|g| {
            g.profile == profile
                && g.actions & action.bit() != 0
                && g.relation.is_empty()
                && match schema {
                    None => g.schema.is_empty(),
                    Some(s) => g.schema.is_empty() || g.schema == s,
                }
        })
    }

    /// May the user see that this schema exists (any grant that can reach into it)?
    pub fn can_see_schema(&self, profile: &str, schema: &str) -> bool {
        let data = &self.0;
        if data.open || data.role == Role::Admin {
            return true;
        }
        data.grants.iter().any(|g| {
            g.profile == profile
                && g.actions & Action::View.bit() != 0
                && (g.schema.is_empty() || g.schema == schema)
        })
    }

    /// Profiles this user may open at all.
    pub fn can_see_profile(&self, profile: &str) -> bool {
        self.allows(Action::View, &Resource::profile(profile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role, grants: Vec<Grant>) -> Principal {
        Principal(Arc::new(PrincipalData {
            user_id: Some(1),
            login: "u".into(),
            role,
            csrf: "c".into(),
            session_hash: vec![],
            grants,
            open: false,
        }))
    }

    fn grant(profile: &str, schema: &str, relation: &str, actions: u8) -> Grant {
        Grant {
            id: 1,
            profile: profile.into(),
            schema: schema.into(),
            relation: relation.into(),
            actions,
        }
    }

    #[test]
    fn no_grants_means_no_access() {
        let p = user(Role::Viewer, vec![]);
        for action in [Action::View, Action::Export, Action::Edit, Action::Sql] {
            assert!(!p.allows(action, &Resource::relation("a", "s", "t")));
            assert!(!p.allows(action, &Resource::profile("a")));
        }
        assert!(!p.can_see_profile("a") && !p.can_see_schema("a", "s"));
    }

    #[test]
    fn grants_scope_to_profile_schema_and_relation() {
        let p = user(
            Role::Editor,
            vec![
                grant("a", "s", "t", 3),
                grant("a", "other", "", 1),
                grant("b", "", "", 15),
            ],
        );
        assert!(p.allows(Action::View, &Resource::relation("a", "s", "t")));
        assert!(
            !p.allows(Action::View, &Resource::relation("a", "s", "u")),
            "other relation"
        );
        assert!(
            !p.allows(Action::Edit, &Resource::relation("a", "s", "t")),
            "action not granted"
        );
        assert!(p.allows(Action::View, &Resource::relation("a", "other", "anything")));
        assert!(!p.allows(Action::Export, &Resource::relation("a", "other", "x")));
        assert!(
            p.allows(Action::Sql, &Resource::relation("b", "x", "y")),
            "whole-profile grant"
        );
        assert!(
            !p.allows(Action::View, &Resource::relation("c", "s", "t")),
            "other profile"
        );
        assert!(
            p.can_see_schema("a", "s"),
            "a relation grant makes its schema navigable"
        );
        assert!(p.can_see_schema("a", "other") && !p.can_see_schema("a", "hidden"));
        assert!(p.can_see_profile("a") && p.can_see_profile("b") && !p.can_see_profile("c"));
    }

    #[test]
    fn role_ceiling_caps_grants_and_admin_is_unrestricted() {
        let viewer = user(Role::Viewer, vec![grant("a", "", "", 15)]);
        assert!(viewer.allows(Action::Export, &Resource::profile("a")));
        assert!(!viewer.allows(Action::Edit, &Resource::profile("a")));
        assert!(!viewer.allows(Action::Sql, &Resource::relation("a", "s", "t")));
        let admin = user(Role::Admin, vec![]);
        assert!(admin.allows(Action::Sql, &Resource::relation("zz", "s", "t")) && admin.is_admin());
    }

    #[test]
    fn the_open_principal_can_only_read() {
        let p = Principal::open();
        assert!(p.allows(Action::View, &Resource::relation("x", "s", "t")));
        assert!(p.allows(Action::Export, &Resource::profile("x")));
        assert!(!p.allows(Action::Edit, &Resource::profile("x")));
        assert!(!p.allows(Action::Sql, &Resource::profile("x")));
        assert!(!p.is_admin());
    }

    #[test]
    fn action_lists_parse_strictly() {
        assert_eq!(parse_actions("view, export").unwrap(), 3);
        assert!(parse_actions("").is_err() && parse_actions("view,drop").is_err());
        assert_eq!(action_names(15), "view,export,edit,sql");
        assert!(Role::parse("root").is_err());
    }
}
