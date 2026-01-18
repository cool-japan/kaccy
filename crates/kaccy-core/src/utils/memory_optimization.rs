//! Memory optimization utilities
//!
//! This module provides memory optimization techniques including:
//! - Object pooling for frequently allocated types
//! - Arena allocators for batch allocations
//! - Memory profiling and leak detection

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Object pool for reusing frequently allocated objects
/// Reduces allocation overhead for hot paths
pub struct ObjectPool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
}

impl<T> ObjectPool<T> {
    /// Create a new object pool with a factory function
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::new())),
            factory: Arc::new(factory),
            max_size,
        }
    }

    /// Get an object from the pool or create a new one
    pub fn acquire(&self) -> PooledObject<T> {
        let obj = self
            .pool
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| (self.factory)());

        PooledObject {
            object: Some(obj),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.pool.lock().unwrap().len()
    }

    /// Pre-warm the pool with objects
    pub fn warm(&self, count: usize) {
        let mut pool = self.pool.lock().unwrap();
        for _ in 0..count.min(self.max_size - pool.len()) {
            pool.push_back((self.factory)());
        }
    }
}

/// RAII wrapper for pooled objects
/// Automatically returns object to pool when dropped
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
    max_size: usize,
}

impl<T> PooledObject<T> {
    /// Get a reference to the underlying object
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get a mutable reference to the underlying object
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.object.take() {
            let mut pool = self.pool.lock().unwrap();
            if pool.len() < self.max_size {
                pool.push_back(obj);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// Arena allocator for batch allocations
/// Allocates objects in contiguous memory blocks
pub struct Arena<T> {
    blocks: Vec<Vec<T>>,
    block_size: usize,
    current_block: usize,
    current_offset: usize,
}

impl<T> Arena<T> {
    /// Create a new arena with specified block size
    pub fn new(block_size: usize) -> Self {
        Self {
            blocks: vec![Vec::with_capacity(block_size)],
            block_size,
            current_block: 0,
            current_offset: 0,
        }
    }

    /// Allocate space for a new object
    pub fn alloc(&mut self, value: T) -> &T {
        if self.current_offset >= self.block_size {
            // Need a new block
            self.blocks.push(Vec::with_capacity(self.block_size));
            self.current_block += 1;
            self.current_offset = 0;
        }

        let block = &mut self.blocks[self.current_block];
        block.push(value);
        self.current_offset += 1;

        &block[block.len() - 1]
    }

    /// Get total number of allocated objects
    pub fn len(&self) -> usize {
        self.blocks.iter().map(|b| b.len()).sum()
    }

    /// Check if arena is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all allocations
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.blocks.push(Vec::with_capacity(self.block_size));
        self.current_block = 0;
        self.current_offset = 0;
    }

    /// Get memory usage in bytes (approximate)
    pub fn memory_usage(&self) -> usize {
        self.blocks.len() * self.block_size * std::mem::size_of::<T>()
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Memory profiler for tracking allocations
#[derive(Debug, Clone)]
pub struct MemoryProfiler {
    allocations: Arc<Mutex<Vec<AllocationRecord>>>,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct AllocationRecord {
    id: Uuid,
    size: usize,
    location: String,
    timestamp: i64,
}

impl MemoryProfiler {
    /// Create a new memory profiler
    pub fn new(enabled: bool) -> Self {
        Self {
            allocations: Arc::new(Mutex::new(Vec::new())),
            enabled,
        }
    }

    /// Record an allocation
    pub fn record_allocation(&self, size: usize, location: &str) -> Uuid {
        if !self.enabled {
            return Uuid::new_v4();
        }

        let id = Uuid::new_v4();
        let record = AllocationRecord {
            id,
            size,
            location: location.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.allocations.lock().unwrap().push(record);
        id
    }

    /// Record a deallocation
    pub fn record_deallocation(&self, id: Uuid) {
        if !self.enabled {
            return;
        }

        let mut allocations = self.allocations.lock().unwrap();
        allocations.retain(|a| a.id != id);
    }

    /// Get current allocation count
    pub fn allocation_count(&self) -> usize {
        self.allocations.lock().unwrap().len()
    }

    /// Get total allocated bytes
    pub fn total_allocated(&self) -> usize {
        self.allocations
            .lock()
            .unwrap()
            .iter()
            .map(|a| a.size)
            .sum()
    }

    /// Check for potential memory leaks
    pub fn detect_leaks(&self, threshold_seconds: i64) -> Vec<String> {
        let now = chrono::Utc::now().timestamp();
        let allocations = self.allocations.lock().unwrap();

        allocations
            .iter()
            .filter(|a| now - a.timestamp > threshold_seconds)
            .map(|a| {
                format!(
                    "Potential leak at {}: {} bytes, age: {}s",
                    a.location,
                    a.size,
                    now - a.timestamp
                )
            })
            .collect()
    }

    /// Get allocation statistics
    pub fn stats(&self) -> MemoryStats {
        let allocations = self.allocations.lock().unwrap();
        let total_size: usize = allocations.iter().map(|a| a.size).sum();
        let count = allocations.len();

        MemoryStats {
            allocation_count: count,
            total_bytes: total_size,
            average_size: if count > 0 { total_size / count } else { 0 },
        }
    }
}

impl Default for MemoryProfiler {
    fn default() -> Self {
        Self::new(false)
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub allocation_count: usize,
    pub total_bytes: usize,
    pub average_size: usize,
}

/// Slab allocator for fixed-size objects
/// Provides O(1) allocation and deallocation
pub struct SlabAllocator<T> {
    slabs: Vec<Option<T>>,
    free_list: Vec<usize>,
    capacity: usize,
}

impl<T> SlabAllocator<T> {
    /// Create a new slab allocator with specified capacity
    pub fn new(capacity: usize) -> Self {
        let mut slabs = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slabs.push(None);
        }

        let free_list = (0..capacity).collect();

        Self {
            slabs,
            free_list,
            capacity,
        }
    }

    /// Allocate a slot and store the object
    /// Returns the slot index
    pub fn allocate(&mut self, value: T) -> Option<usize> {
        if let Some(idx) = self.free_list.pop() {
            self.slabs[idx] = Some(value);
            Some(idx)
        } else {
            None
        }
    }

    /// Deallocate a slot
    pub fn deallocate(&mut self, idx: usize) -> Option<T> {
        if idx < self.capacity {
            let value = self.slabs[idx].take();
            if value.is_some() {
                self.free_list.push(idx);
            }
            value
        } else {
            None
        }
    }

    /// Get a reference to an allocated object
    pub fn get(&self, idx: usize) -> Option<&T> {
        if idx < self.capacity {
            self.slabs[idx].as_ref()
        } else {
            None
        }
    }

    /// Get a mutable reference to an allocated object
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        if idx < self.capacity {
            self.slabs[idx].as_mut()
        } else {
            None
        }
    }

    /// Get number of allocated slots
    pub fn allocated_count(&self) -> usize {
        self.capacity - self.free_list.len()
    }

    /// Get number of free slots
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(Vec::<i32>::new, 10);

        // Acquire an object
        let mut obj1 = pool.acquire();
        obj1.push(42);
        assert_eq!(obj1.len(), 1);

        // Pool should be empty
        assert_eq!(pool.size(), 0);

        // Drop obj1, it should return to pool
        drop(obj1);
        assert_eq!(pool.size(), 1);

        // Acquire again, should reuse
        let obj2 = pool.acquire();
        assert_eq!(pool.size(), 0);
        drop(obj2);
    }

    #[test]
    fn test_pool_warm() {
        let pool = ObjectPool::new(Vec::<i32>::new, 10);
        pool.warm(5);
        assert_eq!(pool.size(), 5);
    }

    #[test]
    fn test_arena() {
        let mut arena = Arena::new(10);

        let _obj1 = arena.alloc(42);
        let _obj2 = arena.alloc(43);

        assert_eq!(arena.len(), 2);
        assert!(!arena.is_empty());

        arena.clear();
        assert!(arena.is_empty());
    }

    #[test]
    fn test_arena_multiple_blocks() {
        let mut arena = Arena::new(2);

        for i in 0..5 {
            let _ = arena.alloc(i);
        }

        assert_eq!(arena.len(), 5);
        assert_eq!(arena.blocks.len(), 3); // 2 + 2 + 1
    }

    #[test]
    fn test_memory_profiler() {
        let profiler = MemoryProfiler::new(true);

        let id1 = profiler.record_allocation(100, "test_location");
        let id2 = profiler.record_allocation(200, "test_location");

        assert_eq!(profiler.allocation_count(), 2);
        assert_eq!(profiler.total_allocated(), 300);

        profiler.record_deallocation(id1);
        assert_eq!(profiler.allocation_count(), 1);
        assert_eq!(profiler.total_allocated(), 200);

        profiler.record_deallocation(id2);
        assert_eq!(profiler.allocation_count(), 0);
    }

    #[test]
    fn test_memory_stats() {
        let profiler = MemoryProfiler::new(true);

        profiler.record_allocation(100, "loc1");
        profiler.record_allocation(200, "loc2");
        profiler.record_allocation(300, "loc3");

        let stats = profiler.stats();
        assert_eq!(stats.allocation_count, 3);
        assert_eq!(stats.total_bytes, 600);
        assert_eq!(stats.average_size, 200);
    }

    #[test]
    fn test_slab_allocator() {
        let mut slab = SlabAllocator::new(10);

        let idx1 = slab.allocate(42).unwrap();
        let idx2 = slab.allocate(43).unwrap();

        assert_eq!(slab.allocated_count(), 2);
        assert_eq!(slab.free_count(), 8);

        assert_eq!(*slab.get(idx1).unwrap(), 42);
        assert_eq!(*slab.get(idx2).unwrap(), 43);

        let val = slab.deallocate(idx1).unwrap();
        assert_eq!(val, 42);
        assert_eq!(slab.allocated_count(), 1);
        assert_eq!(slab.free_count(), 9);
    }

    #[test]
    fn test_slab_full() {
        let mut slab = SlabAllocator::new(2);

        assert!(slab.allocate(1).is_some());
        assert!(slab.allocate(2).is_some());
        assert!(slab.allocate(3).is_none()); // Full

        assert_eq!(slab.free_count(), 0);
    }
}
