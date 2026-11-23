//! Opt-in, coordinate-free native diagnostics with a bounded atomic store.
use crate::Error;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst},
    },
    time::Instant,
};

const CAPACITY: usize = 64;

/// Native callback, recovery and lifecycle phases recorded when diagnostics are enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DiagnosticPhase {
    /// An original button-up entered the low-level mouse callback.
    RawUpEntered = 1,
    /// The callback returned suppression for the original button-up.
    RawUpConsumed,
    /// The callback passed the original button-up onward.
    RawUpPassed,
    /// A stopped worker passed the original button-up onward.
    RawUpStopped,
    /// A failed worker passed the original button-up onward.
    RawUpError,
    /// Reentrant worker borrowing prevented button-up processing.
    RawUpBorrowFailed,
    /// No worker was available to process the original button-up.
    RawUpNoWorker,
    /// A callback panic prevented original button-up suppression.
    RawUpPanicked,
    /// A negative hook code required forwarding without examining its packet.
    NegativeHookCode,
    /// An SDK-marked replay up passed through the callback.
    OwnUpPassed,
    /// A recovery up was added to the owner's deferred injection queue.
    ReplayUpQueued,
    /// The owner began submitting a batch containing a recovery up.
    ReplayUpSending,
    /// Native submission of a batch containing a recovery up completed.
    ReplayUpSent,
    /// The owner received an explicit cancellation command.
    CancelCommand,
    /// The owner replaced the resolver for future presses.
    ResolverCommand,
    /// The owner received an explicit stop command.
    StopCommand,
    /// The native worker encountered a terminal failure.
    WorkerFailure,
    /// The native worker completed teardown.
    WorkerFinished,
    /// An original button-down entered the low-level mouse callback.
    RawDownEntered,
    /// The callback returned suppression for the original button-down.
    RawDownConsumed,
    /// The callback passed the original button-down onward.
    RawDownPassed,
    /// A stopped worker passed the original button-down onward.
    RawDownStopped,
    /// A failed worker passed the original button-down onward.
    RawDownError,
    /// Reentrant worker borrowing prevented button-down processing.
    RawDownBorrowFailed,
    /// No worker was available to process the original button-down.
    RawDownNoWorker,
    /// A callback panic prevented original button-down suppression.
    RawDownPanicked,
    /// An SDK-marked replay down passed through the callback.
    OwnDownPassed,
    /// Native input hooks were successfully installed.
    HooksInstalled,
    /// Native input hooks were removed during owner teardown.
    HooksRemoved,
    /// The cancellation state transition completed; deferred recovery may still be queued.
    CancelCompleted,
    /// The owner finished replacing the resolver for future presses.
    ResolverCompleted,
    /// Shutdown recovery was queued; draining, hook removal and teardown may continue.
    StopCompleted,
    /// The owner began removing native input hooks during teardown.
    HooksRemoving,
    /// An original movement entered the low-level mouse callback.
    RawMoveEntered,
    /// The callback returned suppression for the original movement.
    RawMoveConsumed,
    /// The callback passed the original movement onward.
    RawMovePassed,
    /// A stopped worker passed the original movement onward.
    RawMoveStopped,
    /// A failed worker passed the original movement onward.
    RawMoveError,
    /// Reentrant worker borrowing prevented movement processing.
    RawMoveBorrowFailed,
    /// No worker was available to process the original movement.
    RawMoveNoWorker,
    /// A callback panic prevented original movement suppression.
    RawMovePanicked,
    /// An SDK-marked replay movement entered the low-level mouse callback.
    OwnMoveEntered,
    /// An SDK-marked replay movement passed through the callback.
    OwnMovePassed,
    /// The low-level keyboard callback began forwarding without recording key data.
    KeyboardPassthroughEntered,
    /// The low-level keyboard callback completed forwarding without recording key data.
    KeyboardPassthroughCompleted,
    /// The owner began publishing observations to its bounded queue.
    PublicationEntered,
    /// The owner's bounded observation publication attempt completed.
    PublicationCompleted,
    /// The owner began a native input submission call.
    SendInputStarted,
    /// The native input submission call returned, with its accepted count recorded separately.
    SendInputCompleted,
}

