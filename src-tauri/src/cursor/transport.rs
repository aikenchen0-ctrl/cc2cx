//! Request-scoped in-memory transport registry.
//!
//! This is the smallest runtime boundary needed by the Cursor protocol adapter. It owns
//! request-id stickiness, append ordering, replayable output, terminal closure, and cancellation.
//! Provider execution and Cursor conversation semantics deliberately remain outside this module.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bytes::Bytes;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("transport request_id is required")]
    EmptyRequestId,
    #[error("transport request_id is invalid")]
    InvalidRequestId,
    #[error("append sequence expected {expected}, received {received}")]
    Sequence { expected: i64, received: i64 },
    #[error("transport is already closed")]
    Closed,
    #[error("transport request_id was not found")]
    NotFound,
    #[error("transport append sequence is exhausted")]
    SequenceExhausted,
    #[error("transport generation is exhausted")]
    GenerationExhausted,
}

#[derive(Clone, Default)]
pub struct TransportRegistry {
    inner: Arc<RegistryState>,
}

#[derive(Default)]
struct RegistryState {
    transports: std::sync::Mutex<HashMap<String, TransportHandle>>,
    next_generation: std::sync::atomic::AtomicU64,
}

#[derive(Clone)]
pub struct TransportHandle {
    inner: Arc<TransportState>,
}

impl std::fmt::Debug for TransportHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransportHandle")
            .field("request_id", &self.inner.request_id)
            .field("generation", &self.inner.generation)
            .finish()
    }
}

struct TransportState {
    request_id: String,
    generation: u64,
    lifecycle: std::sync::Mutex<TransportLifecycle>,
    sequence: std::sync::Mutex<AppendSequence<Bytes>>,
    output: OutputHub,
    cancellation: CancellationToken,
    started: std::sync::atomic::AtomicBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransportLifecycle {
    Open,
    Terminal,
    Disconnected,
}

#[derive(Default)]
struct OutputHub {
    state: std::sync::Mutex<OutputState>,
}

#[derive(Default)]
struct OutputState {
    history: Vec<Bytes>,
    subscribers: Vec<mpsc::UnboundedSender<Bytes>>,
    closed: bool,
}

#[derive(Default)]
struct AppendSequence<T> {
    next: i64,
    pending: BTreeMap<i64, T>,
}

impl TransportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_create(&self, request_id: &str) -> Result<TransportHandle, TransportError> {
        self.get_or_create_inner(request_id, false)
    }

    pub fn get_or_create_for_append(
        &self,
        request_id: &str,
    ) -> Result<TransportHandle, TransportError> {
        self.get_or_create_inner(request_id, true)
    }

    pub fn current(&self, request_id: &str) -> Option<TransportHandle> {
        let request_id = canonical_request_id(request_id).ok()?;
        self.inner
            .transports
            .lock()
            .expect("transport registry mutex poisoned")
            .get(&request_id)
            .cloned()
    }

    pub fn remove_if_current(&self, request_id: &str, generation: u64) -> bool {
        let Ok(request_id) = canonical_request_id(request_id) else {
            return false;
        };
        let mut transports = self
            .inner
            .transports
            .lock()
            .expect("transport registry mutex poisoned");
        if transports.get(&request_id).is_some_and(|transport| {
            transport.generation() == generation
                && (transport.is_terminal() || transport.is_disconnected())
        }) {
            transports.remove(&request_id);
            true
        } else {
            false
        }
    }

    fn get_or_create_inner(
        &self,
        request_id: &str,
        replace_closed: bool,
    ) -> Result<TransportHandle, TransportError> {
        let request_id = canonical_request_id(request_id)?;

        let mut transports = self
            .inner
            .transports
            .lock()
            .expect("transport registry mutex poisoned");
        if let Some(transport) = transports.get(&request_id) {
            if !replace_closed || (!transport.is_disconnected() && !transport.is_terminal()) {
                return Ok(transport.clone());
            }
        }
        transports.remove(&request_id);

        let generation = self
            .inner
            .next_generation
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |current| current.checked_add(1),
            )
            .map_err(|_| TransportError::GenerationExhausted)?
            .saturating_add(1);
        let transport = TransportHandle::new(request_id.clone(), generation);
        transports.insert(request_id, transport.clone());
        Ok(transport)
    }
}

const MAX_REQUEST_ID_BYTES: usize = 256;

fn canonical_request_id(request_id: &str) -> Result<String, TransportError> {
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err(TransportError::EmptyRequestId);
    }
    if request_id.len() > MAX_REQUEST_ID_BYTES {
        return Err(TransportError::InvalidRequestId);
    }
    Ok(request_id.to_owned())
}

