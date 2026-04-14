//! Access control system with RBAC and ABAC
//!
//! This module provides:
//! - Role-Based Access Control (RBAC)
//! - Attribute-Based Access Control (ABAC)
//! - Permission management
//! - Access decision making

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Permission represents a specific action that can be performed
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// Resource type (e.g., "token", "order", "user")
    pub resource: String,
    /// Action (e.g., "create", "read", "update", "delete")
    pub action: String,
}

impl Permission {
    /// Create a new permission
    pub fn new(resource: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            action: action.into(),
        }
    }

    /// Parse permission from string format "resource:action"
    pub fn from_string(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(CoreError::Validation(
                "Permission must be in format 'resource:action'".to_string(),
            ));
        }

        Ok(Self {
            resource: parts[0].to_string(),
            action: parts[1].to_string(),
        })
    }

    /// Convert permission to string format
    pub fn to_string_format(&self) -> String {
        format!("{}:{}", self.resource, self.action)
    }

    /// Check if this permission matches a pattern (supports wildcards)
    pub fn matches(&self, pattern: &Permission) -> bool {
        let resource_match = pattern.resource == "*" || pattern.resource == self.resource;
        let action_match = pattern.action == "*" || pattern.action == self.action;

        resource_match && action_match
    }
}

/// Role in RBAC system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role ID
    pub id: Uuid,
    /// Role name
    pub name: String,
    /// Role description
    pub description: Option<String>,
    /// Permissions associated with this role
    pub permissions: HashSet<Permission>,
    /// Parent roles (for role inheritance)
    pub parent_roles: Vec<Uuid>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

impl Role {
    /// Create a new role
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            permissions: HashSet::new(),
            parent_roles: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a permission to the role
    pub fn add_permission(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    /// Remove a permission from the role
    pub fn remove_permission(&mut self, permission: &Permission) {
        self.permissions.remove(permission);
    }

    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions
            .iter()
            .any(|p| p == permission || permission.matches(p))
    }

    /// Add parent role for inheritance
    pub fn add_parent(&mut self, parent_id: Uuid) {
        if !self.parent_roles.contains(&parent_id) {
            self.parent_roles.push(parent_id);
        }
    }
}

/// User with assigned roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoles {
    /// User ID
    pub user_id: Uuid,
    /// Assigned role IDs
    pub role_ids: Vec<Uuid>,
    /// Direct permissions (outside of roles)
    pub direct_permissions: HashSet<Permission>,
}

impl UserRoles {
    /// Create new user roles
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            role_ids: Vec::new(),
            direct_permissions: HashSet::new(),
        }
    }

    /// Assign a role to the user
    pub fn assign_role(&mut self, role_id: Uuid) {
        if !self.role_ids.contains(&role_id) {
            self.role_ids.push(role_id);
        }
    }

    /// Revoke a role from the user
    pub fn revoke_role(&mut self, role_id: &Uuid) {
        self.role_ids.retain(|id| id != role_id);
    }

    /// Add direct permission
    pub fn add_permission(&mut self, permission: Permission) {
        self.direct_permissions.insert(permission);
    }
}

/// Attribute for ABAC
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Attribute {
    /// Attribute key
    pub key: String,
    /// Attribute value
    pub value: String,
}

impl Attribute {
    /// Create a new attribute
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// ABAC policy condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    /// Attribute equals value
    AttributeEquals {
        /// Attribute name to check.
        key: String,
        /// Expected attribute value.
        value: String,
    },
    /// Attribute contains value
    AttributeContains {
        /// Attribute name to check.
        key: String,
        /// Substring or element the attribute must contain.
        value: String,
    },
    /// Attribute greater than value (numeric)
    AttributeGreaterThan {
        /// Attribute name to check.
        key: String,
        /// Threshold value the attribute must exceed.
        value: String,
    },
    /// Time-based condition
    TimeWindow {
        /// Hour-of-day (0-23) when the window begins.
        start_hour: u8,
        /// Hour-of-day (0-23) when the window ends.
        end_hour: u8,
    },
    /// Combination of conditions (AND)
    And(Vec<PolicyCondition>),
    /// Combination of conditions (OR)
    Or(Vec<PolicyCondition>),
    /// Negation
    Not(Box<PolicyCondition>),
}