impl DiagnosticPhase {
    fn from_raw(value: u32) -> Option<Self> {
        Some(match value {
            1 => Self::RawUpEntered,
            2 => Self::RawUpConsumed,
            3 => Self::RawUpPassed,
            4 => Self::RawUpStopped,
            5 => Self::RawUpError,
            6 => Self::RawUpBorrowFailed,
            7 => Self::RawUpNoWorker,
            8 => Self::RawUpPanicked,
            9 => Self::NegativeHookCode,
            10 => Self::OwnUpPassed,
            11 => Self::ReplayUpQueued,
            12 => Self::ReplayUpSending,
            13 => Self::ReplayUpSent,
            14 => Self::CancelCommand,
            15 => Self::ResolverCommand,
            16 => Self::StopCommand,
            17 => Self::WorkerFailure,
            18 => Self::WorkerFinished,
            19 => Self::RawDownEntered,
            20 => Self::RawDownConsumed,
            21 => Self::RawDownPassed,
            22 => Self::RawDownStopped,
            23 => Self::RawDownError,
            24 => Self::RawDownBorrowFailed,
            25 => Self::RawDownNoWorker,
            26 => Self::RawDownPanicked,
            27 => Self::OwnDownPassed,
            28 => Self::HooksInstalled,
            29 => Self::HooksRemoved,
            30 => Self::CancelCompleted,
            31 => Self::ResolverCompleted,
            32 => Self::StopCompleted,
            33 => Self::HooksRemoving,
            34 => Self::RawMoveEntered,
            35 => Self::RawMoveConsumed,
            36 => Self::RawMovePassed,
            37 => Self::RawMoveStopped,
            38 => Self::RawMoveError,
            39 => Self::RawMoveBorrowFailed,
            40 => Self::RawMoveNoWorker,
            41 => Self::RawMovePanicked,
            42 => Self::OwnMoveEntered,
            43 => Self::OwnMovePassed,
            44 => Self::KeyboardPassthroughEntered,
            45 => Self::KeyboardPassthroughCompleted,
            46 => Self::PublicationEntered,
            47 => Self::PublicationCompleted,
            48 => Self::SendInputStarted,
            49 => Self::SendInputCompleted,
            _ => return None,
        })
    }
}

/// Coordinate-free native input lifetime at one diagnostic boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticState {
    /// Whether the adapter still expects the configured physical release.
    pub physical_down: bool,
    /// Whether the original configured down was consumed.
    pub captured: bool,
    /// Whether a recovered native down is retained or queued.
    pub logical_down: bool,
    /// Host context identity retained for the press, including zero when supplied.
    pub epoch: Option<u64>,
}

/// One copied native diagnostic record. No coordinates, paths or host input text are retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticRecord {
    /// Nonzero monotonically increasing sequence within this recognizer's store.
    pub sequence: u64,
    /// Monotonic nanoseconds from the recognizer's clock origin.
    pub elapsed_ns: u64,
    /// Callback, recovery or lifecycle phase.
    pub phase: DiagnosticPhase,
    /// Related native packet timestamp in milliseconds; zero when not applicable.
    pub event_time_ms: u32,
    /// Wrapping native packet age at callback entry; zero when not applicable.
    /// Callers can supply synthetic timestamps, so this is evidence rather than a deadline verdict.
    pub event_age_ms: u32,
    /// Full callback duration through its final suppression or forwarding decision.
    /// Zero for an entry record or a phase outside the low-level callback.
    pub callback_duration_ns: u64,
    /// Actual native result returned by the low-level callback; zero outside it.
    pub hook_result: i64,
    /// Related native button identifier; zero when no button is available.
    /// A raw packet may refer to a button other than the configured capture button.
    pub button: u32,
    /// Input lifetime before processing, when the worker could be inspected.
    pub before: Option<DiagnosticState>,
    /// Input lifetime after processing, when the worker could be inspected.
    pub after: Option<DiagnosticState>,
    /// Exact native adapter failure associated with this phase, if any.
    pub failure: Option<Error>,
    /// Number of native events in the related planned injection batch.
    pub planned_count: u32,
    /// Number of native events accepted from the related injection batch.
    pub accepted_count: u32,
    /// Context epoch retained by the deferred recovery batch, including zero when supplied.
    /// This can differ from the current input lifetime's epoch; `None` outside replay phases.
    pub delivery_epoch: Option<u64>,
}

