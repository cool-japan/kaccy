//! Event sourcing infrastructure for state reconstruction and CQRS
//!
//! This module provides event store implementation, state reconstruction from events,
//! event replay capabilities, and snapshot optimization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::fmt::Debug;
use uuid::Uuid;

/// Trait for domain events that can be sourced
pub trait DomainEvent: Clone + Debug + Serialize + DeserializeOwned {
    /// Get the event type identifier
    fn event_type(&self) -> &'static str;

    /// Get the aggregate ID this event belongs to
    fn aggregate_id(&self) -> Uuid;

    /// Get the event timestamp
    fn timestamp(&self) -> DateTime<Utc>;
}

/// Stored event with metadata
#[derive(Debug, Clone, Serialize)]
#[serde(bound = "E: Serialize + DeserializeOwned")]
pub struct StoredEvent<E: DomainEvent> {
    /// Event sequence number
    pub sequence: u64,
    /// Aggregate ID
    pub aggregate_id: Uuid,
    /// Event type
    pub event_type: String,
    /// Event data
    pub event: E,
    /// Timestamp when stored
    pub stored_at: DateTime<Utc>,
    /// Metadata about the event
    pub metadata: EventMetadata,
}

/// Metadata about an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// User who caused the event
    pub user_id: Option<Uuid>,
    /// Correlation ID for tracking related events
    pub correlation_id: Option<Uuid>,
    /// Causation ID (the event that caused this event)
    pub causation_id: Option<Uuid>,
    /// Additional custom metadata
    pub custom: HashMap<String, String>,
}

impl EventMetadata {
    /// Create new metadata
    pub fn new() -> Self {
        Self {
            user_id: None,
            correlation_id: None,
            causation_id: None,
            custom: HashMap::new(),
        }
    }

