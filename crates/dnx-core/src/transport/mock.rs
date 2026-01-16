//! Mock USB transport for testing.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::traits::{TransportError, UsbTransport};
use crate::protocol::AckCode;

/// Mock transport for unit testing state machine logic.
pub struct MockTransport {
    /// Queued ACKs to return on read.
    ack_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Captured writes.
    write_log: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Simulated VID/PID.
    vid: u16,
    pid: u16,
    /// Whether device is "connected".
    connected: Arc<Mutex<bool>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            ack_queue: Arc::new(Mutex::new(VecDeque::new())),
            write_log: Arc::new(Mutex::new(Vec::new())),
            vid: 0x8086,
            pid: 0xE004,
            connected: Arc::new(Mutex::new(true)),
        }
    }

    /// Queue an ACK response to be returned on next read.
    pub fn queue_ack(&self, ack_bytes: &[u8]) {
        self.ack_queue.lock().unwrap().push_back(ack_bytes.to_vec());
    }

    /// Queue an ACK from a u32 constant.
    pub fn queue_ack_u32(&self, ack: u32) {
        self.queue_ack(&ack.to_be_bytes());
    }

    /// Queue an ACK from a u64 constant (for 5+ byte ACKs).
    pub fn queue_ack_u64(&self, ack: u64, len: usize) {
        let bytes = ack.to_be_bytes();
        let start = 8 - len;
        self.queue_ack(&bytes[start..]);
    }

    /// Get all captured writes.
    pub fn get_writes(&self) -> Vec<Vec<u8>> {
        self.write_log.lock().unwrap().clone()
    }

    /// Clear captured writes.
    pub fn clear_writes(&self) {
        self.write_log.lock().unwrap().clear();
    }

    /// Simulate device disconnect.
    pub fn disconnect(&self) {
        *self.connected.lock().unwrap() = false;
    }

    /// Simulate device reconnect.
    pub fn reconnect(&self) {
        *self.connected.lock().unwrap() = true;
    }

    /// Set VID/PID for re-enumeration testing.
    pub fn set_ids(&mut self, vid: u16, pid: u16) {
        self.vid = vid;
        self.pid = pid;
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbTransport for MockTransport {
    fn write(&self, data: &[u8]) -> Result<usize, TransportError> {
        if !*self.connected.lock().unwrap() {
            return Err(TransportError::Disconnected);
        }
        self.write_log.lock().unwrap().push(data.to_vec());
        Ok(data.len())
    }

    fn read(&self, _max_len: usize) -> Result<Vec<u8>, TransportError> {
        if !*self.connected.lock().unwrap() {
            return Err(TransportError::Disconnected);
        }
        self.ack_queue
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(TransportError::Timeout { timeout_ms: 5000 })
    }

    fn read_ack(&self) -> Result<AckCode, TransportError> {
        let bytes = self.read(512)?;
        Ok(AckCode::from_bytes(&bytes))
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }

    fn vendor_id(&self) -> u16 {
        self.vid
    }

    fn product_id(&self) -> u16 {
        self.pid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::*;

    #[test]
    fn test_mock_ack_queue() {
        let mock = MockTransport::new();
        mock.queue_ack_u32(BULK_ACK_DFRM);
        mock.queue_ack_u32(BULK_ACK_DONE);

        let ack1 = mock.read_ack().unwrap();
        assert!(ack1.matches_u32(BULK_ACK_DFRM));

        let ack2 = mock.read_ack().unwrap();
        assert!(ack2.matches_u32(BULK_ACK_DONE));

        // Queue is empty now
        assert!(mock.read_ack().is_err());
    }

    #[test]
    fn test_mock_write_capture() {
        let mock = MockTransport::new();
        mock.write(b"Hello").unwrap();
        mock.write(b"World").unwrap();

        let writes = mock.get_writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], b"Hello");
        assert_eq!(writes[1], b"World");
    }

    #[test]
    fn test_mock_disconnect() {
        let mock = MockTransport::new();
        assert!(mock.is_connected());

        mock.disconnect();
        assert!(!mock.is_connected());
        assert!(mock.write(b"test").is_err());
    }
}