impl DiagnosticRecord {
    #[cfg(any(windows, test))]
    pub(crate) fn new(phase: DiagnosticPhase) -> Self {
        Self {
            sequence: 0,
            elapsed_ns: 0,
            phase,
            event_time_ms: 0,
            event_age_ms: 0,
            callback_duration_ns: 0,
            hook_result: 0,
            button: 0,
            before: None,
            after: None,
            failure: None,
            planned_count: 0,
            accepted_count: 0,
            delivery_epoch: None,
        }
    }
}

/// Copied diagnostics that can be obtained without scheduling work on the native owner thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticSnapshot {
    /// Whether future records are currently enabled.
    pub enabled: bool,
    /// Current monotonic nanoseconds from the same origin as the records.
    pub elapsed_ns: u64,
    /// Total record sequences reserved while diagnostics were enabled.
    pub records_written: u64,
    /// Number of sequences older than the store's 64-record retention window.
    pub overwritten: u64,
    /// Reserved sequences in the retained window omitted from this snapshot because
    /// their slots were in flight or concurrently overwritten while being copied.
    pub unavailable: u64,
    /// Committed records in sequence order, at most 64.
    /// A concurrently overwritten or in-flight record may be omitted.
    pub records: Vec<DiagnosticRecord>,
}

/// Read-only diagnostic access retained independently of the recognizer and its worker.
#[derive(Clone)]
pub struct DiagnosticReader {
    store: Arc<Diagnostics>,
}

impl DiagnosticReader {
    #[cfg(any(windows, test))]
    pub(crate) fn new(store: Arc<Diagnostics>) -> Self {
        Self { store }
    }

    /// Copy the current bounded record window without waiting on the native thread.
    /// This allocates only on the calling thread and remains available after native teardown.
    pub fn snapshot(&self) -> DiagnosticSnapshot {
        self.store.snapshot()
    }
}

// Recording has exactly one native owner-thread writer. Readers and the enabling
// thread never record; concurrent or overlapping calls to record are unsupported.
pub(crate) struct Diagnostics {
    origin: Instant,
    enabled: AtomicBool,
    written: AtomicU64,
    slots: [Slot; CAPACITY],
}

impl Diagnostics {
    #[cfg(any(windows, test))]
    pub(crate) fn new(origin: Instant) -> Self {
        Self {
            origin,
            enabled: AtomicBool::new(false),
            written: AtomicU64::new(0),
            slots: [const { Slot::new() }; CAPACITY],
        }
    }

    #[cfg(any(windows, test))]
    pub(crate) fn origin(&self) -> Instant {
        self.origin
    }

    #[cfg(any(windows, test))]
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, SeqCst);
    }

    #[cfg(any(windows, test))]
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(SeqCst)
    }

    /// Reserve and publish on the single native owner thread only. No other thread
    /// may record, and calls must not overlap; snapshots and enable changes may race.
    #[cfg(any(windows, test))]
    pub(crate) fn record(&self, mut record: DiagnosticRecord) {
        if !self.is_enabled() {
            return;
        }
        let previous = self.written.load(SeqCst);
        let Some(sequence) = previous.checked_add(1) else {
            return;
        };
        self.written.store(sequence, SeqCst);
        record.sequence = sequence;
        record.elapsed_ns = self.elapsed_ns();
        self.slots[previous as usize % CAPACITY].store(&record);
    }

    pub(crate) fn snapshot(&self) -> DiagnosticSnapshot {
        let written = self.written.load(SeqCst);
        let retained = written.min(CAPACITY as u64);
        let first = written.saturating_sub(CAPACITY as u64).saturating_add(1);
        let mut records = Vec::with_capacity(retained as usize);
        for sequence in first..=written {
            if let Some(record) = self.slots[(sequence - 1) as usize % CAPACITY].load(sequence) {
                records.push(record);
            }
        }
        DiagnosticSnapshot {
            enabled: self.enabled.load(SeqCst),
            elapsed_ns: self.elapsed_ns(),
            records_written: written,
            overwritten: written.saturating_sub(CAPACITY as u64),
            unavailable: retained - records.len() as u64,
            records,
        }
    }

    fn elapsed_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
}

