//! Memory profiling module
//!
//! Provides a `GlobalAlloc`-wrapping `TrackingAllocator` for precise, atomic
//! allocation accounting, a `MemoryProfiler` utility that captures deltas
//! between snapshots, RAII `MemoryGuard` scopes, and lightweight value types
//! `MemoryStats` / `MemoryDelta` for reporting.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::alloc::System;
//! use kaccy_core::utils::memory_profiler::{TrackingAllocator, MemoryProfiler};
//!
//! static ALLOC: TrackingAllocator<System> = TrackingAllocator::new(System);
//!
//! fn main() {
//!     let profiler = MemoryProfiler::start("my-label", &ALLOC);
//!     let _v: Vec<u8> = Vec::with_capacity(1024);
//!     let report = profiler.report(&ALLOC);
//!     println!("{report}");
//! }
//! ```

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// MemoryStats
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of allocator counters.
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    /// Bytes currently live (allocated but not yet freed).
    pub current_bytes: usize,
    /// Maximum `current_bytes` value observed since last reset.
    pub peak_bytes: usize,
    /// Cumulative bytes handed to callers via `alloc`.
    pub total_allocated_bytes: u64,
    /// Cumulative bytes returned via `dealloc`.
    pub total_freed_bytes: u64,
    /// Number of successful `alloc` calls.
    pub allocation_count: u64,
    /// Number of `dealloc` calls.
    pub free_count: u64,
}

impl MemoryStats {
    /// Currently live bytes (same as `current_bytes`; provided for ergonomics).
    #[inline]
    pub fn live_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Number of allocations that have not yet been freed.
    #[inline]
    pub fn live_count(&self) -> u64 {
        self.allocation_count.saturating_sub(self.free_count)
    }

    /// Ratio of peak to current bytes.
    ///
    /// Returns 1.0 when `current_bytes == 0` to avoid division by zero.
    /// Values > 1.0 indicate headroom has been reclaimed since the peak.
    #[inline]
    pub fn fragmentation_ratio(&self) -> f64 {
        if self.current_bytes == 0 {
            1.0
        } else {
            self.peak_bytes as f64 / self.current_bytes as f64
        }
    }
}

// ---------------------------------------------------------------------------
// TrackingAllocator
// ---------------------------------------------------------------------------

/// A `GlobalAlloc` adapter that wraps any inner allocator and maintains
/// per-allocation accounting using lock-free atomics.
///
/// # Usage as global allocator
///
/// ```rust,no_run
/// use std::alloc::System;
/// use kaccy_core::utils::memory_profiler::TrackingAllocator;
///
/// #[global_allocator]
/// static GLOBAL: TrackingAllocator<System> = TrackingAllocator::new(System);
/// ```
pub struct TrackingAllocator<A: GlobalAlloc> {
    inner: A,
    current: AtomicUsize,
    peak: AtomicUsize,
    total_alloc: AtomicU64,
    total_free: AtomicU64,
    alloc_count: AtomicU64,
    free_count: AtomicU64,
}

// SAFETY: All mutable state is mediated through atomics; the inner allocator
// must itself be Send + Sync (which `System` is).
unsafe impl<A: GlobalAlloc + Send> Send for TrackingAllocator<A> {}
unsafe impl<A: GlobalAlloc + Sync> Sync for TrackingAllocator<A> {}

impl<A: GlobalAlloc> TrackingAllocator<A> {
    /// Construct a new `TrackingAllocator` wrapping `inner`.
    ///
    /// This function is `const` so it can be used in `static` declarations.
    pub const fn new(inner: A) -> Self {
        Self {
            inner,
            current: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            total_alloc: AtomicU64::new(0),
            total_free: AtomicU64::new(0),
            alloc_count: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
        }
    }

    /// Read a consistent snapshot of all counters.
    pub fn stats(&self) -> MemoryStats {
        // Acquire ordering ensures we see all writes that preceded any release.
        let current_bytes = self.current.load(Ordering::Acquire);
        let peak_bytes = self.peak.load(Ordering::Acquire);
        let total_allocated_bytes = self.total_alloc.load(Ordering::Acquire);
        let total_freed_bytes = self.total_free.load(Ordering::Acquire);
        let allocation_count = self.alloc_count.load(Ordering::Acquire);
        let free_count = self.free_count.load(Ordering::Acquire);

        MemoryStats {
            current_bytes,
            peak_bytes,
            total_allocated_bytes,
            total_freed_bytes,
            allocation_count,
            free_count,
        }
    }

    /// Reset the peak counter to the current live byte count.
    ///
    /// Useful when starting a new measurement window.
    pub fn reset_peak(&self) {
        let current = self.current.load(Ordering::Acquire);
        self.peak.store(current, Ordering::Release);
    }

