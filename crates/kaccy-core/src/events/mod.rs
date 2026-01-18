//! Event system for domain events and real-time notifications

pub mod bus;
pub mod payload;
pub mod persistence;
pub mod sourcing;
pub mod subscriber;
pub mod types;

pub use bus::{EventBus, EventDispatcher};
pub use payload::EventPayload;
pub use persistence::{EventStore, PersistedEvent};
pub use sourcing::{
    AggregateRoot, DomainEvent, DomainEventStore, EventMetadata, EventSourcedRepository, Snapshot,
    StoredEvent,
};
pub use subscriber::{EventSubscriber, SubscriberHandle};
pub use types::*;