// Native callbacks and the owner loop are one writer. Every slot field is atomic:
// even a reader racing an overwrite has no non-atomic access or data race. SeqCst
// ordering places invalidation before field writes and validation after field reads.
struct Slot {
    sequence: AtomicU64,
    elapsed_ns: AtomicU64,
    phase: AtomicU32,
    event_time_ms: AtomicU32,
    event_age_ms: AtomicU32,
    callback_duration_ns: AtomicU64,
    hook_result: AtomicU64,
    button: AtomicU32,
    before_flags: AtomicU32,
    before_epoch: AtomicU64,
    after_flags: AtomicU32,
    after_epoch: AtomicU64,
    failure: AtomicU32,
    native_code: AtomicU32,
    failure_sent: AtomicU32,
    failure_expected: AtomicU32,
    planned_count: AtomicU32,
    accepted_count: AtomicU32,
    delivery_epoch_present: AtomicBool,
    delivery_epoch: AtomicU64,
}

impl Slot {
    #[cfg(any(windows, test))]
    const fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            elapsed_ns: AtomicU64::new(0),
            phase: AtomicU32::new(0),
            event_time_ms: AtomicU32::new(0),
            event_age_ms: AtomicU32::new(0),
            callback_duration_ns: AtomicU64::new(0),
            hook_result: AtomicU64::new(0),
            button: AtomicU32::new(0),
            before_flags: AtomicU32::new(0),
            before_epoch: AtomicU64::new(0),
            after_flags: AtomicU32::new(0),
            after_epoch: AtomicU64::new(0),
            failure: AtomicU32::new(0),
            native_code: AtomicU32::new(0),
            failure_sent: AtomicU32::new(0),
            failure_expected: AtomicU32::new(0),
            planned_count: AtomicU32::new(0),
            accepted_count: AtomicU32::new(0),
            delivery_epoch_present: AtomicBool::new(false),
            delivery_epoch: AtomicU64::new(0),
        }
    }

    #[cfg(any(windows, test))]
    fn store(&self, record: &DiagnosticRecord) {
        let (before_flags, before_epoch) = encode_state(record.before);
        let (after_flags, after_epoch) = encode_state(record.after);
        let (failure, native_code, sent, expected) = encode_error(record.failure);
        self.sequence.store(0, SeqCst);
        self.elapsed_ns.store(record.elapsed_ns, SeqCst);
        self.phase.store(record.phase as u32, SeqCst);
        self.event_time_ms.store(record.event_time_ms, SeqCst);
        self.event_age_ms.store(record.event_age_ms, SeqCst);
        self.callback_duration_ns
            .store(record.callback_duration_ns, SeqCst);
        self.hook_result.store(record.hook_result as u64, SeqCst);
        self.button.store(record.button, SeqCst);
        self.before_flags.store(before_flags, SeqCst);
        self.before_epoch.store(before_epoch, SeqCst);
        self.after_flags.store(after_flags, SeqCst);
        self.after_epoch.store(after_epoch, SeqCst);
        self.failure.store(failure, SeqCst);
        self.native_code.store(native_code, SeqCst);
        self.failure_sent.store(sent, SeqCst);
        self.failure_expected.store(expected, SeqCst);
        self.planned_count.store(record.planned_count, SeqCst);
        self.accepted_count.store(record.accepted_count, SeqCst);
        self.delivery_epoch_present
            .store(record.delivery_epoch.is_some(), SeqCst);
        self.delivery_epoch
            .store(record.delivery_epoch.unwrap_or(0), SeqCst);
        self.sequence.store(record.sequence, SeqCst);
    }

    fn load(&self, expected_sequence: u64) -> Option<DiagnosticRecord> {
        if self.sequence.load(SeqCst) != expected_sequence {
            return None;
        }
        let record = DiagnosticRecord {
            sequence: expected_sequence,
            elapsed_ns: self.elapsed_ns.load(SeqCst),
            phase: DiagnosticPhase::from_raw(self.phase.load(SeqCst))?,
            event_time_ms: self.event_time_ms.load(SeqCst),
            event_age_ms: self.event_age_ms.load(SeqCst),
            callback_duration_ns: self.callback_duration_ns.load(SeqCst),
            hook_result: self.hook_result.load(SeqCst) as i64,
            button: self.button.load(SeqCst),
            before: decode_state(
                self.before_flags.load(SeqCst),
                self.before_epoch.load(SeqCst),
            ),
            after: decode_state(self.after_flags.load(SeqCst), self.after_epoch.load(SeqCst)),
            failure: decode_error(
                self.failure.load(SeqCst),
                self.native_code.load(SeqCst),
                self.failure_sent.load(SeqCst),
                self.failure_expected.load(SeqCst),
            ),
            planned_count: self.planned_count.load(SeqCst),
            accepted_count: self.accepted_count.load(SeqCst),
            delivery_epoch: self
                .delivery_epoch_present
                .load(SeqCst)
                .then_some(self.delivery_epoch.load(SeqCst)),
        };
        (self.sequence.load(SeqCst) == expected_sequence).then_some(record)
    }
}

