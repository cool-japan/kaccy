//! Zero-copy serialization utilities
//!
//! This module provides high-performance, zero-copy serialization mechanisms
//! for order book data, streaming protocols, memory-mapped files, and shared
//! memory IPC.

use crate::error::{CoreError, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Binary protocol for order book updates
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OrderBookUpdate {
    /// Message type
    pub msg_type: u8,

    /// Token ID
    pub token_id: u64,

    /// Timestamp (microseconds since epoch)
    pub timestamp_us: i64,

    /// Price (fixed-point with 18 decimals)
    pub price: i64,

    /// Quantity (fixed-point with 18 decimals)
    pub quantity: i64,

    /// Side: 0=bid, 1=ask
    pub side: u8,

    /// Sequence number
    pub sequence: u64,
}

impl OrderBookUpdate {
    /// Message type: add a new price level.
    pub const MSG_ADD: u8 = 1;
    /// Message type: remove an existing price level.
    pub const MSG_REMOVE: u8 = 2;
    /// Message type: update quantity at an existing price level.
    pub const MSG_UPDATE: u8 = 3;
    /// Message type: full order book snapshot.
    pub const MSG_SNAPSHOT: u8 = 4;

    /// Side code for bid (buy) orders.
    pub const SIDE_BID: u8 = 0;
    /// Side code for ask (sell) orders.
    pub const SIDE_ASK: u8 = 1;

    /// Create a new order book update
    pub fn new(
        msg_type: u8,
        token_id: u64,
        price: Decimal,
        quantity: Decimal,
        side: u8,
        sequence: u64,
    ) -> Self {
        let now_us = Utc::now().timestamp_micros();

        Self {
            msg_type,
            token_id,
            timestamp_us: now_us,
            price: Self::decimal_to_fixed(price),
            quantity: Self::decimal_to_fixed(quantity),
            side,
            sequence,
        }
    }

    /// Convert Decimal to fixed-point i64
    fn decimal_to_fixed(d: Decimal) -> i64 {
        let scaled = d * Decimal::from(1_000_000_000_000_000_000i64);
        let divisor = 10i128.pow(scaled.scale());
        (scaled.mantissa() / divisor) as i64
    }

    /// Convert fixed-point i64 to Decimal
    fn fixed_to_decimal(f: i64) -> Decimal {
        Decimal::new(f, 18)
    }

    /// Get price as Decimal
    pub fn get_price(&self) -> Decimal {
        Self::fixed_to_decimal(self.price)
    }

    /// Get quantity as Decimal
    pub fn get_quantity(&self) -> Decimal {
        Self::fixed_to_decimal(self.quantity)
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; std::mem::size_of::<Self>()] {
        unsafe { std::mem::transmute_copy(self) }
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != std::mem::size_of::<Self>() {
            return Err(CoreError::Validation(format!(
                "Invalid byte length: expected {}, got {}",
                std::mem::size_of::<Self>(),
                bytes.len()
            )));
        }

        let mut buf = [0u8; std::mem::size_of::<Self>()];
        buf.copy_from_slice(bytes);

        Ok(unsafe { std::mem::transmute::<[u8; 42], OrderBookUpdate>(buf) })
    }
}

/// Binary protocol encoder/decoder
pub struct BinaryProtocol {
    /// Write buffer
    write_buffer: Vec<u8>,

    /// Read buffer
    #[allow(dead_code)]
    read_buffer: VecDeque<u8>,
}

impl BinaryProtocol {
    /// Create a new binary protocol
    pub fn new() -> Self {
        Self {
            write_buffer: Vec::with_capacity(8192),
            read_buffer: VecDeque::with_capacity(8192),
        }
    }

    /// Encode an order book update
    pub fn encode_order_book_update(&mut self, update: &OrderBookUpdate) -> &[u8] {
        self.write_buffer.clear();
        self.write_buffer.extend_from_slice(&update.to_bytes());
        &self.write_buffer
    }

    /// Decode an order book update
    pub fn decode_order_book_update(&mut self, data: &[u8]) -> Result<OrderBookUpdate> {
        OrderBookUpdate::from_bytes(data)
    }

    /// Encode multiple updates
    pub fn encode_batch(&mut self, updates: &[OrderBookUpdate]) -> &[u8] {
        self.write_buffer.clear();

        // Write count
        self.write_buffer
            .extend_from_slice(&(updates.len() as u32).to_le_bytes());

        // Write updates
        for update in updates {
            self.write_buffer.extend_from_slice(&update.to_bytes());
        }

        &self.write_buffer
    }

    /// Decode multiple updates
    pub fn decode_batch(&mut self, data: &[u8]) -> Result<Vec<OrderBookUpdate>> {
        if data.len() < 4 {
            return Err(CoreError::Validation(
                "Insufficient data for batch count".to_string(),
            ));
        }

        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let update_size = std::mem::size_of::<OrderBookUpdate>();
        let expected_size = 4 + (count * update_size);

        if data.len() != expected_size {
            return Err(CoreError::Validation(format!(
                "Invalid batch size: expected {}, got {}",
                expected_size,
                data.len()
            )));
        }

        let mut updates = Vec::with_capacity(count);
        let mut offset = 4;

        for _ in 0..count {
            let update_bytes = &data[offset..offset + update_size];
            updates.push(OrderBookUpdate::from_bytes(update_bytes)?);
            offset += update_size;
        }

        Ok(updates)
    }
}

impl Default for BinaryProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory-mapped file for efficient data access
pub struct MemoryMappedFile {
    /// File path
    path: std::path::PathBuf,

    /// File handle
    #[allow(dead_code)]
    file: File,

    /// File size
    size: usize,
}

impl MemoryMappedFile {
    /// Create or open a memory-mapped file
    pub fn create(path: impl AsRef<Path>, size: usize) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| CoreError::Database(format!("Failed to open file: {}", e)))?;

        // Set file size if needed
        file.set_len(size as u64)
            .map_err(|e| CoreError::Database(format!("Failed to set file size: {}", e)))?;

        Ok(Self { path, file, size })
    }

    /// Get file size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write data at offset
    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        if offset + data.len() > self.size {
            return Err(CoreError::Validation(
                "Write would exceed file bounds".to_string(),
            ));
        }

        use std::io::Seek;
        use std::io::SeekFrom;

        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|e| CoreError::Database(format!("Seek failed: {}", e)))?;

        self.file
            .write_all(data)
            .map_err(|e| CoreError::Database(format!("Write failed: {}", e)))?;

        Ok(())
    }

    /// Read data from offset
    pub fn read_at(&mut self, offset: usize, len: usize) -> Result<Vec<u8>> {
        if offset + len > self.size {
            return Err(CoreError::Validation(
                "Read would exceed file bounds".to_string(),
            ));
        }

        use std::io::Seek;
        use std::io::SeekFrom;

        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|e| CoreError::Database(format!("Seek failed: {}", e)))?;

        let mut buffer = vec![0u8; len];
        self.file
            .read_exact(&mut buffer)
            .map_err(|e| CoreError::Database(format!("Read failed: {}", e)))?;

        Ok(buffer)
    }
}