impl PolicyCondition {
    /// Evaluate condition against attributes
    pub fn evaluate(&self, attributes: &HashMap<String, String>) -> bool {
        match self {
            PolicyCondition::AttributeEquals { key, value } => {
                attributes.get(key).map(|v| v == value).unwrap_or(false)
            }
            PolicyCondition::AttributeContains { key, value } => attributes
                .get(key)
                .map(|v| v.contains(value))
                .unwrap_or(false),
            PolicyCondition::AttributeGreaterThan { key, value } => {
                if let Some(attr_value) = attributes.get(key) {
                    if let (Ok(av), Ok(cv)) = (attr_value.parse::<f64>(), value.parse::<f64>()) {
                        return av > cv;
                    }
                }
                false
            }
            PolicyCondition::TimeWindow {
                start_hour,
                end_hour,
            } => {
                let now = Utc::now().time();
                let current_hour = now.hour() as u8;
                current_hour >= *start_hour && current_hour < *end_hour
            }
            PolicyCondition::And(conditions) => conditions.iter().all(|c| c.evaluate(attributes)),
            PolicyCondition::Or(conditions) => conditions.iter().any(|c| c.evaluate(attributes)),
            PolicyCondition::Not(condition) => !condition.evaluate(attributes),
        }
    }
}

/// ABAC policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbacPolicy {
    /// Policy ID
    pub id: Uuid,
    /// Policy name
    pub name: String,
    /// Resource this policy applies to
    pub resource: String,
    /// Action this policy allows
    pub action: String,
    /// Conditions that must be met
    pub conditions: Vec<PolicyCondition>,
    /// Policy priority (higher = evaluated first)
    pub priority: i32,
}

impl AbacPolicy {
    /// Create a new ABAC policy
    pub fn new(
        name: impl Into<String>,
        resource: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            resource: resource.into(),
            action: action.into(),
            conditions: Vec::new(),
            priority: 0,
        }
    }

    /// Add a condition to the policy
    pub fn add_condition(&mut self, condition: PolicyCondition) {
        self.conditions.push(condition);
    }

    /// Evaluate if policy allows access
    pub fn allows(&self, permission: &Permission, attributes: &HashMap<String, String>) -> bool {
        // Check if permission matches
        if self.resource != permission.resource || self.action != permission.action {
            return false;
        }

        // Evaluate all conditions (AND logic)
        self.conditions.iter().all(|c| c.evaluate(attributes))
    }
}

/// Access control manager (combines RBAC and ABAC)
pub struct AccessControlManager {
    /// All roles in the system
    roles: HashMap<Uuid, Role>,
    /// User role assignments
    user_roles: HashMap<Uuid, UserRoles>,
    /// ABAC policies
    policies: Vec<AbacPolicy>,
}