    /// Set user ID
    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set correlation ID
    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Add custom metadata
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of aggregate state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct Snapshot<T> {
    /// Aggregate ID
    pub aggregate_id: Uuid,
    /// Sequence number when snapshot was taken
    pub sequence: u64,
    /// The snapshot data
    pub state: T,
    /// When the snapshot was created
    pub created_at: DateTime<Utc>,
}

/// Event store for persisting and retrieving domain events
pub struct DomainEventStore<E: DomainEvent> {
    /// All stored events, indexed by aggregate ID
    events: HashMap<Uuid, Vec<StoredEvent<E>>>,
    /// Global sequence counter
    next_sequence: u64,
    /// Snapshots indexed by aggregate ID
    snapshots: HashMap<Uuid, Vec<u8>>, // Serialized snapshots
}

impl<E: DomainEvent> DomainEventStore<E> {
    /// Create a new event store
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
            next_sequence: 1,
            snapshots: HashMap::new(),
        }
    }

    /// Append an event to the store
    pub fn append(&mut self, event: E, metadata: EventMetadata) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;

        let stored_event = StoredEvent {
            sequence,
            aggregate_id: event.aggregate_id(),
            event_type: event.event_type().to_string(),
            event,
            stored_at: Utc::now(),
            metadata,
        };

        self.events
            .entry(stored_event.aggregate_id)
            .or_default()
            .push(stored_event);

        sequence
    }

    /// Get all events for an aggregate
    pub fn get_events(&self, aggregate_id: Uuid) -> Vec<&StoredEvent<E>> {
        self.events
            .get(&aggregate_id)
            .map(|events| events.iter().collect())
            .unwrap_or_default()
    }

    /// Get events for an aggregate after a specific sequence
    pub fn get_events_after(
        &self,
        aggregate_id: Uuid,
        after_sequence: u64,
    ) -> Vec<&StoredEvent<E>> {
        self.events
            .get(&aggregate_id)
            .map(|events| {
                events
                    .iter()
                    .filter(|e| e.sequence > after_sequence)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get events in a sequence range
    pub fn get_events_range(
        &self,
        aggregate_id: Uuid,
        from_sequence: u64,
        to_sequence: u64,
    ) -> Vec<&StoredEvent<E>> {
        self.events
            .get(&aggregate_id)
            .map(|events| {
                events
                    .iter()
                    .filter(|e| e.sequence >= from_sequence && e.sequence <= to_sequence)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all events across all aggregates (for global event stream)
    pub fn get_all_events(&self) -> Vec<&StoredEvent<E>> {
        let mut all_events: Vec<&StoredEvent<E>> = self
            .events
            .values()
            .flat_map(|events| events.iter())
            .collect();

        // Sort by sequence number
        all_events.sort_by_key(|e| e.sequence);
        all_events
    }

    /// Save a snapshot
    pub fn save_snapshot<T: Serialize>(&mut self, snapshot: &Snapshot<T>) -> Result<(), String> {
        let serialized =
            serde_json::to_vec(snapshot).map_err(|e| format!("Serialization error: {}", e))?;

        self.snapshots.insert(snapshot.aggregate_id, serialized);
        Ok(())
    }

    /// Load a snapshot
    pub fn load_snapshot<T: DeserializeOwned>(
        &self,
        aggregate_id: Uuid,
    ) -> Result<Option<Snapshot<T>>, String> {
        if let Some(data) = self.snapshots.get(&aggregate_id) {
            let snapshot: Snapshot<T> = serde_json::from_slice(data)
                .map_err(|e| format!("Deserialization error: {}", e))?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    /// Get the current sequence for an aggregate
    pub fn get_current_sequence(&self, aggregate_id: Uuid) -> u64 {
        self.events
            .get(&aggregate_id)
            .and_then(|events| events.last().map(|e| e.sequence))
            .unwrap_or(0)
    }

    /// Get event count for an aggregate
    pub fn get_event_count(&self, aggregate_id: Uuid) -> usize {
        self.events
            .get(&aggregate_id)
            .map(|events| events.len())
            .unwrap_or(0)
    }
}

impl<E: DomainEvent> Default for DomainEventStore<E> {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for aggregates that can be reconstructed from events
pub trait AggregateRoot: Sized {
    /// The event type for this aggregate
    type Event: DomainEvent;

    /// Create a new empty aggregate
    fn new(id: Uuid) -> Self;

    /// Apply an event to the aggregate
    fn apply_event(&mut self, event: &Self::Event);

    /// Get the aggregate ID
    fn aggregate_id(&self) -> Uuid;
}

/// Event sourcing repository for loading and saving aggregates
pub struct EventSourcedRepository<A: AggregateRoot> {
    /// Event store
    store: DomainEventStore<A::Event>,
    /// Snapshot frequency (take snapshot every N events)
    snapshot_frequency: usize,
}

impl<A: AggregateRoot> EventSourcedRepository<A>
where
    A: Serialize + DeserializeOwned,
{
    /// Create a new repository
    pub fn new(snapshot_frequency: usize) -> Self {
        Self {
            store: DomainEventStore::new(),
            snapshot_frequency,
        }
    }

    /// Load an aggregate from events
    pub fn load(&self, aggregate_id: Uuid) -> Result<Option<A>, String> {
        // Try to load from snapshot first
        let (mut aggregate, start_sequence) =
            if let Some(snapshot) = self.store.load_snapshot::<A>(aggregate_id)? {
                (snapshot.state, snapshot.sequence)
            } else {
                (A::new(aggregate_id), 0)
            };

        // Apply events after the snapshot
        let events = self.store.get_events_after(aggregate_id, start_sequence);

        if events.is_empty() && start_sequence == 0 {
            return Ok(None); // No events found
        }

        for stored_event in events {
            aggregate.apply_event(&stored_event.event);
        }

        Ok(Some(aggregate))
    }

    /// Save events for an aggregate
    pub fn save(&mut self, events: Vec<A::Event>, metadata: EventMetadata) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }

        let aggregate_id = events[0].aggregate_id();

        // Append all events
        for event in events {
            self.store.append(event, metadata.clone());
        }

        // Check if we should create a snapshot
        let event_count = self.store.get_event_count(aggregate_id);
        if event_count >= self.snapshot_frequency && event_count % self.snapshot_frequency == 0 {
            if let Ok(Some(aggregate)) = self.load(aggregate_id) {
                let snapshot = Snapshot {
                    aggregate_id,
                    sequence: self.store.get_current_sequence(aggregate_id),
                    state: aggregate,
                    created_at: Utc::now(),
                };
                self.store.save_snapshot(&snapshot)?;
            }
        }

        Ok(())
    }

    /// Replay events and return all states
    pub fn replay(&self, aggregate_id: Uuid) -> Result<Vec<A>, String> {
        let mut states = Vec::new();
        let mut aggregate = A::new(aggregate_id);

        let events = self.store.get_events(aggregate_id);

        for stored_event in events {
            aggregate.apply_event(&stored_event.event);
            // Clone the aggregate at each step (this is expensive but useful for replay)
            states.push(
                serde_json::from_str::<A>(
                    &serde_json::to_string(&aggregate)
                        .expect("aggregate serialization to string is infallible"),
                )
                .expect("aggregate serialized just above so deserialization succeeds"),
            );
        }

        Ok(states)
    }

    /// Get the event store
    pub fn event_store(&self) -> &DomainEventStore<A::Event> {
        &self.store
    }

    /// Get mutable event store
    pub fn event_store_mut(&mut self) -> &mut DomainEventStore<A::Event> {
        &mut self.store
    }
}

impl<A: AggregateRoot> Default for EventSourcedRepository<A>
where
    A: Serialize + DeserializeOwned,
{
    fn default() -> Self {
        Self::new(100) // Default: snapshot every 100 events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test domain event
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum AccountEvent {
        Created { id: Uuid, name: String },
        Deposited { id: Uuid, amount: i64 },
        Withdrawn { id: Uuid, amount: i64 },
    }

    impl DomainEvent for AccountEvent {
        fn event_type(&self) -> &'static str {
            match self {
                AccountEvent::Created { .. } => "account.created",
                AccountEvent::Deposited { .. } => "account.deposited",
                AccountEvent::Withdrawn { .. } => "account.withdrawn",
            }
        }

        fn aggregate_id(&self) -> Uuid {
            match self {
                AccountEvent::Created { id, .. } => *id,
                AccountEvent::Deposited { id, .. } => *id,
                AccountEvent::Withdrawn { id, .. } => *id,
            }
        }

        fn timestamp(&self) -> DateTime<Utc> {
            Utc::now()
        }
    }

    // Test aggregate
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Account {
        id: Uuid,
        name: String,
        balance: i64,
    }

    impl AggregateRoot for Account {
        type Event = AccountEvent;

        fn new(id: Uuid) -> Self {
            Self {
                id,
                name: String::new(),
                balance: 0,
            }
        }

        fn apply_event(&mut self, event: &Self::Event) {
            match event {
                AccountEvent::Created { name, .. } => {
                    self.name = name.clone();
                }
                AccountEvent::Deposited { amount, .. } => {
                    self.balance += amount;
                }
                AccountEvent::Withdrawn { amount, .. } => {
                    self.balance -= amount;
                }
            }
        }

        fn aggregate_id(&self) -> Uuid {
            self.id
        }
    }

    #[test]
    fn test_event_store_append_and_retrieve() {
        let mut store = DomainEventStore::new();
        let account_id = Uuid::new_v4();

        let event = AccountEvent::Created {
            id: account_id,
            name: "Test Account".to_string(),
        };

        let sequence = store.append(event, EventMetadata::new());
        assert_eq!(sequence, 1);

        let events = store.get_events(account_id);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
    }

    #[test]
    fn test_event_store_multiple_events() {
        let mut store = DomainEventStore::new();
        let account_id = Uuid::new_v4();

        store.append(
            AccountEvent::Created {
                id: account_id,
                name: "Test".to_string(),
            },
            EventMetadata::new(),
        );
        store.append(
            AccountEvent::Deposited {
                id: account_id,
                amount: 100,
            },
            EventMetadata::new(),
        );
        store.append(
            AccountEvent::Withdrawn {
                id: account_id,
                amount: 50,
            },
            EventMetadata::new(),
        );

        let events = store.get_events(account_id);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_aggregate_reconstruction() {
        let mut repo = EventSourcedRepository::<Account>::new(10);
        let account_id = Uuid::new_v4();

        let events = vec![
            AccountEvent::Created {
                id: account_id,
                name: "Alice".to_string(),
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 100,
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 50,
            },
            AccountEvent::Withdrawn {
                id: account_id,
                amount: 30,
            },
        ];

        repo.save(events, EventMetadata::new()).unwrap();

        let account = repo.load(account_id).unwrap().unwrap();
        assert_eq!(account.name, "Alice");
        assert_eq!(account.balance, 120); // 100 + 50 - 30
    }

    #[test]
    fn test_event_metadata() {
        let user_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();

        let metadata = EventMetadata::new()
            .with_user(user_id)
            .with_correlation(correlation_id)
            .with_custom("source".to_string(), "api".to_string());

        assert_eq!(metadata.user_id, Some(user_id));
        assert_eq!(metadata.correlation_id, Some(correlation_id));
        assert_eq!(metadata.custom.get("source"), Some(&"api".to_string()));
    }

    #[test]
    fn test_snapshot_creation() {
        let mut repo = EventSourcedRepository::<Account>::new(3);
        let account_id = Uuid::new_v4();

        // Create enough events to trigger snapshot
        let events = vec![
            AccountEvent::Created {
                id: account_id,
                name: "Bob".to_string(),
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 100,
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 200,
            },
        ];

        repo.save(events, EventMetadata::new()).unwrap();

        // Verify snapshot exists
        let snapshot = repo
            .event_store()
            .load_snapshot::<Account>(account_id)
            .unwrap();
        assert!(snapshot.is_some());
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.state.balance, 300);
    }

    #[test]
    fn test_event_replay() {
        let mut repo = EventSourcedRepository::<Account>::new(100);
        let account_id = Uuid::new_v4();

        let events = vec![
            AccountEvent::Created {
                id: account_id,
                name: "Charlie".to_string(),
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 50,
            },
            AccountEvent::Deposited {
                id: account_id,
                amount: 30,
            },
        ];

        repo.save(events, EventMetadata::new()).unwrap();

        let states = repo.replay(account_id).unwrap();
        assert_eq!(states.len(), 3);
        assert_eq!(states[0].balance, 0); // After creation
        assert_eq!(states[1].balance, 50); // After first deposit
        assert_eq!(states[2].balance, 80); // After second deposit
    }

    #[test]
    fn test_get_events_after() {
        let mut store = DomainEventStore::new();
        let account_id = Uuid::new_v4();

        store.append(
            AccountEvent::Created {
                id: account_id,
                name: "Test".to_string(),
            },
            EventMetadata::new(),
        );
        store.append(
            AccountEvent::Deposited {
                id: account_id,
                amount: 100,
            },
            EventMetadata::new(),
        );
        store.append(
            AccountEvent::Deposited {
                id: account_id,
                amount: 50,
            },
            EventMetadata::new(),
        );

        let events_after_1 = store.get_events_after(account_id, 1);
        assert_eq!(events_after_1.len(), 2);
        assert!(events_after_1[0].sequence > 1);
    }

    #[test]
    fn test_load_nonexistent_aggregate() {
        let repo = EventSourcedRepository::<Account>::new(10);
        let account_id = Uuid::new_v4();

        let result = repo.load(account_id).unwrap();
        assert!(result.is_none());
    }
}
