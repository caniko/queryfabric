use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use async_trait::async_trait;
use queryfabric_access::{GroupId, OwnershipSource};
use queryfabric_contract::{ResourceRef, Subject};
use uuid::Uuid;

use crate::model::{Account, Group};

#[derive(Default)]
struct State {
    accounts: HashMap<Uuid, Account>,
    groups: HashMap<GroupId, Group>,
    resource_owner: HashMap<ResourceRef, Uuid>,
    resource_groups: HashMap<ResourceRef, Vec<GroupId>>,
    agreements: HashSet<(Uuid, ResourceRef)>,
}

/// In-memory [`OwnershipSource`] over [`Account`]s and [`Group`]s.
///
/// Reference implementation for tests and the demonstrator host; production
/// hosts back the trait with their identity store.
#[derive(Default)]
pub struct InMemoryOwnership {
    state: RwLock<State>,
}

impl InMemoryOwnership {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Register an account.
    pub fn add_account(&self, account: Account) {
        self.write().accounts.insert(account.id, account);
    }

    /// Register a group, including its membership.
    pub fn add_group(&self, group: Group) {
        self.write().groups.insert(group.id, group);
    }

    /// Declare `owner` (an account id) as the owner of `resource`.
    pub fn set_owner(&self, resource: ResourceRef, owner: Uuid) {
        self.write().resource_owner.insert(resource, owner);
    }

    /// Authorize `group` for `resource`.
    pub fn authorize_group(&self, resource: ResourceRef, group: GroupId) {
        self.write()
            .resource_groups
            .entry(resource)
            .or_default()
            .push(group);
    }

    /// Record an accepted data-use agreement between an account and a
    /// resource.
    pub fn accept_agreement(&self, account: Uuid, resource: ResourceRef) {
        self.write().agreements.insert((account, resource));
    }
}

#[async_trait]
impl OwnershipSource for InMemoryOwnership {
    async fn owner(&self, resource: ResourceRef) -> Option<Subject> {
        let state = self.read();
        let owner = state.resource_owner.get(&resource)?;
        state.accounts.get(owner).map(Account::subject)
    }

    async fn groups_for(&self, subject: &Subject) -> Vec<GroupId> {
        self.read()
            .groups
            .values()
            .filter(|group| group.members.contains(&subject.id))
            .map(|group| group.id)
            .collect()
    }

    async fn resource_groups(&self, resource: ResourceRef) -> Vec<GroupId> {
        self.read()
            .resource_groups
            .get(&resource)
            .cloned()
            .unwrap_or_default()
    }

    async fn has_accepted_agreement(&self, subject: &Subject, resource: ResourceRef) -> bool {
        self.read().agreements.contains(&(subject.id, resource))
    }
}