impl TransportHandle {
    fn new(request_id: String, generation: u64) -> Self {
        Self {
            inner: Arc::new(TransportState {
                request_id,
                generation,
                lifecycle: std::sync::Mutex::new(TransportLifecycle::Open),
                sequence: std::sync::Mutex::new(AppendSequence::default()),
                output: OutputHub::default(),
                cancellation: CancellationToken::new(),
                started: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    pub fn request_id(&self) -> &str {
        &self.inner.request_id
    }

    pub fn generation(&self) -> u64 {
        self.inner.generation
    }

    pub fn append(&self, seqno: i64, payload: Bytes) -> Result<Vec<(i64, Bytes)>, TransportError> {
        let lifecycle = self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned");
        if *lifecycle != TransportLifecycle::Open {
            return Err(TransportError::Closed);
        }
        let mut sequence = self
            .inner
            .sequence
            .lock()
            .expect("transport sequence mutex poisoned");
        if seqno == i64::MAX {
            return Err(TransportError::SequenceExhausted);
        }
        if seqno < 0 || seqno < sequence.next || sequence.pending.contains_key(&seqno) {
            return Err(TransportError::Sequence {
                expected: sequence.next,
                received: seqno,
            });
        }
        sequence.pending.insert(seqno, payload);
        let mut ready = Vec::new();
        loop {
            let next = sequence.next;
            let Some(payload) = sequence.pending.remove(&next) else {
                break;
            };
            sequence.next += 1;
            ready.push((next, payload));
        }
        Ok(ready)
    }

    pub fn accept_append(&self, seqno: i64) -> Result<(), TransportError> {
        self.append(seqno, Bytes::new()).map(|_| ())
    }

    pub fn emit_frame(&self, frame: Bytes) -> bool {
        let lifecycle = self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned");
        if *lifecycle != TransportLifecycle::Open {
            return false;
        }
        self.inner.output.emit(frame)
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<Bytes> {
        self.inner.output.subscribe()
    }

    pub fn finish(&self, terminal_frame: Bytes) -> bool {
        let mut lifecycle = self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned");
        if *lifecycle != TransportLifecycle::Open {
            return false;
        }
        *lifecycle = TransportLifecycle::Terminal;
        self.inner.output.emit_and_close(terminal_frame)
    }

    /// Cancel an active stream while still publishing one explicit terminal frame to Cursor.
    ///
    /// Closing the output hub alone would make the HTTP body end without a Connect status,
    /// which leaves the client unable to distinguish cancellation from a broken connection.
    pub fn cancel_with_frame(&self, terminal_frame: Bytes) -> bool {
        let mut lifecycle = self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned");
        if *lifecycle != TransportLifecycle::Open {
            return false;
        }
        *lifecycle = TransportLifecycle::Terminal;
        self.inner.cancellation.cancel();
        self.inner.output.emit_and_close(terminal_frame)
    }

    pub fn disconnect(&self) {
        let mut lifecycle = self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned");
        if *lifecycle == TransportLifecycle::Open {
            *lifecycle = TransportLifecycle::Disconnected;
            self.inner.cancellation.cancel();
            self.inner.output.close();
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.inner.cancellation.clone()
    }

    pub(crate) fn start_once(&self) -> bool {
        !self
            .inner
            .started
            .swap(true, std::sync::atomic::Ordering::AcqRel)
    }

    pub(crate) fn is_started(&self) -> bool {
        self.inner
            .started
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_disconnected(&self) -> bool {
        *self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned")
            == TransportLifecycle::Disconnected
    }

    pub fn is_terminal(&self) -> bool {
        *self
            .inner
            .lifecycle
            .lock()
            .expect("transport lifecycle mutex poisoned")
            == TransportLifecycle::Terminal
    }
}

impl OutputHub {
    fn emit(&self, frame: Bytes) -> bool {
        let mut state = self.state.lock().expect("transport output mutex poisoned");
        if state.closed {
            return false;
        }
        state.history.push(frame.clone());
        state
            .subscribers
            .retain(|subscriber| subscriber.send(frame.clone()).is_ok());
        true
    }

    fn emit_and_close(&self, frame: Bytes) -> bool {
        let mut state = self.state.lock().expect("transport output mutex poisoned");
        if state.closed {
            return false;
        }
        state.history.push(frame.clone());
        state
            .subscribers
            .retain(|subscriber| subscriber.send(frame.clone()).is_ok());
        state.closed = true;
        state.subscribers.clear();
        true
    }

    fn subscribe(&self) -> mpsc::UnboundedReceiver<Bytes> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut state = self.state.lock().expect("transport output mutex poisoned");
        for frame in &state.history {
            let _ = sender.send(frame.clone());
        }
        if !state.closed {
            state.subscribers.push(sender);
        }
        receiver
    }

    fn close(&self) {
        let mut state = self.state.lock().expect("transport output mutex poisoned");
        state.closed = true;
        state.subscribers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{TransportError, TransportRegistry};
    use std::sync::atomic::Ordering;

    #[test]
    fn empty_registry_starts_generations_at_one() {
        let registry = TransportRegistry::new();
        let handle = registry.get_or_create("request").unwrap();
        assert_eq!(handle.generation(), 1);
    }

    #[test]
    fn generation_exhaustion_is_reported_instead_of_reusing_ids() {
        let registry = TransportRegistry::new();
        registry
            .inner
            .next_generation
            .store(u64::MAX - 1, Ordering::Relaxed);

        let first = registry
            .get_or_create_for_append("request-generation-limit")
            .expect("the maximum generation can be allocated once");
        assert_eq!(first.generation(), u64::MAX);
        first.disconnect();

        assert_eq!(
            registry
                .get_or_create_for_append("request-generation-limit")
                .expect_err("generation reuse must be rejected"),
            TransportError::GenerationExhausted
        );
    }
}