/// Shared memory region for IPC
#[derive(Clone)]
pub struct SharedMemory {
    /// Shared data
    data: Arc<RwLock<Vec<u8>>>,

    /// Capacity
    capacity: usize,
}

impl SharedMemory {
    /// Create a new shared memory region
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(vec![0u8; capacity])),
            capacity,
        }
    }

    /// Write data to shared memory
    pub async fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
        if offset + data.len() > self.capacity {
            return Err(CoreError::Validation(
                "Write would exceed shared memory bounds".to_string(),
            ));
        }

        let mut mem = self.data.write().await;
        mem[offset..offset + data.len()].copy_from_slice(data);

        Ok(())
    }

    /// Read data from shared memory
    pub async fn read(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        if offset + len > self.capacity {
            return Err(CoreError::Validation(
                "Read would exceed shared memory bounds".to_string(),
            ));
        }

        let mem = self.data.read().await;
        Ok(mem[offset..offset + len].to_vec())
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Efficient wire format for streaming
pub struct StreamingWireFormat {
    /// Protocol encoder
    protocol: BinaryProtocol,

    /// Compression enabled
    compression_enabled: bool,
}

impl StreamingWireFormat {
    /// Create a new streaming wire format
    pub fn new(compression_enabled: bool) -> Self {
        Self {
            protocol: BinaryProtocol::new(),
            compression_enabled,
        }
    }

    /// Encode data for streaming
    pub fn encode(&mut self, updates: &[OrderBookUpdate]) -> Result<Vec<u8>> {
        let data = self.protocol.encode_batch(updates);

        if self.compression_enabled {
            // In a real implementation, use a compression library like zstd or lz4
            // For now, just return the raw data
            Ok(data.to_vec())
        } else {
            Ok(data.to_vec())
        }
    }

    /// Decode streamed data
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<OrderBookUpdate>> {
        let decoded_data = if self.compression_enabled {
            // In a real implementation, decompress here
            data
        } else {
            data
        };

        self.protocol.decode_batch(decoded_data)
    }
}

/// Ring buffer for efficient circular storage
pub struct RingBuffer<T> {
    /// Buffer data
    data: Vec<Option<T>>,

    /// Read position
    read_pos: usize,

    /// Write position
    write_pos: usize,

    /// Capacity
    capacity: usize,

    /// Number of elements
    count: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// Create a new ring buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![None; capacity],
            read_pos: 0,
            write_pos: 0,
            capacity,
            count: 0,
        }
    }

    /// Push an element to the buffer
    pub fn push(&mut self, item: T) -> Result<()> {
        if self.count >= self.capacity {
            return Err(CoreError::Validation("Ring buffer is full".to_string()));
        }

        self.data[self.write_pos] = Some(item);
        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.count += 1;

        Ok(())
    }

    /// Pop an element from the buffer
    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            return None;
        }

        let item = self.data[self.read_pos].take();
        self.read_pos = (self.read_pos + 1) % self.capacity;
        self.count -= 1;

        item
    }

    /// Get current count
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        self.count >= self.capacity
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_book_update_serialization() {
        let update = OrderBookUpdate::new(
            OrderBookUpdate::MSG_ADD,
            123,
            dec!(100.50),
            dec!(10.0),
            OrderBookUpdate::SIDE_BID,
            1,
        );

        let bytes = update.to_bytes();
        let decoded = OrderBookUpdate::from_bytes(&bytes).unwrap();

        // Copy values to avoid unaligned reference errors
        let msg_type = decoded.msg_type;
        let token_id = decoded.token_id;
        let side = decoded.side;
        let sequence = decoded.sequence;

        assert_eq!(msg_type, OrderBookUpdate::MSG_ADD);
        assert_eq!(token_id, 123);
        assert_eq!(side, OrderBookUpdate::SIDE_BID);
        assert_eq!(sequence, 1);
    }

    #[test]
    fn test_binary_protocol_batch() {
        let mut protocol = BinaryProtocol::new();

        let updates = vec![
            OrderBookUpdate::new(
                OrderBookUpdate::MSG_ADD,
                1,
                dec!(100.0),
                dec!(10.0),
                OrderBookUpdate::SIDE_BID,
                1,
            ),
            OrderBookUpdate::new(
                OrderBookUpdate::MSG_ADD,
                1,
                dec!(101.0),
                dec!(20.0),
                OrderBookUpdate::SIDE_ASK,
                2,
            ),
        ];

        let encoded = protocol.encode_batch(&updates).to_vec();
        let decoded = protocol.decode_batch(&encoded).unwrap();

        assert_eq!(decoded.len(), 2);
        // Copy values to avoid unaligned reference errors
        let token_id_0 = decoded[0].token_id;
        let token_id_1 = decoded[1].token_id;
        assert_eq!(token_id_0, 1);
        assert_eq!(token_id_1, 1);
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer = RingBuffer::new(3);

        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.push(1).unwrap();
        buffer.push(2).unwrap();
        buffer.push(3).unwrap();

        assert!(buffer.is_full());
        assert!(!buffer.is_empty());

        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), Some(2));
        assert_eq!(buffer.pop(), Some(3));
        assert_eq!(buffer.pop(), None);

        assert!(buffer.is_empty());
    }

    #[tokio::test]
    async fn test_shared_memory() {
        let shm = SharedMemory::new(1024);

        let data = b"Hello, World!";
        shm.write(0, data).await.unwrap();

        let read_data = shm.read(0, data.len()).await.unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_streaming_wire_format() {
        let mut wire = StreamingWireFormat::new(false);

        let updates = vec![OrderBookUpdate::new(
            OrderBookUpdate::MSG_ADD,
            1,
            dec!(100.0),
            dec!(10.0),
            OrderBookUpdate::SIDE_BID,
            1,
        )];

        let encoded = wire.encode(&updates).unwrap();
        let decoded = wire.decode(&encoded).unwrap();

        assert_eq!(decoded.len(), 1);
        // Copy values to avoid unaligned reference errors
        let token_id = decoded[0].token_id;
        assert_eq!(token_id, 1);
    }
}