#[cfg(any(windows, test))]
fn encode_state(state: Option<DiagnosticState>) -> (u32, u64) {
    state.map_or((0, 0), |state| {
        (
            8 | u32::from(state.physical_down)
                | (u32::from(state.captured) << 1)
                | (u32::from(state.logical_down) << 2)
                | (u32::from(state.epoch.is_some()) << 4),
            state.epoch.unwrap_or(0),
        )
    })
}

fn decode_state(flags: u32, epoch: u64) -> Option<DiagnosticState> {
    (flags & 8 != 0).then_some(DiagnosticState {
        physical_down: flags & 1 != 0,
        captured: flags & 2 != 0,
        logical_down: flags & 4 != 0,
        epoch: (flags & 16 != 0).then_some(epoch),
    })
}

#[cfg(any(windows, test))]
fn encode_error(error: Option<Error>) -> (u32, u32, u32, u32) {
    match error {
        None => (0, 0, 0, 0),
        Some(Error::InvalidConfiguration) => (1, 0, 0, 0),
        Some(Error::AlreadyRunning) => (2, 0, 0, 0),
        Some(Error::System(code)) => (3, code, 0, 0),
        Some(Error::Replay {
            system,
            sent,
            expected,
        }) => (4, system, sent, expected),
        Some(Error::ConsumerLagged) => (5, 0, 0, 0),
        Some(Error::ReplayTargetChanged) => (6, 0, 0, 0),
        Some(Error::WorkerStopped) => (7, 0, 0, 0),
    }
}

