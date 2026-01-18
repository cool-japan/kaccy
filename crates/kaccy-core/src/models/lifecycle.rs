//! Model lifecycle hooks for before/after operations

use crate::error::Result;
use sqlx::PgPool;

/// Trait for models that support lifecycle hooks
#[async_trait::async_trait]
pub trait Lifecycle: Sized + Send + Sync {
    /// Called before creating a new model instance
    async fn before_create(&mut self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called after creating a new model instance
    async fn after_create(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called before updating a model instance
    async fn before_update(&mut self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called after updating a model instance
    async fn after_update(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called before deleting a model instance
    async fn before_delete(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called after deleting a model instance
    async fn after_delete(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Called before saving (create or update)
    async fn before_save(&mut self, pool: &PgPool) -> Result<()> {
        self.validate(pool).await?;
        Ok(())
    }

    /// Called after saving (create or update)
    async fn after_save(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }

    /// Validate the model before save
    async fn validate(&self, _pool: &PgPool) -> Result<()> {
        Ok(())
    }
}

/// Hook executor for managing lifecycle operations
pub struct HookExecutor<T> {
    model: T,
}

impl<T: Lifecycle> HookExecutor<T> {
    /// Create a new hook executor
    pub fn new(model: T) -> Self {
        Self { model }
    }

    /// Execute create with hooks
    pub async fn create(mut self, pool: &PgPool) -> Result<T> {
        self.model.before_create(pool).await?;
        self.model.before_save(pool).await?;

        // The actual create operation would be performed here
        // by the calling code after this returns

        self.model.after_save(pool).await?;
        self.model.after_create(pool).await?;

        Ok(self.model)
    }

    /// Execute update with hooks
    pub async fn update(mut self, pool: &PgPool) -> Result<T> {
        self.model.before_update(pool).await?;
        self.model.before_save(pool).await?;

        // The actual update operation would be performed here
        // by the calling code after this returns

        self.model.after_save(pool).await?;
        self.model.after_update(pool).await?;

        Ok(self.model)
    }

    /// Execute delete with hooks
    pub async fn delete(self, pool: &PgPool) -> Result<T> {
        self.model.before_delete(pool).await?;

        // The actual delete operation would be performed here
        // by the calling code after this returns

        self.model.after_delete(pool).await?;

        Ok(self.model)
    }

    /// Get the underlying model
    pub fn into_inner(self) -> T {
        self.model
    }
}

impl<T: Lifecycle> AsRef<T> for HookExecutor<T> {
    fn as_ref(&self) -> &T {
        &self.model
    }
}

impl<T: Lifecycle> AsMut<T> for HookExecutor<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.model
    }
}

/// Observer trait for watching model changes
#[async_trait::async_trait]
pub trait ModelObserver<T>: Send + Sync {
    /// Called when a model is created
    async fn on_created(&self, model: &T, pool: &PgPool) -> Result<()>;

    /// Called when a model is updated
    async fn on_updated(&self, model: &T, pool: &PgPool) -> Result<()>;

    /// Called when a model is deleted
    async fn on_deleted(&self, model: &T, pool: &PgPool) -> Result<()>;
}

/// Registry for model observers
pub struct ObserverRegistry<T> {
    observers: Vec<Box<dyn ModelObserver<T>>>,
}

impl<T> ObserverRegistry<T> {
    /// Create a new observer registry
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    /// Register an observer
    pub fn register(&mut self, observer: Box<dyn ModelObserver<T>>) {
        self.observers.push(observer);
    }

    /// Notify all observers of a create event
    pub async fn notify_created(&self, model: &T, pool: &PgPool) -> Result<()> {
        for observer in &self.observers {
            observer.on_created(model, pool).await?;
        }
        Ok(())
    }

    /// Notify all observers of an update event
    pub async fn notify_updated(&self, model: &T, pool: &PgPool) -> Result<()> {
        for observer in &self.observers {
            observer.on_updated(model, pool).await?;
        }
        Ok(())
    }

    /// Notify all observers of a delete event
    pub async fn notify_deleted(&self, model: &T, pool: &PgPool) -> Result<()> {
        for observer in &self.observers {
            observer.on_deleted(model, pool).await?;
        }
        Ok(())
    }
}

impl<T> Default for ObserverRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation context for models
pub struct ValidationContext {
    /// Validation errors
    errors: Vec<String>,
}

impl ValidationContext {
    /// Create a new validation context
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add a validation error
    pub fn add_error(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors
            .push(format!("{}: {}", field.into(), message.into()));
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get all errors
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Convert to a result
    pub fn into_result(self) -> Result<()> {
        if self.has_errors() {
            Err(crate::error::CoreError::Validation(self.errors.join(", ")))
        } else {
            Ok(())
        }
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestModel {
        name: String,
        validated: bool,
    }

    #[async_trait::async_trait]
    impl Lifecycle for TestModel {
        async fn validate(&self, _pool: &PgPool) -> Result<()> {
            let mut ctx = ValidationContext::new();

            if self.name.is_empty() {
                ctx.add_error("name", "cannot be empty");
            }

            ctx.into_result()
        }

        async fn before_create(&mut self, _pool: &PgPool) -> Result<()> {
            self.validated = true;
            Ok(())
        }
    }

    #[test]
    fn test_validation_context() {
        let mut ctx = ValidationContext::new();
        assert!(!ctx.has_errors());

        ctx.add_error("field1", "error1");
        assert!(ctx.has_errors());
        assert_eq!(ctx.errors().len(), 1);

        ctx.add_error("field2", "error2");
        assert_eq!(ctx.errors().len(), 2);
    }

    #[test]
    fn test_validation_context_result() {
        let ctx = ValidationContext::new();
        assert!(ctx.into_result().is_ok());

        let mut ctx = ValidationContext::new();
        ctx.add_error("field", "error");
        assert!(ctx.into_result().is_err());
    }

    #[test]
    fn test_hook_executor_creation() {
        let model = TestModel {
            name: "test".to_string(),
            validated: false,
        };

        let executor = HookExecutor::new(model);
        assert_eq!(executor.as_ref().name, "test");
        assert!(!executor.as_ref().validated);
    }
}