impl AccessControlManager {
    /// Create a new access control manager
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
            user_roles: HashMap::new(),
            policies: Vec::new(),
        }
    }

    /// Add a role to the system
    pub fn add_role(&mut self, role: Role) {
        self.roles.insert(role.id, role);
    }

    /// Get a role by ID
    pub fn get_role(&self, role_id: &Uuid) -> Option<&Role> {
        self.roles.get(role_id)
    }

    /// Assign a role to a user
    pub fn assign_role_to_user(&mut self, user_id: Uuid, role_id: Uuid) -> Result<()> {
        // Check if role exists
        if !self.roles.contains_key(&role_id) {
            return Err(CoreError::NotFound(format!("Role {} not found", role_id)));
        }

        let user_roles = self
            .user_roles
            .entry(user_id)
            .or_insert_with(|| UserRoles::new(user_id));
        user_roles.assign_role(role_id);

        Ok(())
    }

    /// Revoke a role from a user
    pub fn revoke_role_from_user(&mut self, user_id: Uuid, role_id: Uuid) {
        if let Some(user_roles) = self.user_roles.get_mut(&user_id) {
            user_roles.revoke_role(&role_id);
        }
    }

    /// Add a direct permission to a user
    pub fn add_permission_to_user(&mut self, user_id: Uuid, permission: Permission) {
        let user_roles = self
            .user_roles
            .entry(user_id)
            .or_insert_with(|| UserRoles::new(user_id));
        user_roles.add_permission(permission);
    }

    /// Get all permissions for a user (including inherited from roles)
    pub fn get_user_permissions(&self, user_id: &Uuid) -> HashSet<Permission> {
        let mut permissions = HashSet::new();

        if let Some(user_roles) = self.user_roles.get(user_id) {
            // Add direct permissions
            permissions.extend(user_roles.direct_permissions.clone());

            // Add permissions from roles (with inheritance)
            for role_id in &user_roles.role_ids {
                self.add_role_permissions(&mut permissions, role_id, &mut HashSet::new());
            }
        }

        permissions
    }

    /// Recursively add role permissions (handles inheritance)
    fn add_role_permissions(
        &self,
        permissions: &mut HashSet<Permission>,
        role_id: &Uuid,
        visited: &mut HashSet<Uuid>,
    ) {
        // Prevent infinite loops
        if visited.contains(role_id) {
            return;
        }
        visited.insert(*role_id);

        if let Some(role) = self.roles.get(role_id) {
            permissions.extend(role.permissions.clone());

            // Add parent role permissions
            for parent_id in &role.parent_roles {
                self.add_role_permissions(permissions, parent_id, visited);
            }
        }
    }

    /// Add an ABAC policy
    pub fn add_policy(&mut self, policy: AbacPolicy) {
        self.policies.push(policy);
        // Sort by priority (higher first)
        self.policies.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Check if user has permission (RBAC)
    pub fn has_permission(&self, user_id: &Uuid, permission: &Permission) -> bool {
        let user_permissions = self.get_user_permissions(user_id);
        user_permissions
            .iter()
            .any(|p| p == permission || permission.matches(p))
    }

    /// Check if access is allowed (RBAC + ABAC)
    pub fn is_access_allowed(
        &self,
        user_id: &Uuid,
        permission: &Permission,
        attributes: &HashMap<String, String>,
    ) -> bool {
        // First, check RBAC
        if self.has_permission(user_id, permission) {
            return true;
        }

        // Then, check ABAC policies
        for policy in &self.policies {
            if policy.allows(permission, attributes) {
                return true;
            }
        }

        false
    }

    /// Get all roles
    pub fn get_all_roles(&self) -> Vec<&Role> {
        self.roles.values().collect()
    }

    /// Get user's roles
    pub fn get_user_roles(&self, user_id: &Uuid) -> Vec<&Role> {
        if let Some(user_roles) = self.user_roles.get(user_id) {
            user_roles
                .role_ids
                .iter()
                .filter_map(|role_id| self.roles.get(role_id))
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for AccessControlManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_creation() {
        let perm = Permission::new("token", "create");
        assert_eq!(perm.resource, "token");
        assert_eq!(perm.action, "create");
    }

    #[test]
    fn test_permission_from_string() {
        let perm = Permission::from_string("order:read").unwrap();
        assert_eq!(perm.resource, "order");
        assert_eq!(perm.action, "read");
    }

    #[test]
    fn test_permission_matches_wildcard() {
        let perm = Permission::new("token", "create");
        let wildcard = Permission::new("*", "create");

        assert!(perm.matches(&wildcard));
    }

    #[test]
    fn test_role_permissions() {
        let mut role = Role::new("admin");
        role.add_permission(Permission::new("token", "create"));
        role.add_permission(Permission::new("order", "read"));

        assert_eq!(role.permissions.len(), 2);
        assert!(role.has_permission(&Permission::new("token", "create")));
    }

    #[test]
    fn test_user_role_assignment() {
        let mut user_roles = UserRoles::new(Uuid::new_v4());
        let role_id = Uuid::new_v4();

        user_roles.assign_role(role_id);
        assert_eq!(user_roles.role_ids.len(), 1);

        user_roles.revoke_role(&role_id);
        assert_eq!(user_roles.role_ids.len(), 0);
    }

    #[test]
    fn test_attribute_condition() {
        let condition = PolicyCondition::AttributeEquals {
            key: "department".to_string(),
            value: "engineering".to_string(),
        };

        let mut attributes = HashMap::new();
        attributes.insert("department".to_string(), "engineering".to_string());

        assert!(condition.evaluate(&attributes));

        attributes.insert("department".to_string(), "sales".to_string());
        assert!(!condition.evaluate(&attributes));
    }

    #[test]
    fn test_time_window_condition() {
        let condition = PolicyCondition::TimeWindow {
            start_hour: 0,
            end_hour: 24,
        };

        let attributes = HashMap::new();
        assert!(condition.evaluate(&attributes));
    }

    #[test]
    fn test_and_condition() {
        let condition = PolicyCondition::And(vec![
            PolicyCondition::AttributeEquals {
                key: "role".to_string(),
                value: "admin".to_string(),
            },
            PolicyCondition::AttributeEquals {
                key: "department".to_string(),
                value: "it".to_string(),
            },
        ]);

        let mut attributes = HashMap::new();
        attributes.insert("role".to_string(), "admin".to_string());
        attributes.insert("department".to_string(), "it".to_string());

        assert!(condition.evaluate(&attributes));
    }

    #[test]
    fn test_abac_policy() {
        let mut policy = AbacPolicy::new("engineering_only", "token", "create");
        policy.add_condition(PolicyCondition::AttributeEquals {
            key: "department".to_string(),
            value: "engineering".to_string(),
        });

        let mut attributes = HashMap::new();
        attributes.insert("department".to_string(), "engineering".to_string());

        assert!(policy.allows(&Permission::new("token", "create"), &attributes));

        attributes.insert("department".to_string(), "sales".to_string());
        assert!(!policy.allows(&Permission::new("token", "create"), &attributes));
    }

    #[test]
    fn test_access_control_manager_rbac() {
        let mut manager = AccessControlManager::new();

        let mut role = Role::new("trader");
        role.add_permission(Permission::new("order", "create"));
        let role_id = role.id;

        manager.add_role(role);

        let user_id = Uuid::new_v4();
        manager.assign_role_to_user(user_id, role_id).unwrap();

        assert!(manager.has_permission(&user_id, &Permission::new("order", "create")));
        assert!(!manager.has_permission(&user_id, &Permission::new("order", "delete")));
    }

    #[test]
    fn test_access_control_manager_abac() {
        let mut manager = AccessControlManager::new();

        let mut policy = AbacPolicy::new("vip_only", "token", "mint");
        policy.add_condition(PolicyCondition::AttributeEquals {
            key: "tier".to_string(),
            value: "vip".to_string(),
        });

        manager.add_policy(policy);

        let user_id = Uuid::new_v4();
        let mut attributes = HashMap::new();
        attributes.insert("tier".to_string(), "vip".to_string());

        assert!(manager.is_access_allowed(
            &user_id,
            &Permission::new("token", "mint"),
            &attributes
        ));

        attributes.insert("tier".to_string(), "basic".to_string());
        assert!(!manager.is_access_allowed(
            &user_id,
            &Permission::new("token", "mint"),
            &attributes
        ));
    }

    #[test]
    fn test_role_inheritance() {
        let mut manager = AccessControlManager::new();

        // Create parent role
        let mut parent_role = Role::new("user");
        parent_role.add_permission(Permission::new("order", "read"));
        let parent_id = parent_role.id;
        manager.add_role(parent_role);

        // Create child role with inheritance
        let mut child_role = Role::new("admin");
        child_role.add_permission(Permission::new("order", "create"));
        child_role.add_parent(parent_id);
        let child_id = child_role.id;
        manager.add_role(child_role);

        let user_id = Uuid::new_v4();
        manager.assign_role_to_user(user_id, child_id).unwrap();

        // Should have both permissions
        assert!(manager.has_permission(&user_id, &Permission::new("order", "read")));
        assert!(manager.has_permission(&user_id, &Permission::new("order", "create")));
    }
}