    /// Atomically update `peak` to `new_current` if `new_current` exceeds the
    /// stored peak.  Uses a CAS loop so concurrent updates are handled safely.
    fn update_peak(&self, new_current: usize) {
        let mut stored = self.peak.load(Ordering::Relaxed);
        loop {
            if new_current <= stored {
                break;
            }
            match self.peak.compare_exchange_weak(
                stored,
                new_current,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => stored = actual,
            }
        }
    }
}

// SAFETY: We delegate every allocation and deallocation to the inner allocator,
// adding only atomic bookkeeping.  The returned pointers are exactly those
// provided by the inner allocator and are therefore valid per its contract.
unsafe impl<A: GlobalAlloc> GlobalAlloc for TrackingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Caller guarantees layout is valid; we forward directly.
        let ptr = unsafe { self.inner.alloc(layout) };
        if !ptr.is_null() {
            let size = layout.size();
            // Relaxed is fine for pure statistics; no happens-before is needed.
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_alloc.fetch_add(size as u64, Ordering::Relaxed);
            let new_current = self.current.fetch_add(size, Ordering::Relaxed) + size;
            self.update_peak(new_current);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: Caller guarantees ptr was allocated with this layout.
        unsafe { self.inner.dealloc(ptr, layout) };
        let size = layout.size();
        self.free_count.fetch_add(1, Ordering::Relaxed);
        self.total_free.fetch_add(size as u64, Ordering::Relaxed);
        // Saturating sub avoids underflow if stats are reset mid-flight.
        self.current
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                Some(c.saturating_sub(size))
            })
            .ok();
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Caller guarantees layout is valid; we forward directly.
        let ptr = unsafe { self.inner.alloc_zeroed(layout) };
        if !ptr.is_null() {
            let size = layout.size();
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_alloc.fetch_add(size as u64, Ordering::Relaxed);
            let new_current = self.current.fetch_add(size, Ordering::Relaxed) + size;
            self.update_peak(new_current);
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        // SAFETY: Caller guarantees ptr/layout/new_size are valid.
        let new_ptr = unsafe { self.inner.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // Account for the net change in live bytes.
            if new_size > old_size {
                let delta = new_size - old_size;
                self.total_alloc.fetch_add(delta as u64, Ordering::Relaxed);
                let new_current = self.current.fetch_add(delta, Ordering::Relaxed) + delta;
                self.update_peak(new_current);
            } else {
                let delta = old_size - new_size;
                self.total_free.fetch_add(delta as u64, Ordering::Relaxed);
                self.current
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                        Some(c.saturating_sub(delta))
                    })
                    .ok();
            }
        }
        new_ptr
    }
}

// ---------------------------------------------------------------------------
// MemoryDelta
// ---------------------------------------------------------------------------

/// The change in memory metrics between two `MemoryProfiler` snapshots.
#[derive(Debug, Clone)]
pub struct MemoryDelta {
    /// Identifying label for the measurement window.
    pub label: String,
    /// Signed change in live bytes (positive = more memory in use).
    pub current_delta_bytes: i64,
    /// Signed change in the peak high-water mark.
    pub peak_delta_bytes: i64,
    /// Allocations made during the window.
    pub new_allocations: u64,
    /// Wall-clock time elapsed during the window (milliseconds).
    pub elapsed_ms: u64,
}