fn decode_error(kind: u32, native_code: u32, sent: u32, expected: u32) -> Option<Error> {
    match kind {
        1 => Some(Error::InvalidConfiguration),
        2 => Some(Error::AlreadyRunning),
        3 => Some(Error::System(native_code)),
        4 => Some(Error::Replay {
            system: native_code,
            sent,
            expected,
        }),
        5 => Some(Error::ConsumerLagged),
        6 => Some(Error::ReplayTargetChanged),
        7 => Some(Error::WorkerStopped),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disabled_store_records_nothing_and_retains_history_when_disabled() {
        let origin = Instant::now();
        let store = Diagnostics::new(origin);
        assert_eq!(store.origin(), origin);
        store.record(DiagnosticRecord::new(DiagnosticPhase::RawUpEntered));
        let snapshot = store.snapshot();
        assert!(!snapshot.enabled);
        assert_eq!(snapshot.records_written, 0);
        assert_eq!(snapshot.unavailable, 0);
        assert!(snapshot.records.is_empty());
        store.set_enabled(true);
        store.record(DiagnosticRecord::new(DiagnosticPhase::RawUpConsumed));
        store.set_enabled(false);
        store.record(DiagnosticRecord::new(DiagnosticPhase::RawUpPassed));
        let snapshot = store.snapshot();
        assert!(!snapshot.enabled);
        assert_eq!(snapshot.records_written, 1);
        assert_eq!(snapshot.unavailable, 0);
        assert_eq!(snapshot.records[0].phase, DiagnosticPhase::RawUpConsumed);
        assert!(snapshot.elapsed_ns >= snapshot.records[0].elapsed_ns);
    }

    #[test]
    fn bounded_history_preserves_order_states_zero_epoch_and_every_failure() {
        let store = Diagnostics::new(Instant::now());
        store.set_enabled(true);
        for _ in 0..CAPACITY + 3 {
            store.record(DiagnosticRecord::new(DiagnosticPhase::RawUpEntered));
        }
        let errors = [
            Error::InvalidConfiguration,
            Error::AlreadyRunning,
            Error::System(u32::MAX),
            Error::Replay {
                system: 5,
                sent: 3,
                expected: 7,
            },
            Error::ConsumerLagged,
            Error::ReplayTargetChanged,
            Error::WorkerStopped,
        ];
        for (index, failure) in errors.iter().copied().enumerate() {
            let mut record = DiagnosticRecord::new(DiagnosticPhase::WorkerFailure);
            record.before = Some(DiagnosticState {
                physical_down: true,
                captured: true,
                logical_down: false,
                epoch: Some(0),
            });
            record.after = Some(DiagnosticState {
                physical_down: false,
                captured: false,
                logical_down: true,
                epoch: None,
            });
            record.failure = Some(failure);
            record.event_time_ms = index as u32;
            record.event_age_ms = 99;
            record.callback_duration_ns = u64::MAX;
            record.hook_result = i64::MIN;
            record.button = 4;
            record.planned_count = 7;
            record.accepted_count = 3;
            store.record(record);
        }
        let snapshot = store.snapshot();
        assert_eq!(
            snapshot.records_written,
            (CAPACITY + 3 + errors.len()) as u64
        );
        assert_eq!(snapshot.overwritten, (3 + errors.len()) as u64);
        assert_eq!(snapshot.unavailable, 0);
        assert_eq!(snapshot.records.len(), CAPACITY);
        assert!(
            snapshot
                .records
                .windows(2)
                .all(|pair| pair[0].sequence + 1 == pair[1].sequence)
        );
        for (record, failure) in snapshot.records[CAPACITY - errors.len()..]
            .iter()
            .zip(errors)
        {
            assert_eq!(record.failure, Some(failure));
            assert_eq!(record.before.unwrap().epoch, Some(0));
            assert_eq!(record.after.unwrap().epoch, None);
            assert!(record.before.unwrap().physical_down && record.before.unwrap().captured);
            assert!(!record.before.unwrap().logical_down);
            assert!(!record.after.unwrap().physical_down && !record.after.unwrap().captured);
            assert!(record.after.unwrap().logical_down);
            assert_eq!(record.event_age_ms, 99);
            assert_eq!(record.callback_duration_ns, u64::MAX);
            assert_eq!(record.hook_result, i64::MIN);
            assert_eq!(record.button, 4);
            assert_eq!(record.planned_count, 7);
            assert_eq!(record.accepted_count, 3);
        }
    }

    #[test]
    fn concurrent_reader_never_observes_a_mixed_overwrite() {
        let store = Arc::new(Diagnostics::new(Instant::now()));
        store.set_enabled(true);
        let reader = DiagnosticReader::new(Arc::clone(&store));
        let done = Arc::new(AtomicBool::new(false));
        let complete = Arc::clone(&done);
        let writer = std::thread::spawn(move || {
            for sequence in 1..=10_000 {
                let mut record = DiagnosticRecord::new(DiagnosticPhase::ReplayUpSent);
                record.event_time_ms = sequence;
                record.event_age_ms = sequence ^ 0x1234;
                record.callback_duration_ns = u64::from(sequence) * 123;
                record.hook_result = -i64::from(sequence);
                record.before = Some(DiagnosticState {
                    physical_down: true,
                    captured: false,
                    logical_down: true,
                    epoch: Some(u64::from(sequence)),
                });
                record.failure = Some(Error::Replay {
                    system: sequence ^ 0x4321,
                    sent: sequence,
                    expected: sequence + 1,
                });
                record.planned_count = sequence + 1;
                record.accepted_count = sequence;
                record.delivery_epoch = Some(u64::MAX - u64::from(sequence));
                store.record(record);
            }
            complete.store(true, SeqCst);
        });
        let verify = |snapshot: DiagnosticSnapshot| {
            assert!(snapshot.records.len() <= CAPACITY);
            assert_eq!(
                snapshot.unavailable + snapshot.records.len() as u64,
                snapshot.records_written.min(CAPACITY as u64)
            );
            assert!(
                snapshot
                    .records
                    .windows(2)
                    .all(|pair| pair[0].sequence < pair[1].sequence)
            );
            for record in snapshot.records {
                let sequence = record.sequence as u32;
                assert_eq!(record.phase, DiagnosticPhase::ReplayUpSent);
                assert_eq!(record.event_time_ms, sequence);
                assert_eq!(record.event_age_ms, sequence ^ 0x1234);
                assert_eq!(record.callback_duration_ns, u64::from(sequence) * 123);
                assert_eq!(record.hook_result, -i64::from(sequence));
                assert_eq!(record.before.unwrap().epoch, Some(record.sequence));
                assert_eq!(
                    record.failure,
                    Some(Error::Replay {
                        system: sequence ^ 0x4321,
                        sent: sequence,
                        expected: sequence + 1,
                    })
                );
                assert_eq!(record.planned_count, sequence + 1);
                assert_eq!(record.accepted_count, sequence);
                assert_eq!(record.delivery_epoch, Some(u64::MAX - u64::from(sequence)));
                assert!(record.elapsed_ns <= snapshot.elapsed_ns);
            }
        };
        while !done.load(SeqCst) {
            verify(reader.snapshot());
        }
        writer.join().unwrap();
        let final_snapshot = reader.snapshot();
        assert_eq!(final_snapshot.records_written, 10_000);
        assert_eq!(final_snapshot.unavailable, 0);
        assert_eq!(final_snapshot.records.len(), CAPACITY);
        verify(final_snapshot);
    }

    #[test]
    fn readonly_reader_outlives_the_original_owner() {
        let store = Arc::new(Diagnostics::new(Instant::now()));
        store.set_enabled(true);
        let reader = DiagnosticReader::new(Arc::clone(&store));
        store.record(DiagnosticRecord::new(DiagnosticPhase::WorkerFinished));
        drop(store);
        let clone = reader.clone();
        drop(reader);
        let snapshot = clone.snapshot();
        assert_eq!(snapshot.records_written, 1);
        assert_eq!(snapshot.unavailable, 0);
        assert_eq!(snapshot.records[0].phase, DiagnosticPhase::WorkerFinished);
    }

    #[test]
    fn delivery_epoch_preserves_absent_zero_and_maximum_separately_from_current_state() {
        let store = Diagnostics::new(Instant::now());
        store.set_enabled(true);
        let delivery_epochs = [None, Some(0), Some(u64::MAX)];
        for delivery_epoch in delivery_epochs {
            let mut record = DiagnosticRecord::new(DiagnosticPhase::ReplayUpSending);
            record.before = Some(DiagnosticState {
                physical_down: true,
                captured: true,
                logical_down: false,
                epoch: Some(41),
            });
            record.after = Some(DiagnosticState {
                physical_down: true,
                captured: true,
                logical_down: false,
                epoch: Some(42),
            });
            record.delivery_epoch = delivery_epoch;
            store.record(record);
        }
        let snapshot = store.snapshot();
        assert_eq!(snapshot.records.len(), delivery_epochs.len());
        for (record, delivery_epoch) in snapshot.records.iter().zip(delivery_epochs) {
            assert_eq!(record.delivery_epoch, delivery_epoch);
            assert_eq!(record.before.unwrap().epoch, Some(41));
            assert_eq!(record.after.unwrap().epoch, Some(42));
        }
    }

    #[test]
    fn reserved_record_is_unavailable_until_publication_and_does_not_wrap_at_limit() {
        let store = Diagnostics::new(Instant::now());
        store.set_enabled(true);
        // Pause the single writer between reservation and publication.
        store.written.store(1, SeqCst);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.records_written, 1);
        assert_eq!(snapshot.overwritten, 0);
        assert_eq!(snapshot.unavailable, 1);
        assert!(snapshot.records.is_empty());

        let mut record = DiagnosticRecord::new(DiagnosticPhase::RawUpEntered);
        record.sequence = 1;
        store.slots[0].store(&record);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.unavailable, 0);
        assert_eq!(snapshot.records, [record]);

        store.written.store(u64::MAX, SeqCst);
        store.record(DiagnosticRecord::new(DiagnosticPhase::RawUpConsumed));
        let snapshot = store.snapshot();
        assert_eq!(snapshot.records_written, u64::MAX);
        assert_eq!(snapshot.overwritten, u64::MAX - CAPACITY as u64);
        assert_eq!(snapshot.unavailable, CAPACITY as u64);
        assert!(snapshot.records.is_empty());
    }

    #[test]
    fn all_callback_recovery_and_lifecycle_phases_survive_the_atomic_store() {
        let store = Diagnostics::new(Instant::now());
        store.set_enabled(true);
        let phases = [
            DiagnosticPhase::RawUpEntered,
            DiagnosticPhase::RawUpConsumed,
            DiagnosticPhase::RawUpPassed,
            DiagnosticPhase::RawUpStopped,
            DiagnosticPhase::RawUpError,
            DiagnosticPhase::RawUpBorrowFailed,
            DiagnosticPhase::RawUpNoWorker,
            DiagnosticPhase::RawUpPanicked,
            DiagnosticPhase::NegativeHookCode,
            DiagnosticPhase::OwnUpPassed,
            DiagnosticPhase::ReplayUpQueued,
            DiagnosticPhase::ReplayUpSending,
            DiagnosticPhase::ReplayUpSent,
            DiagnosticPhase::CancelCommand,
            DiagnosticPhase::ResolverCommand,
            DiagnosticPhase::StopCommand,
            DiagnosticPhase::WorkerFailure,
            DiagnosticPhase::WorkerFinished,
            DiagnosticPhase::RawDownEntered,
            DiagnosticPhase::RawDownConsumed,
            DiagnosticPhase::RawDownPassed,
            DiagnosticPhase::RawDownStopped,
            DiagnosticPhase::RawDownError,
            DiagnosticPhase::RawDownBorrowFailed,
            DiagnosticPhase::RawDownNoWorker,
            DiagnosticPhase::RawDownPanicked,
            DiagnosticPhase::OwnDownPassed,
            DiagnosticPhase::HooksInstalled,
            DiagnosticPhase::HooksRemoved,
            DiagnosticPhase::CancelCompleted,
            DiagnosticPhase::ResolverCompleted,
            DiagnosticPhase::StopCompleted,
            DiagnosticPhase::HooksRemoving,
            DiagnosticPhase::RawMoveEntered,
            DiagnosticPhase::RawMoveConsumed,
            DiagnosticPhase::RawMovePassed,
            DiagnosticPhase::RawMoveStopped,
            DiagnosticPhase::RawMoveError,
            DiagnosticPhase::RawMoveBorrowFailed,
            DiagnosticPhase::RawMoveNoWorker,
            DiagnosticPhase::RawMovePanicked,
            DiagnosticPhase::OwnMoveEntered,
            DiagnosticPhase::OwnMovePassed,
            DiagnosticPhase::KeyboardPassthroughEntered,
            DiagnosticPhase::KeyboardPassthroughCompleted,
            DiagnosticPhase::PublicationEntered,
            DiagnosticPhase::PublicationCompleted,
            DiagnosticPhase::SendInputStarted,
            DiagnosticPhase::SendInputCompleted,
        ];
        for phase in phases {
            store.record(DiagnosticRecord::new(phase));
        }
        assert_eq!(
            store
                .snapshot()
                .records
                .iter()
                .map(|record| record.phase)
                .collect::<Vec<_>>(),
            phases
        );
    }
}