impl MemoryDelta {
    /// Format a signed byte count as a human-readable string with SI suffix.
    ///
    /// Examples: `"+1.50 KB"`, `"-2.00 MB"`, `"+512 B"`.
    pub fn format_bytes(bytes: i64) -> String {
        let sign = if bytes >= 0 { "+" } else { "-" };
        let abs = bytes.unsigned_abs();

        if abs >= 1_073_741_824 {
            format!("{}{:.2} GB", sign, abs as f64 / 1_073_741_824.0)
        } else if abs >= 1_048_576 {
            format!("{}{:.2} MB", sign, abs as f64 / 1_048_576.0)
        } else if abs >= 1_024 {
            format!("{}{:.2} KB", sign, abs as f64 / 1_024.0)
        } else {
            format!("{}{} B", sign, abs)
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryProfiler
// ---------------------------------------------------------------------------

/// Utility that captures a baseline snapshot and computes deltas on demand.
///
/// Create with [`MemoryProfiler::start`], then call [`snapshot`] or [`report`].
///
/// [`snapshot`]: MemoryProfiler::snapshot
/// [`report`]: MemoryProfiler::report
pub struct MemoryProfiler {
    baseline: MemoryStats,
    label: String,
    started_at: Instant,
}

impl MemoryProfiler {
    /// Capture the current allocator state and begin a measurement window.
    pub fn start(label: &str, allocator: &TrackingAllocator<impl GlobalAlloc>) -> Self {
        Self {
            baseline: allocator.stats(),
            label: label.to_owned(),
            started_at: Instant::now(),
        }
    }

    /// Compute a [`MemoryDelta`] against the baseline snapshot.
    pub fn snapshot(&self, allocator: &TrackingAllocator<impl GlobalAlloc>) -> MemoryDelta {
        let current = allocator.stats();
        let elapsed_ms = self.started_at.elapsed().as_millis() as u64;

        let current_delta_bytes = current.current_bytes as i64 - self.baseline.current_bytes as i64;
        let peak_delta_bytes = current.peak_bytes as i64 - self.baseline.peak_bytes as i64;
        let new_allocations = current
            .allocation_count
            .saturating_sub(self.baseline.allocation_count);

        MemoryDelta {
            label: self.label.clone(),
            current_delta_bytes,
            peak_delta_bytes,
            new_allocations,
            elapsed_ms,
        }
    }

    /// Return a human-readable multi-line report string.
    pub fn report(&self, allocator: &TrackingAllocator<impl GlobalAlloc>) -> String {
        let delta = self.snapshot(allocator);
        let current = allocator.stats();

        format!(
            "[MemoryProfiler] label={} elapsed={}ms\n  \
             live={} (delta={})\n  \
             peak={} (delta={})\n  \
             new_allocs={} live_count={}",
            delta.label,
            delta.elapsed_ms,
            MemoryDelta::format_bytes(current.current_bytes as i64),
            MemoryDelta::format_bytes(delta.current_delta_bytes),
            MemoryDelta::format_bytes(current.peak_bytes as i64),
            MemoryDelta::format_bytes(delta.peak_delta_bytes),
            delta.new_allocations,
            current.live_count(),
        )
    }
}

// ---------------------------------------------------------------------------
// MemoryGuard — RAII profiling scope
// ---------------------------------------------------------------------------

/// A RAII guard that begins memory profiling on creation and logs the delta
/// via `tracing::debug!` when dropped.
///
/// # Example
///
/// ```rust,no_run
/// use std::alloc::System;
/// use kaccy_core::utils::memory_profiler::{TrackingAllocator, MemoryGuard};
///
/// static ALLOC: TrackingAllocator<System> = TrackingAllocator::new(System);
///
/// fn my_operation() {
///     let _guard = MemoryGuard::new("my_operation", &ALLOC);
///     // … do work …
/// }
/// ```
pub struct MemoryGuard<'a, A: GlobalAlloc> {
    profiler: MemoryProfiler,
    allocator: &'a TrackingAllocator<A>,
}

impl<'a, A: GlobalAlloc> MemoryGuard<'a, A> {
    /// Begin a profiling scope with the given label.
    pub fn new(label: &str, allocator: &'a TrackingAllocator<A>) -> Self {
        Self {
            profiler: MemoryProfiler::start(label, allocator),
            allocator,
        }
    }
}

impl<'a, A: GlobalAlloc> Drop for MemoryGuard<'a, A> {
    fn drop(&mut self) {
        let delta = self.profiler.snapshot(self.allocator);
        tracing::debug!(
            label = %delta.label,
            elapsed_ms = delta.elapsed_ms,
            current_delta = %MemoryDelta::format_bytes(delta.current_delta_bytes),
            peak_delta = %MemoryDelta::format_bytes(delta.peak_delta_bytes),
            new_allocations = delta.new_allocations,
            "MemoryGuard exiting scope",
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{Layout, System};

    // Helpers ----------------------------------------------------------------

    fn make_allocator() -> TrackingAllocator<System> {
        TrackingAllocator::new(System)
    }

    // MemoryStats tests -------------------------------------------------------

    #[test]
    fn test_memory_stats_live_bytes() {
        let stats = MemoryStats {
            current_bytes: 512,
            peak_bytes: 1024,
            total_allocated_bytes: 2048,
            total_freed_bytes: 1536,
            allocation_count: 10,
            free_count: 7,
        };
        assert_eq!(stats.live_bytes(), stats.current_bytes);
        assert_eq!(stats.live_bytes(), 512);
    }

    #[test]
    fn test_memory_stats_fragmentation() {
        let stats = MemoryStats {
            current_bytes: 512,
            peak_bytes: 1024,
            total_allocated_bytes: 2048,
            total_freed_bytes: 1536,
            allocation_count: 10,
            free_count: 7,
        };
        // peak / current = 1024 / 512 = 2.0
        let ratio = stats.fragmentation_ratio();
        assert!((ratio - 2.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn test_memory_stats_fragmentation_zero_current() {
        let stats = MemoryStats {
            current_bytes: 0,
            peak_bytes: 0,
            total_allocated_bytes: 0,
            total_freed_bytes: 0,
            allocation_count: 0,
            free_count: 0,
        };
        // Should return 1.0 rather than divide by zero.
        assert!((stats.fragmentation_ratio() - 1.0_f64).abs() < f64::EPSILON);
    }

    // MemoryDelta format_bytes tests ------------------------------------------

    #[test]
    fn test_memory_delta_format_bytes_positive() {
        // Exact KB
        assert_eq!(MemoryDelta::format_bytes(1024), "+1.00 KB");
        // Exact MB
        assert_eq!(MemoryDelta::format_bytes(1_048_576), "+1.00 MB");
        // Exact GB
        assert_eq!(MemoryDelta::format_bytes(1_073_741_824), "+1.00 GB");
        // Sub-KB
        assert_eq!(MemoryDelta::format_bytes(512), "+512 B");
        // Zero
        assert_eq!(MemoryDelta::format_bytes(0), "+0 B");
    }

    #[test]
    fn test_memory_delta_sign() {
        let pos = MemoryDelta::format_bytes(2048);
        let neg = MemoryDelta::format_bytes(-2048);
        assert!(pos.starts_with('+'), "positive delta must start with '+'");
        assert!(neg.starts_with('-'), "negative delta must start with '-'");
        // Both should report the same magnitude
        assert!(pos.contains("2.00 KB"));
        assert!(neg.contains("2.00 KB"));
    }

    // TrackingAllocator tests -------------------------------------------------

    #[test]
    fn test_tracking_allocator_alloc_updates_stats() {
        let alloc = make_allocator();
        let layout = Layout::from_size_align(128, 8).expect("valid layout");

        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null(), "alloc must succeed");

        let stats = alloc.stats();
        assert_eq!(stats.allocation_count, 1);
        assert_eq!(stats.total_allocated_bytes, 128);
        assert_eq!(stats.current_bytes, 128);
        assert_eq!(stats.free_count, 0);

        unsafe { alloc.dealloc(ptr, layout) };

        let stats = alloc.stats();
        assert_eq!(stats.free_count, 1);
        assert_eq!(stats.total_freed_bytes, 128);
        assert_eq!(stats.current_bytes, 0);
    }

    #[test]
    fn test_tracking_allocator_peak_tracking() {
        let alloc = make_allocator();
        let layout128 = Layout::from_size_align(128, 8).expect("valid layout 128");
        let layout64 = Layout::from_size_align(64, 8).expect("valid layout 64");

        let ptr1 = unsafe { alloc.alloc(layout128) };
        assert!(!ptr1.is_null());
        // peak should be at least 128
        assert!(alloc.stats().peak_bytes >= 128);

        let ptr2 = unsafe { alloc.alloc(layout64) };
        assert!(!ptr2.is_null());
        // peak should now be at least 192
        assert!(alloc.stats().peak_bytes >= 192);

        // Free both and verify peak does NOT drop
        unsafe { alloc.dealloc(ptr1, layout128) };
        unsafe { alloc.dealloc(ptr2, layout64) };

        let stats = alloc.stats();
        assert_eq!(stats.current_bytes, 0, "all memory freed");
        assert!(stats.peak_bytes >= 192, "peak must not decrease on free");
    }

    #[test]
    fn test_tracking_allocator_reset_peak() {
        let alloc = make_allocator();
        let layout = Layout::from_size_align(256, 8).expect("valid layout");

        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());
        assert!(alloc.stats().peak_bytes >= 256);

        // Free to bring current to 0
        unsafe { alloc.dealloc(ptr, layout) };
        assert_eq!(alloc.stats().current_bytes, 0);

        // peak is still ≥ 256
        assert!(alloc.stats().peak_bytes >= 256);

        // After reset_peak, peak == current (0)
        alloc.reset_peak();
        assert_eq!(alloc.stats().peak_bytes, 0);
    }

    // MemoryProfiler tests ----------------------------------------------------

    #[test]
    fn test_profiler_snapshot_captures_delta() {
        let alloc = make_allocator();
        let layout = Layout::from_size_align(512, 8).expect("valid layout");

        let profiler = MemoryProfiler::start("test_profiler", &alloc);

        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());

        let delta = profiler.snapshot(&alloc);

        assert_eq!(delta.label, "test_profiler");
        assert!(
            delta.current_delta_bytes >= 512,
            "delta must reflect the 512-byte allocation"
        );
        assert!(delta.new_allocations >= 1);

        unsafe { alloc.dealloc(ptr, layout) };
    }

    #[test]
    fn test_profiler_report_is_non_empty() {
        let alloc = make_allocator();
        let layout = Layout::from_size_align(64, 8).expect("valid layout");
        let profiler = MemoryProfiler::start("report_test", &alloc);

        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());

        let report = profiler.report(&alloc);
        assert!(
            report.contains("report_test"),
            "report should contain the label"
        );

        unsafe { alloc.dealloc(ptr, layout) };
    }
}
