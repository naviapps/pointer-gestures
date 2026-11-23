use crate::ReadResult;
use crate::diagnostics::Diagnostics;
use crate::notification::{self, ObservationReceiver, ObservationSender};
use crate::replay::{Poll, Replay, Signal, SubmitError};
use crate::session::{Delivery, Injection, MODIFIER_KEYS, MouseAction, Pointer, Session, absolute};
use crate::{
    ContextOutput, ContextReadResult, DiagnosticPhase, DiagnosticReader, DiagnosticRecord,
    DiagnosticSnapshot, Error, Policy, PressContext, SessionContext,
};
use pointer_gesture_recognition::{Configuration, Event, Matcher};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    mem::size_of,
    os::windows::io::{AsRawHandle, OwnedHandle},
    ptr::null_mut,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    System::{LibraryLoader::*, SystemInformation::GetTickCount, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const MARKER: usize = 0x50475257;
const OBSERVATIONS: usize = 256;
const PENDING_DELIVERIES: usize = OBSERVATIONS + 2;
static ACTIVE: AtomicBool = AtomicBool::new(false);
thread_local! { static HOOK_FAILED: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static WORKER: RefCell<Option<Worker>> = const { RefCell::new(None) }; }
// This survives run()/unwind until the outer owner has unhooked and recovered input.
thread_local! { static REPLAY: RefCell<Option<NativeReplay>> = const { RefCell::new(None) }; }
// Separate from WORKER so reentrant borrow failures can still be recorded. Only
// this owner thread writes records; readers and enablement never publish records.
thread_local! { static DIAGNOSTICS: RefCell<Option<Arc<Diagnostics>>> = const { RefCell::new(None) }; }

struct EventHandle(usize);
// Windows event handles may be signalled from any thread. Ownership closes the
// handle after both the recognizer and its joined worker have released it.
impl EventHandle {
    fn raw(&self) -> HANDLE {
        self.0 as HANDLE
    }
}
impl Signal for EventHandle {
    fn signal(&self) -> bool {
        unsafe { SetEvent(self.raw()) != 0 }
    }
}
impl Drop for EventHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.raw());
        }
    }
}

enum Operation {
    Matcher(Matcher),
    Policy(Policy),
    ContextResolver(ContextResolver),
    Recording(bool),
    Tuning(Configuration),
    Cancel,
    Stop,
}
type ContextResolver = Box<dyn FnMut(PressContext) -> Result<SessionContext, Error> + Send>;
struct Command {
    operation: Operation,
    reply: mpsc::SyncSender<Result<(), Error>>,
}

/// One desktop recognizer with a dedicated message-loop thread.
///
/// Drain observations promptly. Only Match and Publish effects remain in returned
/// frames: native input recovery has already been delivered. A full 256-frame
/// queue stops capture rather than blocking a Windows hook; reserved slots retain
/// the overflow-triggering frame and final shutdown observation.
/// Explicit stop and Drop both unhook and recover held input before joining.
pub struct Recognizer {
    commands: mpsc::Sender<Command>,
    wake: Arc<EventHandle>,
    outputs: ObservationReceiver,
    worker: Option<JoinHandle<()>>,
    origin: Instant,
    diagnostics: Arc<Diagnostics>,
}
impl Recognizer {
    /// Install mouse/Escape hooks for one configured button (0 through 4).
    /// Distances use physical pixels; maximum_points must not exceed 4096.
    /// Requires an accessible interactive desktop; no elevation is requested.
    pub fn start(config: Configuration, matcher: &Matcher) -> Result<Self, Error> {
        Self::start_inner(config, matcher, None)
    }
    /// Resolve matcher, policy and an opaque host epoch at each eligible button-down.
    /// The resolver runs synchronously inside the input hook. It must finish promptly:
    /// use bounded native identity queries and immutable lookups, never blocking locks,
    /// UI, IPC, filesystem/network work, effects or recognizer/overlay methods.
    /// Panics and invalid policies stop capture before suppressing the new press.
    /// Matching and epoch snapshots remain attached to the press through completion.
    pub fn start_with_context(
        config: Configuration,
        mut resolver: impl FnMut(PressContext) -> SessionContext + Send + 'static,
    ) -> Result<Self, Error> {
        Self::try_start_with_context(config, move |press| Ok(resolver(press)))
    }
    /// Start capture with a fallible resolver. A resolution error stops capture
    /// before suppressing the new original press.
    pub fn try_start_with_context(
        config: Configuration,
        resolver: impl FnMut(PressContext) -> Result<SessionContext, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        Self::start_inner(config, &Matcher::default(), Some(Box::new(resolver)))
    }
    fn start_inner(
        config: Configuration,
        matcher: &Matcher,
        resolver: Option<ContextResolver>,
    ) -> Result<Self, Error> {
        let session = Session::new(config, matcher)?;
        if ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::AlreadyRunning);
        }
        let handle = unsafe { CreateEventW(null_mut(), 1, 0, std::ptr::null()) };
        if handle.is_null() {
            ACTIVE.store(false, Ordering::Release);
            return Err(Error::System(unsafe { GetLastError() }));
        }
        let wake = Arc::new(EventHandle(handle as usize));
        let worker_wake = Arc::clone(&wake);
        let (commands, receiver) = mpsc::channel();
        let (sender, outputs) = match notification::channel(OBSERVATIONS) {
            Ok(channel) => channel,
            Err(error) => {
                ACTIVE.store(false, Ordering::Release);
                return Err(error);
            }
        };
        let worker_outputs = sender.clone();
        let (started, ready) = mpsc::sync_channel(1);
        let origin = Instant::now();
        let diagnostics = Arc::new(Diagnostics::new(origin));
        let worker_diagnostics = Arc::clone(&diagnostics);
        let worker = match thread::Builder::new()
            .name("pointer-gesture-input".into())
            .spawn(move || {
                DIAGNOSTICS.with(|slot| {
                    *slot.borrow_mut() = Some(Arc::clone(&worker_diagnostics));
                });
                let mut outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        session,
                        resolver,
                        receiver,
                        sender,
                        worker_wake,
                        Arc::clone(&worker_diagnostics),
                        started,
                    )
                }))
                .unwrap_or(Err(Error::WorkerStopped));
                // A panic in the pumping owner must not lose the native prefix
                // that its injector accepted while Session processed later input.
                if outcome.is_err()
                    && WORKER.with(|slot| {
                        slot.borrow().as_ref().is_some_and(|worker| {
                            worker.in_flight.is_some() || worker.closing_submission.is_some()
                        })
                    })
                {
                    WORKER.with(|slot| {
                        if let Some(worker) = slot.borrow_mut().as_mut() {
                            worker.error.get_or_insert(Error::WorkerStopped);
                            worker.record_failure(Error::WorkerStopped);
                            worker.detach_hooks();
                        }
                    });
                    REPLAY.with(|slot| {
                        if let Some(replay) = slot.borrow_mut().as_mut() {
                            replay.healthy = false;
                            let _ = replay.worker.close_and_join();
                        }
                    });
                    let close = WORKER.with(|slot| {
                        if let Some(worker) = slot.borrow_mut().as_mut() {
                            let close = worker.reconcile_in_flight();
                            if let Some(accepted) = worker.closing_submission.take() {
                                worker
                                    .session
                                    .replay_close_completed(accepted.load(Ordering::Acquire) == 1);
                            }
                            worker.pending.clear();
                            close
                        } else {
                            None
                        }
                    });
                    complete_replay_close(close);
                }
                // Teardown also runs after an unexpected native-loop panic.
                // Teardown itself invokes no host callbacks.
                WORKER.with(|slot| {
                    if let Some(worker) = slot.borrow_mut().as_mut() {
                        if let Err(error) = outcome {
                            worker.record_failure(error);
                        }
                        worker.finish();
                    } else if let Err(error) = outcome {
                        let mut record = DiagnosticRecord::new(DiagnosticPhase::WorkerFailure);
                        record.failure = Some(error);
                        worker_diagnostics.record(record);
                    }
                });
                flush();
                WORKER.with(|slot| {
                    if let Some(worker) = slot.borrow_mut().as_mut() {
                        worker.detach_hooks();
                    }
                });
                // finish() removed the hooks before any blocking injector join.
                REPLAY.with(|slot| {
                    if let Some(mut replay) = slot.borrow_mut().take()
                        && replay.worker.close_and_join().is_err()
                    {
                        outcome = outcome.and(Err(Error::WorkerStopped));
                        WORKER.with(|worker| {
                            if let Some(worker) = worker.borrow_mut().as_mut() {
                                worker.error.get_or_insert(Error::WorkerStopped);
                                worker.record_failure(Error::WorkerStopped);
                            }
                        });
                    }
                });
                WORKER.with(|slot| {
                    slot.borrow_mut().take();
                });
                worker_diagnostics.record(DiagnosticRecord::new(DiagnosticPhase::WorkerFinished));
                DIAGNOSTICS.with(|slot| {
                    slot.borrow_mut().take();
                });
                worker_outputs.finish(outcome);
                ACTIVE.store(false, Ordering::Release);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                ACTIVE.store(false, Ordering::Release);
                return Err(Error::System(error.raw_os_error().unwrap_or(0) as u32));
            }
        };
        match ready.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                wake,
                outputs,

                worker: Some(worker),
                origin,
                diagnostics,
            }),
            status => {
                let _ = worker.join();
                Err(status.unwrap_or(Err(Error::WorkerStopped)).unwrap_err())
            }
        }
    }

    /// Copy the next owned observation without blocking. Recovery effects are
    /// absent; only the host's Match/Publish work remains. Drain until Pending or Completed.
    pub fn try_read(&self) -> Result<ReadResult, Error> {
        self.outputs.try_read()
    }
    /// Drain the same queue as `try_read`, retaining the host epoch of each frame.
    /// Do not alternate readers when the host needs every frame's context identity.
    pub fn try_read_with_context(&self) -> Result<ContextReadResult, Error> {
        self.outputs.try_read_with_context()
    }
    /// Duplicate an owned wait-only notification handle. It signals queued frames,
    /// terminal errors and completion, and remains valid after this recognizer drops.
    /// Only `try_read` resets readiness; drain until empty before waiting again.
    pub fn duplicate_notification_handle(&self) -> Result<OwnedHandle, Error> {
        self.outputs.duplicate_handle()
    }
    /// Monotonic nanoseconds in the same clock domain as observation deadlines.
    pub fn now_ns(&self) -> u64 {
        elapsed(self.origin)
    }
    /// Enable bounded coordinate-free native diagnostics. Disabled by default.
    /// This atomic switch neither schedules work nor clears retained records.
    pub fn set_diagnostics_enabled(&self, enabled: bool) {
        self.diagnostics.set_enabled(enabled);
    }
    /// Copy at most 64 retained records without waiting for the native worker.
    /// Recording uses fixed preallocated atomic slots; copying allocates here.
    pub fn diagnostics(&self) -> DiagnosticSnapshot {
        self.diagnostics.snapshot()
    }
    /// Retain readonly diagnostics independently of the worker and recognizer.
    /// Snapshots remain available during stalled callbacks and after teardown.
    pub fn diagnostic_reader(&self) -> DiagnosticReader {
        DiagnosticReader::new(Arc::clone(&self.diagnostics))
    }
    /// Replace the future matcher; active input retains the captured snapshot.
    pub fn use_matcher(&mut self, matcher: &Matcher) -> Result<(), Error> {
        self.command(Operation::Matcher(matcher.clone()))
    }
    /// Change future enablement and required Shift/Control/Alt/Windows groups.
    /// A context resolver replaces these values at the next eligible press.
    pub fn set_policy(&mut self, policy: Policy) -> Result<(), Error> {
        if policy.required_modifiers > 15 {
            return Err(Error::InvalidConfiguration);
        }
        self.command(Operation::Policy(policy))
    }
    /// Replace the resolver for future presses on the adapter owner thread.
    /// The new closure can own a freshly published immutable host snapshot without
    /// shared locks. Active/queued frames retain their captured matcher and epoch.
    /// This has the same hook-side execution contract as `start_with_context`.
    pub fn set_context_resolver(
        &mut self,
        mut resolver: impl FnMut(PressContext) -> SessionContext + Send + 'static,
    ) -> Result<(), Error> {
        self.set_fallible_context_resolver(move |press| Ok(resolver(press)))
    }
    /// Replace the resolver with one that can return a terminal error.
    pub fn set_fallible_context_resolver(
        &mut self,
        resolver: impl FnMut(PressContext) -> Result<SessionContext, Error> + Send + 'static,
    ) -> Result<(), Error> {
        self.command(Operation::ContextResolver(Box::new(resolver)))
    }
    /// Set recording mode for future sessions.
    pub fn set_recording(&mut self, value: bool) -> Result<(), Error> {
        self.command(Operation::Recording(value))
    }
    /// Apply mutable tuning, retaining the configured button and empty policy.
    pub fn update_tuning(&mut self, config: Configuration) -> Result<(), Error> {
        if config.maximum_points > 4096 {
            return Err(Error::InvalidConfiguration);
        }
        self.command(Operation::Tuning(config))
    }
    /// Cancel current recognition. A pending held click returns to its native
    /// pressed state so the later real button-up stays balanced.
    pub fn cancel(&mut self) -> Result<(), Error> {
        self.command(Operation::Cancel)
    }
    /// Unhook, recover consumed input and join the message loop. Repeated stops
    /// report the same terminal outcome without replaying input twice. A retained
    /// press whose original recipient changed reports ReplayTargetChanged.
    pub fn stop(&mut self) -> Result<(), Error> {
        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                let _ = self.command(Operation::Stop);
            }
            worker.join().map_err(|_| Error::WorkerStopped)?;
        }
        self.outputs.outcome().unwrap_or(Err(Error::WorkerStopped))
    }
    fn command(&self, operation: Operation) -> Result<(), Error> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.commands
            .send(Command { operation, reply })
            .map_err(|_| Error::WorkerStopped)?;
        // A manual-reset event is paired with queue draining on the owner thread.
        // Reset happens before draining, so a concurrent command cannot lose wakeup.
        unsafe {
            SetEvent(self.wake.raw());
        }
        receive.recv().unwrap_or(Err(Error::WorkerStopped))
    }
}
impl Drop for Recognizer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn elapsed(origin: Instant) -> u64 {
    origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

struct Worker {
    session: Session,
    resolver: Option<ContextResolver>,
    wake: Arc<EventHandle>,
    origin: Instant,
    diagnostics: Arc<Diagnostics>,
    mouse: HHOOK,
    keyboard: HHOOK,
    outputs: ObservationSender,
    pending: VecDeque<(Delivery, Option<Target>)>,
    in_flight: Option<InFlight>,
    closing_replay: bool,
    closing_submission: Option<Arc<AtomicU32>>,
    target: Option<Target>,
    last_output: Option<ContextOutput>,
    deadline: Option<u64>,
    error: Option<Error>,
    failure_recorded: bool,
    stopped: bool,
    finishing: bool,
    shutdown_attempt: Option<u64>,
    escape_down: bool,
    integrity: u32,
}
struct InFlight {
    press_id: u64,
    release: Option<Pointer>,
    release_consumed: bool,
    target: Option<Target>,
    epoch: Option<u64>,
    injections: Vec<Injection>,
    logical_before: bool,
    accepted: Arc<AtomicU32>,
}
impl Worker {
    fn recovery_pending(&self) -> bool {
        self.closing_replay
            || self
                .in_flight
                .as_ref()
                .is_some_and(|batch| !batch.injections.is_empty())
            || self
                .pending
                .iter()
                .any(|(delivery, _)| !delivery.injections.is_empty())
    }
    fn reconcile_in_flight(&mut self) -> Option<(Injection, Option<Target>, Option<u64>)> {
        if let Some(batch) = self.in_flight.take() {
            let accepted = batch.accepted.load(Ordering::Acquire) as usize;
            let close = self
                .session
                .reconcile_failed(
                    batch.press_id,
                    batch.logical_before,
                    &batch.injections[..accepted.min(batch.injections.len())],
                    batch.release,
                )
                .map(|up| (up, batch.target, batch.epoch));
            // If a callback panicked before suppressing the physical up, its
            // native ordering is unknown. Keep the failed-close guard rather
            // than synthesize a second up or restore a different press's down.
            return if batch.release.is_some() && !batch.release_consumed {
                None
            } else {
                close
            };
        }
        None
    }
    fn defer(&mut self, delivery: Delivery) -> bool {
        let consume = delivery.consume;
        if let Some(batch) = self.in_flight.as_mut()
            && batch.press_id == delivery.press_id
            && delivery.release.is_some()
        {
            if batch.release.is_none() {
                batch.release = delivery.release;
            }
            batch.release_consumed = delivery.consume;
        }
        if self.pending.len() >= OBSERVATIONS {
            self.error.get_or_insert(Error::ConsumerLagged);
        }
        // Two closing reservations retain the overflow-triggering frame and
        // the original held release while an earlier batch is still in flight.
        // Once failed, mouse()/keyboard_hook() admit no further ordinary work.
        if self.pending.len() >= PENDING_DELIVERIES {
            self.error.get_or_insert(Error::ConsumerLagged);
            // Fatal backlog failure already discards unsent later work. Native
            // lifetime metadata has still advanced; reconciliation/Stop recovers
            // the current held press after closing the actually accepted prefix.
            return consume;
        }
        if self.diagnostics.is_enabled()
            && let Some(time) = replay_up_time(&delivery.injections)
        {
            let mut record = self.record(DiagnosticPhase::ReplayUpQueued);
            record.event_time_ms = time;
            record.delivery_epoch = delivery.epoch;
            record.planned_count = delivery.injections.len() as u32;
            self.diagnostics.record(record);
        }
        // SendInput must run after the low-level hook returns its suppression
        // decision. Injecting recursively inside the hook can release original
        // input before Windows has applied that decision.
        self.pending.push_back((delivery, self.target));
        unsafe {
            SetEvent(self.wake.raw());
        }
        consume
    }
    fn apply(&mut self, output: ContextOutput) {
        self.deadline = output.output.deadline_ns;
        if self.last_output.as_ref() == Some(&output) {
            return;
        }
        self.last_output = Some(output.clone());
        let record = self.diagnostics.is_enabled().then(|| {
            let mut record = self.record(DiagnosticPhase::PublicationEntered);
            record.delivery_epoch = output.epoch;
            record
        });
        if let Some(record) = &record {
            self.diagnostics.record(record.clone());
        }
        if let Err(error) = self.outputs.send(output) {
            self.error.get_or_insert(error);
        }
        if let Some(mut record) = record {
            record.phase = DiagnosticPhase::PublicationCompleted;
            record.after = Some(self.session.diagnostic_state());
            record.failure = self.error;
            self.diagnostics.record(record);
        }
    }
    fn finish(&mut self) {
        if self.finishing && self.shutdown_attempt == Some(self.session.current_press_id()) {
            return;
        }
        let already_finishing = self.finishing;
        self.finishing = true;
        self.session.last.modifiers = modifiers();
        let delivery = self.session.shutdown(elapsed(self.origin));
        if !delivery.injections.is_empty() {
            // Each physical lifetime gets at most one final native attempt.
            // Reconciliation may expose a newer held press while we pump it.
            self.shutdown_attempt = Some(delivery.press_id);
        }
        if !already_finishing || !delivery.injections.is_empty() {
            self.defer(delivery);
        }
    }
    fn detach_hooks(&mut self) {
        self.stopped = true;
        if self.mouse.is_null() && self.keyboard.is_null() {
            return;
        }
        self.diagnostics
            .record(self.record(DiagnosticPhase::HooksRemoving));
        // Normal stop drains recovery while pumping before disconnecting. Fatal
        // wait/worker failures disconnect before any blocking fallback or join.
        unsafe {
            if !self.mouse.is_null() {
                UnhookWindowsHookEx(self.mouse);
                self.mouse = null_mut();
            }
            if !self.keyboard.is_null() {
                UnhookWindowsHookEx(self.keyboard);
                self.keyboard = null_mut();
            }
        }
        self.diagnostics
            .record(self.record(DiagnosticPhase::HooksRemoved));
    }
    fn record(&self, phase: DiagnosticPhase) -> DiagnosticRecord {
        let mut record = DiagnosticRecord::new(phase);
        record.button = self.session.button;
        record.before = Some(self.session.diagnostic_state());
        record.after = record.before;
        record.failure = self.error;
        record
    }
    fn record_failure(&mut self, error: Error) {
        if !self.failure_recorded && self.diagnostics.is_enabled() {
            let mut record = self.record(DiagnosticPhase::WorkerFailure);
            record.failure = Some(error);
            self.diagnostics.record(record);
            self.failure_recorded = true;
        }
    }
    fn mouse(&mut self, message: u32, data: MSLLHOOKSTRUCT) -> bool {
        if self.stopped {
            return false;
        }
        let pointer = Pointer {
            point: pointer_gesture_recognition::Point {
                x: f64::from(data.pt.x),
                y: f64::from(data.pt.y),
            },
            modifiers: modifiers(),
            time: data.time,
        };
        if self.session.physical_down
            && mouse_action(message, data.mouseData) == Some((self.session.button, MouseAction::Up))
            && let Some(batch) = self.in_flight.as_mut()
            && batch.release.is_none()
        {
            // Retain physical-release metadata even if Session processing panics.
            // defer() commits whether the original event was actually consumed.
            batch.release = Some(pointer);
            batch.release_consumed = false;
        }
        if self.error.is_some() || self.finishing {
            // A failed in-flight batch still needs the original physical up.
            // Pass cursor motion, quarantine repeated downs, and retain one
            // closing release; accepted-prefix reconciliation runs before stop.
            if self.session.physical_down
                && let Some((button, action)) = mouse_action(message, data.mouseData)
                && button == self.session.button
            {
                if action == MouseAction::Down {
                    return true;
                }
                if action == MouseAction::Up {
                    let delivery = self.session.pointer(action, pointer, elapsed(self.origin));
                    return self.defer(delivery);
                }
            }
            if !self.session.physical_down
                && self.recovery_pending()
                && let Some((button, MouseAction::Down)) = mouse_action(message, data.mouseData)
                && button == self.session.button
                && let Some(target) = Target::at(data.pt, self.integrity)
            {
                self.target = Some(target);
                self.session.policy.enabled = false;
                let mut delivery =
                    self.session
                        .pointer(MouseAction::Down, pointer, elapsed(self.origin));
                self.session.defer_passthrough_down(&mut delivery);
                return self.defer(delivery);
            }
            return false;
        }
        let event = mouse_action(message, data.mouseData);
        if let Some((button, action)) = event {
            if button != self.session.button {
                self.interrupt(pointer);
                return false;
            }
            if action == MouseAction::Down {
                if self.session.physical_down {
                    // Retain the original recipient and host context until up.
                    // Resolving a new context here would relabel old recovery.
                    let delivery = self.session.pointer(action, pointer, elapsed(self.origin));
                    return self.defer(delivery);
                }
                let Some(target) = Target::at(data.pt, self.integrity) else {
                    return false;
                };
                if let Some(resolver) = &mut self.resolver {
                    let mut foreground_process = 0;
                    unsafe {
                        GetWindowThreadProcessId(GetForegroundWindow(), &mut foreground_process);
                    }
                    let context = crate::context::resolve(
                        resolver,
                        PressContext {
                            point: pointer.point,
                            recipient_process_id: target.process,
                            foreground_process_id: (foreground_process != 0)
                                .then_some(foreground_process),
                        },
                    );
                    if let Err(error) =
                        context.and_then(|context| self.session.use_context(context))
                    {
                        self.error.get_or_insert(error);
                        return false;
                    }
                }
                self.target = Some(target);
            }
            let mut delivery = self.session.pointer(action, pointer, elapsed(self.origin));
            if action == MouseAction::Down && !delivery.consume && self.recovery_pending() {
                self.session.defer_passthrough_down(&mut delivery);
            }
            return self.defer(delivery);
        }
        if message == WM_MOUSEMOVE && self.session.physical_down {
            let delivery = self
                .session
                .pointer(MouseAction::Move, pointer, elapsed(self.origin));
            return self.defer(delivery);
        }
        if matches!(message, WM_MOUSEWHEEL | WM_MOUSEHWHEEL) {
            self.interrupt(pointer);
        }
        false
    }
    fn interrupt(&mut self, pointer: Pointer) {
        self.session.last = pointer;
        let delivery = self.session.control(Event::Cancel, elapsed(self.origin));
        self.defer(delivery);
    }
}

// No Worker borrow crosses SendInput. Win32 may reenter hooks while injecting;
// queued foreign input must see current capture state and preserve its ordering.
fn flush() {
    loop {
        let next = WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let w = slot.as_mut()?;
            w.pending.pop_front().map(|(delivery, target)| {
                let accepted = Arc::new(AtomicU32::new(0));
                let release = delivery
                    .release
                    .map(|release| (release, delivery.consume))
                    .or_else(|| {
                        w.pending.iter().find_map(|(queued, _)| {
                            (queued.press_id == delivery.press_id)
                                .then_some(queued.release.map(|release| (release, queued.consume)))
                                .flatten()
                        })
                    });
                w.in_flight = Some(InFlight {
                    press_id: delivery.press_id,
                    release: release.map(|(release, _)| release),
                    release_consumed: release.is_some_and(|(_, consumed)| consumed),
                    target,
                    epoch: delivery.epoch,
                    injections: delivery.injections.clone(),
                    logical_before: delivery.logical_before,
                    accepted: Arc::clone(&accepted),
                });
                let record = w
                    .diagnostics
                    .is_enabled()
                    .then(|| {
                        replay_up_time(&delivery.injections).map(|time| {
                            let mut record = w.record(DiagnosticPhase::ReplayUpSending);
                            record.event_time_ms = time;
                            record.delivery_epoch = delivery.epoch;
                            record.planned_count = delivery.injections.len() as u32;
                            record
                        })
                    })
                    .flatten();
                (
                    delivery,
                    target,
                    w.session.button,
                    w.session.last.modifiers,
                    record,
                    accepted,
                )
            })
        });
        let Some((delivery, target, button, modifiers, record, accepted)) = next else {
            return;
        };
        if let Some(record) = &record {
            record_diagnostic(record.clone());
        }
        let result = inject(
            &delivery.injections,
            target,
            button,
            modifiers,
            delivery.epoch,
            accepted,
        );
        let failed = WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let w = slot.as_mut().unwrap();
            let accepted = match result {
                Ok(()) => delivery.injections.len(),
                Err((error, accepted)) => {
                    w.error.get_or_insert(error);
                    accepted
                }
            };
            if let Some(mut record) = record {
                record.phase = DiagnosticPhase::ReplayUpSent;
                record.accepted_count = accepted as u32;
                record.failure = result.err().map(|(error, _)| error);
                record.after = Some(w.session.diagnostic_state());
                w.diagnostics.record(record);
            }
            w.apply(ContextOutput {
                output: delivery.output,
                epoch: delivery.epoch,
            });
            if w.error.is_some() && (!w.finishing || !delivery.injections.is_empty()) {
                w.record_failure(w.error.unwrap());
                // Later hook callbacks may already have queued speculative
                // recovery. Keep only the actually accepted prefix before stop
                // derives its final recovery; never inject that later work.
                w.in_flight
                    .as_ref()
                    .unwrap()
                    .accepted
                    .store(accepted as u32, Ordering::Release);
                let close = w.reconcile_in_flight();
                w.pending.clear();
                Some(close)
            } else {
                w.in_flight = None;
                None
            }
        });
        if let Some(close) = failed {
            complete_replay_close(close);
            WORKER.with(|slot| slot.borrow_mut().as_mut().unwrap().finish());
            // Reconciliation can expose a newer held physical press while the
            // prior close pumps. Drain its one final attempt before disconnecting.
            continue;
        }
    }
}
fn complete_replay_close(close: Option<(Injection, Option<Target>, Option<u64>)>) {
    if let Some((up, target, epoch)) = close {
        let progress = Arc::new(AtomicU32::new(0));
        WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let worker = slot.as_mut().unwrap();
            worker.closing_replay = true;
            worker.closing_submission = Some(Arc::clone(&progress));
        });
        let modifiers = WORKER.with(|slot| slot.borrow().as_ref().unwrap().session.last.modifiers);
        let result = inject(
            &[up],
            target,
            WORKER.with(|slot| slot.borrow().as_ref().unwrap().session.button),
            modifiers,
            epoch,
            progress,
        );
        let accepted = result
            .as_ref()
            .map_or_else(|(_, accepted)| *accepted == 1, |()| true);
        WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let worker = slot.as_mut().unwrap();
            worker.closing_replay = false;
            worker.closing_submission = None;
            worker.session.replay_close_completed(accepted);
            // Pumping the close may have completed a later gesture. None of its
            // speculative actions/native work may escape the fatal older failure.
            worker.pending.clear();
            if let Err((error, _)) = result {
                worker.error.get_or_insert(error);
                worker.record_failure(error);
            }
        });
    }
}
fn replay_up_time(events: &[Injection]) -> Option<u32> {
    events.iter().find_map(|event| match event {
        Injection::Mouse {
            action: MouseAction::Up,
            time,
            ..
        } => Some(*time),
        _ => None,
    })
}
fn inject(
    events: &[Injection],
    target: Option<Target>,
    button: u32,
    latest_modifiers: u8,
    epoch: Option<u64>,
    accepted: Arc<AtomicU32>,
) -> Result<(), (Error, usize)> {
    if events.is_empty() {
        return Ok(());
    }
    let down_points = events
        .iter()
        .filter_map(|event| match event {
            Injection::Mouse {
                point,
                action: MouseAction::Down,
                ..
            } => Some(POINT {
                x: point.x as i32,
                y: point.y as i32,
            }),
            _ => None,
        })
        .collect();
    let metrics = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if metrics.2 <= 0 || metrics.3 <= 0 {
        return Err((Error::System(ERROR_INVALID_DATA), 0));
    }
    let inputs: Vec<_> = events
        .iter()
        .map(|event| input(*event, button, metrics))
        .collect();
    let time = if diagnostics_enabled() {
        events
            .iter()
            .find_map(|event| match event {
                Injection::Mouse { time, .. } => Some(*time),
                _ => None,
            })
            .unwrap_or(0)
    } else {
        0
    };
    let expected = inputs.len() as u32;
    let (sent, system, failure) =
        send_inputs(inputs, button, epoch, time, target, down_points, accepted);
    if sent == expected && failure.is_none() {
        return Ok(());
    }
    // Restore any modifier changes accepted before a partial failure. A
    // blocked or short injection is always reported, never called success.
    let mut sides = latest_modifiers;
    for event in &events[..sent as usize] {
        if let Injection::Key { index, down } = event {
            if *down {
                sides |= 1 << index;
            } else {
                sides &= !(1 << index);
            }
        }
    }
    let restore: Vec<_> = (0..8)
        .filter(|i| sides & (1 << i) != latest_modifiers & (1 << i))
        .map(|index| {
            input(
                Injection::Key {
                    index,
                    down: latest_modifiers & (1 << index) != 0,
                },
                button,
                metrics,
            )
        })
        .collect();
    if !restore.is_empty() {
        let _ = send_inputs(
            restore,
            button,
            epoch,
            0,
            None,
            vec![],
            Arc::new(AtomicU32::new(0)),
        );
    }
    Err((
        failure.unwrap_or(Error::Replay {
            system,
            sent,
            expected,
        }),
        sent as usize,
    ))
}

struct NativeBatch {
    inputs: Vec<INPUT>,
    target: Option<Target>,
    down_points: Vec<POINT>,
    accepted: Arc<AtomicU32>,
}
type NativeResult = Result<(u32, u32), Error>;

// No host code, Session mutation, diagnostics or observation publication runs here.
fn native_send(batch: NativeBatch) -> NativeResult {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // Revalidate delayed downs on the injector immediately before submission.
    if batch
        .down_points
        .iter()
        .any(|point| !batch.target.is_some_and(|target| target.matches(*point)))
    {
        return Err(Error::ReplayTargetChanged);
    }
    let sent = unsafe {
        SendInput(
            batch.inputs.len() as u32,
            batch.inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    let system = if sent as usize == batch.inputs.len() {
        0
    } else {
        unsafe { GetLastError() }
    };
    // Retain actual acceptance even if result delivery or worker teardown fails.
    batch.accepted.store(sent, Ordering::Release);
    Ok((sent, system))
}

struct NativeReplay {
    worker: Replay<NativeBatch, NativeResult>,
    completed: Arc<EventHandle>,
    healthy: bool,
}
impl NativeReplay {
    fn start() -> Result<Self, Error> {
        let handle = unsafe { CreateEventW(null_mut(), 0, 0, std::ptr::null()) };
        if handle.is_null() {
            return Err(Error::System(unsafe { GetLastError() }));
        }
        let completed = Arc::new(EventHandle(handle as usize));
        let signal: Arc<dyn Signal> = completed.clone();
        let worker = Replay::start(native_send, signal)
            .map_err(|error| Error::System(error.raw_os_error().unwrap_or(0) as u32))?;
        Ok(Self {
            worker,
            completed,
            healthy: true,
        })
    }

    fn submit(&mut self, batch: NativeBatch) -> Result<(u32, u32), (Error, u32)> {
        let accepted = Arc::clone(&batch.accepted);
        if !self.healthy {
            // Exceptional final recovery is safe synchronously only after unhook.
            // Never retry a previously accepted batch on another thread.
            if hooks_removed() {
                return native_send(batch).map_err(|error| (error, 0));
            }
            fail_replay(Error::WorkerStopped);
            return Err((Error::WorkerStopped, 0));
        }
        if unsafe { ResetEvent(self.completed.raw()) } == 0 {
            let error = Error::System(unsafe { GetLastError() });
            fail_replay(error);
            self.healthy = false;
            let _ = self.worker.close_and_join();
            return Err((error, 0));
        }
        if let Err(error) = self.worker.submit(batch) {
            let already_detached = hooks_removed();
            self.healthy = false;
            fail_replay(Error::WorkerStopped);
            let _ = self.worker.close_and_join();
            if let SubmitError::Disconnected(batch) = error
                && already_detached
            {
                // This final-recovery job never entered the disconnected queue.
                return native_send(batch).map_err(|error| (error, 0));
            }
            return Err((Error::WorkerStopped, 0));
        }
        loop {
            match self.worker.poll() {
                Poll::Completed(result) => {
                    if self.worker.thread().is_some_and(JoinHandle::is_finished) {
                        self.healthy = false;
                        fail_replay(Error::WorkerStopped);
                        return Err((Error::WorkerStopped, accepted.load(Ordering::Acquire)));
                    }
                    return result.map_err(|error| (error, 0));
                }
                Poll::Panicked | Poll::Disconnected => {
                    self.healthy = false;
                    fail_replay(Error::WorkerStopped);
                    let _ = self.worker.close_and_join();
                    return Err((Error::WorkerStopped, accepted.load(Ordering::Acquire)));
                }
                Poll::Pending => {}
            }
            let Some(thread) = self.worker.thread() else {
                self.healthy = false;
                fail_replay(Error::WorkerStopped);
                return Err((Error::WorkerStopped, accepted.load(Ordering::Acquire)));
            };
            let handles = [self.completed.raw(), thread.as_raw_handle() as HANDLE];
            let wait = unsafe {
                MsgWaitForMultipleObjectsEx(
                    handles.len() as u32,
                    handles.as_ptr(),
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                )
            };
            if wait == WAIT_FAILED {
                let error = Error::System(unsafe { GetLastError() });
                fail_replay(error); // Unhook before the blocking injector join.
                self.healthy = false;
                let _ = self.worker.close_and_join();
                let sent = accepted.load(Ordering::Acquire);
                let _ = self.worker.poll();
                return Err((error, sent));
            }
            // Do not flush or accept commands recursively. Hook callbacks retain
            // their input lifetimes and queue tails behind this in-flight batch.
            let mut message = unsafe { std::mem::zeroed::<MSG>() };
            unsafe {
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if HOOK_FAILED.with(|flag| flag.replace(false)) {
                fail_replay(Error::WorkerStopped);
                self.healthy = false;
                let _ = self.worker.close_and_join();
                let _ = self.worker.poll();
                return Err((Error::WorkerStopped, accepted.load(Ordering::Acquire)));
            }
        }
    }
}

fn hooks_removed() -> bool {
    WORKER.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_none_or(|worker| worker.mouse.is_null() && worker.keyboard.is_null())
    })
}
fn fail_replay(error: Error) {
    WORKER.with(|slot| {
        if let Some(worker) = slot.borrow_mut().as_mut() {
            worker.error.get_or_insert(error);
            worker.record_failure(error);
            worker.detach_hooks();
        }
    });
}

fn send_inputs(
    inputs: Vec<INPUT>,
    button: u32,
    epoch: Option<u64>,
    time: u32,
    target: Option<Target>,
    down_points: Vec<POINT>,
    accepted: Arc<AtomicU32>,
) -> (u32, u32, Option<Error>) {
    let expected = inputs.len() as u32;
    let record = diagnostics_enabled().then(|| {
        let mut record = DiagnosticRecord::new(DiagnosticPhase::SendInputStarted);
        record.button = button;
        record.delivery_epoch = epoch;
        record.event_time_ms = time;
        record.before = diagnostic_state();
        record.planned_count = expected;
        record
    });
    if let Some(record) = &record {
        record_diagnostic(record.clone());
    }
    let batch = NativeBatch {
        inputs,
        target,
        down_points,
        accepted,
    };
    let result = REPLAY.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map_or(Err((Error::WorkerStopped, 0)), |replay| {
                replay.submit(batch)
            })
    });
    let (sent, system, failure) = match result {
        Ok((sent, system)) => (sent, system, None),
        Err((error, sent)) => (sent, 0, Some(error)),
    };
    if let Some(mut record) = record {
        record.phase = DiagnosticPhase::SendInputCompleted;
        record.after = diagnostic_state();
        record.accepted_count = sent;
        record.failure = failure;
        if record.failure.is_none() && sent != expected {
            record.failure = Some(Error::Replay {
                system,
                sent,
                expected,
            });
        }
        record_diagnostic(record);
    }
    (sent, system, failure)
}

fn run(
    session: Session,
    resolver: Option<ContextResolver>,
    commands: mpsc::Receiver<Command>,
    outputs: ObservationSender,
    wake: Arc<EventHandle>,
    diagnostics: Arc<Diagnostics>,
    started: mpsc::SyncSender<Result<(), Error>>,
) -> Result<(), Error> {
    let origin = diagnostics.origin();
    // Thread-only DPI awareness never changes the host process's UI policy.
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let integrity =
        integrity_level(unsafe { GetCurrentProcess() }).ok_or(Error::System(ERROR_ACCESS_DENIED));
    let integrity = match integrity {
        Ok(level) => level,
        Err(error) => {
            let _ = started.send(Err(error));
            return Err(error);
        }
    };
    let replay = match NativeReplay::start() {
        Ok(replay) => replay,
        Err(error) => {
            let _ = started.send(Err(error));
            return Err(error);
        }
    };
    REPLAY.with(|slot| *slot.borrow_mut() = Some(replay));
    WORKER.with(|slot| {
        *slot.borrow_mut() = Some(Worker {
            session,
            resolver,
            wake: Arc::clone(&wake),
            origin,
            diagnostics: Arc::clone(&diagnostics),
            mouse: null_mut(),
            keyboard: null_mut(),
            outputs,
            pending: VecDeque::new(),
            in_flight: None,
            closing_replay: false,
            closing_submission: None,
            target: None,
            last_output: None,
            deadline: None,
            error: None,
            failure_recorded: false,
            stopped: false,
            finishing: false,
            shutdown_attempt: None,
            escape_down: false,
            integrity,
        })
    });
    let mut module = null_mut();
    let installed = unsafe {
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            mouse_hook as *const () as *const u16,
            &mut module,
        ) == 0
        {
            Err(Error::System(GetLastError()))
        } else {
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
            let error = GetLastError();
            WORKER.with(|slot| {
                let mut slot = slot.borrow_mut();
                let w = slot.as_mut().unwrap();
                w.mouse = mouse;
                w.keyboard = keyboard;
            });
            if mouse.is_null() || keyboard.is_null() {
                Err(Error::System(error))
            } else {
                Ok(())
            }
        }
    };
    if let Err(error) = installed {
        let _ = started.send(Err(error));
        return Err(error);
    }
    diagnostics.record(DiagnosticRecord::new(DiagnosticPhase::HooksInstalled));
    WORKER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let w = slot.as_mut().unwrap();
        let delivery = w.session.control(Event::Start, elapsed(origin));
        w.defer(delivery);
    });
    flush();
    let _ = started.send(Ok(()));
    loop {
        flush();
        let ending = WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let w = slot.as_mut().unwrap();
            if HOOK_FAILED.with(|flag| flag.replace(false)) {
                w.error.get_or_insert(Error::WorkerStopped);
            }
            if w.error.is_some() || w.stopped || w.finishing {
                if let Some(error) = w.error {
                    w.record_failure(error);
                }
                w.finish();
                return true;
            }
            false
        });
        if ending {
            flush();
            WORKER.with(|slot| slot.borrow_mut().as_mut().unwrap().detach_hooks());
            return WORKER.with(|slot| slot.borrow().as_ref().unwrap().error.map_or(Ok(()), Err));
        }
        unsafe {
            ResetEvent(wake.raw());
        }
        while let Ok(command) = commands.try_recv() {
            let status = WORKER.with(|slot| {
                let mut slot = slot.borrow_mut();
                let w = slot.as_mut().unwrap();
                if w.stopped || w.finishing || w.error.is_some() {
                    return Err(w.error.unwrap_or(Error::WorkerStopped));
                }
                let now = elapsed(origin);
                let phase = match &command.operation {
                    Operation::Cancel => Some(DiagnosticPhase::CancelCommand),
                    Operation::ContextResolver(_) => Some(DiagnosticPhase::ResolverCommand),
                    Operation::Stop => Some(DiagnosticPhase::StopCommand),
                    _ => None,
                };
                let record = phase.map(|phase| w.record(phase));
                if let Some(record) = &record {
                    w.diagnostics.record(record.clone());
                }
                let event = match command.operation {
                    Operation::Matcher(matcher) => {
                        w.session.engine.use_matcher(&matcher);
                        Event::Snapshot
                    }
                    Operation::Policy(policy) => {
                        w.session.policy = policy;
                        Event::Snapshot
                    }
                    Operation::ContextResolver(resolver) => {
                        w.resolver = Some(resolver);
                        Event::Snapshot
                    }
                    Operation::Recording(value) => Event::Recording(value),
                    Operation::Tuning(config) => {
                        w.session
                            .engine
                            .update_tuning(config, now, false)
                            .map_err(|_| Error::InvalidConfiguration)?;
                        Event::Snapshot
                    }
                    Operation::Cancel => Event::Cancel,
                    Operation::Stop => {
                        w.finish();
                        if let Some(mut record) = record {
                            record.phase = DiagnosticPhase::StopCompleted;
                            record.after = Some(w.session.diagnostic_state());
                            record.failure = w.error;
                            w.diagnostics.record(record);
                        }
                        return w.error.map_or(Ok(()), Err);
                    }
                };
                w.session.last.modifiers = modifiers();
                let delivery = w.session.control(event, now);
                w.defer(delivery);
                if let Some(mut record) = record {
                    record.phase = match record.phase {
                        DiagnosticPhase::CancelCommand => DiagnosticPhase::CancelCompleted,
                        DiagnosticPhase::ResolverCommand => DiagnosticPhase::ResolverCompleted,
                        _ => unreachable!(),
                    };
                    record.after = Some(w.session.diagnostic_state());
                    record.failure = w.error;
                    w.diagnostics.record(record);
                }
                w.error.map_or(Ok(()), Err)
            });
            flush();
            let status = status.and_then(|()| {
                WORKER.with(|slot| slot.borrow().as_ref().unwrap().error.map_or(Ok(()), Err))
            });
            let _ = command.reply.send(status);
        }
        if WORKER.with(|slot| {
            let slot = slot.borrow();
            let w = slot.as_ref().unwrap();
            w.stopped || w.finishing || w.error.is_some()
        }) {
            continue;
        }
        let timeout = WORKER.with(|slot| {
            slot.borrow()
                .as_ref()
                .unwrap()
                .deadline
                .map_or(INFINITE, |d| {
                    d.saturating_sub(elapsed(origin))
                        .div_ceil(1_000_000)
                        .min(u64::from(INFINITE - 1)) as u32
                })
        });
        let injector = REPLAY.with(|slot| {
            slot.borrow().as_ref().and_then(|replay| {
                replay
                    .worker
                    .thread()
                    .map(|thread| thread.as_raw_handle() as HANDLE)
            })
        });
        let handles = [wake.raw(), injector.unwrap_or(wake.raw())];
        let wait = unsafe {
            MsgWaitForMultipleObjectsEx(
                if injector.is_some() { 2 } else { 1 },
                handles.as_ptr(),
                timeout,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if wait == WAIT_FAILED {
            return Err(Error::System(unsafe { GetLastError() }));
        }
        if injector.is_some() && wait == WAIT_OBJECT_0 + 1 {
            fail_replay(Error::WorkerStopped);
            REPLAY.with(|slot| {
                if let Some(replay) = slot.borrow_mut().as_mut() {
                    replay.healthy = false;
                    let _ = replay.worker.close_and_join();
                }
            });
            continue;
        }
        let mut message = unsafe { std::mem::zeroed::<MSG>() };
        unsafe {
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
                flush();
            }
        }
        flush();
        WORKER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let w = slot.as_mut().unwrap();
            if !w.stopped {
                let delivery = w.session.control(Event::Tick, elapsed(origin));
                // An idle wakeup without an elapsed deadline is not a UI update.
                if !delivery.output.effects.is_empty()
                    || delivery.output.discard_pending_visible_trace
                    || !delivery.injections.is_empty()
                {
                    w.defer(delivery);
                }
            }
        });
    }
}

fn hook_failed() {
    HOOK_FAILED.with(|flag| flag.set(true));
    // Wake the owner loop even when the callback arrived during a native wait.
    unsafe {
        PostThreadMessageW(GetCurrentThreadId(), WM_APP, 0, 0);
    }
}

fn record_diagnostic(record: DiagnosticRecord) {
    DIAGNOSTICS.with(|slot| {
        if let Ok(slot) = slot.try_borrow()
            && let Some(diagnostics) = slot.as_ref()
        {
            diagnostics.record(record);
        }
    });
}

fn diagnostics_enabled() -> bool {
    DIAGNOSTICS.with(|slot| {
        slot.try_borrow().ok().is_some_and(|slot| {
            slot.as_ref()
                .is_some_and(|diagnostics| diagnostics.is_enabled())
        })
    })
}

fn diagnostic_clock() -> Option<Instant> {
    diagnostics_enabled().then(Instant::now)
}

fn diagnostic_state() -> Option<crate::DiagnosticState> {
    WORKER.with(|slot| {
        slot.try_borrow().ok().and_then(|slot| {
            slot.as_ref()
                .map(|worker| worker.session.diagnostic_state())
        })
    })
}

fn finish_hook_record(record: Option<DiagnosticRecord>, began: Option<Instant>, result: LRESULT) {
    if let Some(mut record) = record {
        record.callback_duration_ns = began.map_or(0, elapsed);
        record.hook_result = result as i64;
        record_diagnostic(record);
    }
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    let began = diagnostic_clock();
    if code < 0 {
        let result = unsafe { CallNextHookEx(null_mut(), code, message, data) };
        finish_hook_record(
            began.map(|_| DiagnosticRecord::new(DiagnosticPhase::NegativeHookCode)),
            began,
            result,
        );
        return result;
    }
    let packet = unsafe { *(data as *const MSLLHOOKSTRUCT) };
    let button_action = mouse_action(message as u32, packet.mouseData);
    let motion = message as u32 == WM_MOUSEMOVE;
    let phase = |down, up, movement| match button_action {
        Some((_, MouseAction::Down)) => down,
        Some((_, MouseAction::Up)) => up,
        _ => movement,
    };
    let mut record = began.and_then(|_| {
        (button_action.is_some() || motion).then(|| {
            let mut record = DiagnosticRecord::new(phase(
                DiagnosticPhase::RawDownEntered,
                DiagnosticPhase::RawUpEntered,
                DiagnosticPhase::RawMoveEntered,
            ));
            record.button = button_action.map_or(0, |(button, _)| button);
            record.event_time_ms = packet.time;
            record.event_age_ms = unsafe { GetTickCount() }.wrapping_sub(packet.time);
            record
        })
    });
    if packet.dwExtraInfo == MARKER {
        if let Some(record) = record.as_mut() {
            record.phase = phase(
                DiagnosticPhase::OwnDownPassed,
                DiagnosticPhase::OwnUpPassed,
                DiagnosticPhase::OwnMoveEntered,
            );
            if motion {
                record.before = diagnostic_state();
                record_diagnostic(record.clone());
                record.phase = DiagnosticPhase::OwnMovePassed;
            }
        }
        let result = unsafe { CallNextHookEx(null_mut(), code, message, data) };
        finish_hook_record(record, began, result);
        return result;
    }
    if let Some(record) = record.as_mut() {
        // This inspection is nonblocking and ends before processing. An entry
        // remains useful if a later target query or host resolver never returns.
        record.before = diagnostic_state();
        record_diagnostic(record.clone());
    }
    let consume = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        WORKER.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                if let Some(record) = record.as_mut() {
                    record.phase = phase(
                        DiagnosticPhase::RawDownBorrowFailed,
                        DiagnosticPhase::RawUpBorrowFailed,
                        DiagnosticPhase::RawMoveBorrowFailed,
                    );
                    record.failure = Some(Error::WorkerStopped);
                }
                hook_failed();
                return false;
            };
            let Some(worker) = slot.as_mut() else {
                if let Some(record) = record.as_mut() {
                    record.phase = phase(
                        DiagnosticPhase::RawDownNoWorker,
                        DiagnosticPhase::RawUpNoWorker,
                        DiagnosticPhase::RawMoveNoWorker,
                    );
                }
                return false;
            };
            if let Some(record) = record.as_mut() {
                record.before = Some(worker.session.diagnostic_state());
                record.phase = if worker.stopped {
                    phase(
                        DiagnosticPhase::RawDownStopped,
                        DiagnosticPhase::RawUpStopped,
                        DiagnosticPhase::RawMoveStopped,
                    )
                } else if worker.error.is_some() {
                    phase(
                        DiagnosticPhase::RawDownError,
                        DiagnosticPhase::RawUpError,
                        DiagnosticPhase::RawMoveError,
                    )
                } else {
                    phase(
                        DiagnosticPhase::RawDownPassed,
                        DiagnosticPhase::RawUpPassed,
                        DiagnosticPhase::RawMovePassed,
                    )
                };
            }
            let consume = worker.mouse(message as u32, packet);
            if let Some(record) = record.as_mut() {
                record.after = Some(worker.session.diagnostic_state());
                record.failure = worker.error;
                if consume {
                    record.phase = phase(
                        DiagnosticPhase::RawDownConsumed,
                        DiagnosticPhase::RawUpConsumed,
                        DiagnosticPhase::RawMoveConsumed,
                    );
                }
            }
            consume
        })
    }))
    .unwrap_or_else(|_| {
        if let Some(record) = record.as_mut() {
            record.phase = phase(
                DiagnosticPhase::RawDownPanicked,
                DiagnosticPhase::RawUpPanicked,
                DiagnosticPhase::RawMovePanicked,
            );
            record.failure = Some(Error::WorkerStopped);
        }
        hook_failed();
        false
    });
    let result = if consume {
        1
    } else {
        unsafe { CallNextHookEx(null_mut(), code, message, data) }
    };
    finish_hook_record(record, began, result);
    result
}
unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    let began = diagnostic_clock();
    if code < 0 {
        return pass_keyboard(code, message, data, None, began);
    }
    let packet = unsafe { *(data as *const KBDLLHOOKSTRUCT) };
    if packet.dwExtraInfo == MARKER || packet.vkCode != u32::from(VK_ESCAPE) {
        return pass_keyboard(code, message, data, Some(packet.time), began);
    }
    let consume = std::panic::catch_unwind(|| {
        WORKER.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                hook_failed();
                return false;
            };
            let Some(w) = slot.as_mut() else {
                return false;
            };
            if w.stopped {
                return false;
            }
            if matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
                return std::mem::take(&mut w.escape_down);
            }
            if w.error.is_some() || w.finishing {
                return w.escape_down;
            }
            if w.escape_down {
                return true;
            }
            w.session.last.modifiers = modifiers();
            let delivery = w.session.control(Event::Cancel, elapsed(w.origin));
            let consume = delivery.consume;
            w.defer(delivery);
            w.escape_down = consume;
            consume
        })
    })
    .unwrap_or_else(|_| {
        hook_failed();
        false
    });
    if consume {
        1
    } else {
        pass_keyboard(code, message, data, Some(packet.time), began)
    }
}

fn pass_keyboard(
    code: i32,
    message: WPARAM,
    data: LPARAM,
    time: Option<u32>,
    began: Option<Instant>,
) -> LRESULT {
    let mut record = began.map(|_| {
        let mut record = DiagnosticRecord::new(DiagnosticPhase::KeyboardPassthroughEntered);
        if let Some(time) = time {
            record.event_time_ms = time;
            record.event_age_ms = unsafe { GetTickCount() }.wrapping_sub(time);
        }
        record.before = diagnostic_state();
        record
    });
    if let Some(record) = &record {
        record_diagnostic(record.clone());
    }
    let result = unsafe { CallNextHookEx(null_mut(), code, message, data) };
    if let Some(record) = record.as_mut() {
        record.phase = DiagnosticPhase::KeyboardPassthroughCompleted;
        record.after = diagnostic_state();
    }
    finish_hook_record(record, began, result);
    result
}

fn modifiers() -> u8 {
    MODIFIER_KEYS.iter().enumerate().fold(0, |mask, (i, key)| {
        mask | if unsafe { GetAsyncKeyState(i32::from(*key)) } < 0 {
            1 << i
        } else {
            0
        }
    })
}
fn mouse_action(message: u32, data: u32) -> Option<(u32, MouseAction)> {
    Some(match message {
        WM_LBUTTONDOWN => (0, MouseAction::Down),
        WM_LBUTTONUP => (0, MouseAction::Up),
        WM_RBUTTONDOWN => (1, MouseAction::Down),
        WM_RBUTTONUP => (1, MouseAction::Up),
        WM_MBUTTONDOWN => (2, MouseAction::Down),
        WM_MBUTTONUP => (2, MouseAction::Up),
        WM_XBUTTONDOWN | WM_XBUTTONUP if matches!(data >> 16, 1 | 2) => (
            2 + (data >> 16),
            if message == WM_XBUTTONDOWN {
                MouseAction::Down
            } else {
                MouseAction::Up
            },
        ),
        _ => return None,
    })
}
fn input(event: Injection, button: u32, metrics: (i32, i32, i32, i32)) -> INPUT {
    match event {
        Injection::Mouse {
            point,
            action,
            time,
        } => {
            let flag = match (button, action) {
                (_, MouseAction::Move) => 0,
                (0, MouseAction::Down) => MOUSEEVENTF_LEFTDOWN,
                (0, MouseAction::Up) => MOUSEEVENTF_LEFTUP,
                (1, MouseAction::Down) => MOUSEEVENTF_RIGHTDOWN,
                (1, MouseAction::Up) => MOUSEEVENTF_RIGHTUP,
                (2, MouseAction::Down) => MOUSEEVENTF_MIDDLEDOWN,
                (2, MouseAction::Up) => MOUSEEVENTF_MIDDLEUP,
                (_, MouseAction::Down) => MOUSEEVENTF_XDOWN,
                (_, MouseAction::Up) => MOUSEEVENTF_XUP,
            };
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: absolute(point.x, metrics.0, metrics.2),
                        dy: absolute(point.y, metrics.1, metrics.3),
                        mouseData: if button > 2 && action != MouseAction::Move {
                            button - 2
                        } else {
                            0
                        },
                        dwFlags: flag
                            | MOUSEEVENTF_MOVE
                            | MOUSEEVENTF_ABSOLUTE
                            | MOUSEEVENTF_VIRTUALDESK,
                        time,
                        dwExtraInfo: MARKER,
                    },
                },
            }
        }
        Injection::Key { index, down } => INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: MODIFIER_KEYS[index],
                    wScan: 0,
                    dwFlags: if down { 0 } else { KEYEVENTF_KEYUP }
                        | if matches!(index, 3 | 5 | 6 | 7) {
                            KEYEVENTF_EXTENDEDKEY
                        } else {
                            0
                        },
                    time: 0,
                    dwExtraInfo: MARKER,
                },
            },
        },
    }
}

fn integrity_level(process: HANDLE) -> Option<u32> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut length = 0;
        GetTokenInformation(token, TokenIntegrityLevel, null_mut(), 0, &mut length);
        let mut bytes = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            bytes.as_mut_ptr().cast(),
            length,
            &mut length,
        ) != 0;
        CloseHandle(token);
        if !ok {
            return None;
        }
        let label = &*bytes.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        (count > 0).then(|| *GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)))
    }
}
#[derive(Clone, Copy)]
struct Target {
    window: usize,
    process: u32,
    thread: u32,
}
impl Target {
    fn recipient(point: POINT) -> Option<Self> {
        unsafe {
            // GetCapture only sees the caller's thread. A foreground application
            // can already own native capture, including in another process.
            let mut info = std::mem::zeroed::<GUITHREADINFO>();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(0, &mut info) == 0 {
                return None;
            }
            let window = if info.hwndCapture.is_null() {
                WindowFromPoint(point)
            } else {
                info.hwndCapture
            };
            if window.is_null() {
                return None;
            }
            let mut process = 0;
            let thread = GetWindowThreadProcessId(window, &mut process);
            (thread != 0 && process != 0).then_some(Self {
                window: window as usize,
                process,
                thread,
            })
        }
    }
    fn at(point: POINT, own: u32) -> Option<Self> {
        let target = Self::recipient(point)?;
        unsafe {
            if target.process == GetCurrentProcessId() {
                return Some(target);
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, target.process);
            if process.is_null() {
                return None;
            }
            let integrity = integrity_level(process);
            CloseHandle(process);
            integrity.filter(|value| *value <= own).map(|_| target)
        }
    }
    fn matches(self, point: POINT) -> bool {
        Self::recipient(point).is_some_and(|current| {
            self.window == current.window
                && self.process == current.process
                && self.thread == current.thread
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pointer_gesture_recognition::{Direction, Point};

    // This fixture allocates kernel events but never installs hooks or injects input.
    fn worker() -> Worker {
        let mut matcher = Matcher::default();
        matcher.register(&[Direction::Right], 1).unwrap();
        let mut session = Session::new(
            Configuration {
                start_distance: 10.,
                change_distance: 5.,
                total_duration_ns: 0,
                idle_duration_ns: 0,
                maximum_points: 256,
                maximum_directions: 64,
                button: 1,
                passes_through_empty_matcher: true,
            },
            &matcher,
        )
        .unwrap();
        session.control(Event::Start, 0);
        let origin = Instant::now();
        let raw = unsafe { CreateEventW(null_mut(), 1, 0, std::ptr::null()) };
        assert!(!raw.is_null());
        let (outputs, _) = notification::channel(OBSERVATIONS).unwrap();
        Worker {
            session,
            resolver: None,
            wake: Arc::new(EventHandle(raw as usize)),
            origin,
            diagnostics: Arc::new(Diagnostics::new(origin)),
            mouse: null_mut(),
            keyboard: null_mut(),
            outputs,
            pending: VecDeque::new(),
            in_flight: None,
            closing_replay: false,
            closing_submission: None,
            target: None,
            last_output: None,
            deadline: None,
            error: None,
            failure_recorded: false,
            stopped: false,
            finishing: false,
            shutdown_attempt: None,
            escape_down: false,
            integrity: 0,
        }
    }
    fn pointer(x: f64, time: u32) -> Pointer {
        Pointer {
            point: Point { x, y: 0. },
            modifiers: 0,
            time,
        }
    }
    fn downs(delivery: &Delivery) -> usize {
        delivery
            .injections
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Injection::Mouse {
                        action: MouseAction::Down,
                        ..
                    }
                )
            })
            .count()
    }

    #[test]
    fn failed_stop_recovery_restores_a_newer_held_press_once() {
        let mut worker = worker();
        worker.session.pointer(MouseAction::Down, pointer(0., 1), 1);
        worker
            .session
            .pointer(MouseAction::Move, pointer(30., 2), 2);
        worker.finish();
        let (old, _) = worker.pending.pop_front().unwrap();
        assert_eq!(downs(&old), 1);
        let up = worker.session.pointer(MouseAction::Up, pointer(30., 3), 3);
        worker.session.policy.enabled = false;
        let mut next = worker
            .session
            .pointer(MouseAction::Down, pointer(40., 4), 4);
        worker.session.defer_passthrough_down(&mut next);
        let close = worker.session.reconcile_failed(
            old.press_id,
            old.logical_before,
            &old.injections[..1],
            up.release,
        );
        assert!(matches!(
            close,
            Some(Injection::Mouse {
                action: MouseAction::Up,
                ..
            })
        ));
        worker.session.replay_close_completed(true);
        worker.pending.clear();
        worker.finish();
        let (new, _) = worker
            .pending
            .pop_front()
            .expect("a newer held press needs its own final restoration");
        assert_ne!(new.press_id, old.press_id);
        assert_eq!(downs(&new), 1);
        // Reject this final batch. Stop must not retry the same physical press.
        assert!(
            worker
                .session
                .reconcile_failed(new.press_id, new.logical_before, &[], None)
                .is_none()
        );
        worker.finish();
        assert!(worker.pending.is_empty());
    }

    #[test]
    fn accepted_old_up_still_allows_a_newer_held_stop_recovery() {
        let mut worker = worker();
        worker.session.pointer(MouseAction::Down, pointer(0., 1), 1);
        worker
            .session
            .pointer(MouseAction::Move, pointer(30., 2), 2);
        worker.finish();
        let (old, _) = worker.pending.pop_front().unwrap();
        let up = worker.session.pointer(MouseAction::Up, pointer(30., 3), 3);
        worker.session.policy.enabled = false;
        let mut next = worker
            .session
            .pointer(MouseAction::Down, pointer(40., 4), 4);
        worker.session.defer_passthrough_down(&mut next);
        // The closing up completed before an independent worker/queue failure.
        assert!(
            worker
                .session
                .reconcile_failed(up.press_id, true, &up.injections, up.release)
                .is_none()
        );
        worker.pending.clear();
        worker.finish();
        let (new, _) = worker.pending.pop_front().unwrap();
        assert_ne!(new.press_id, old.press_id);
        assert_eq!(downs(&new), 1);
        worker.finish();
        assert!(worker.pending.is_empty());
    }

    #[test]
    fn an_empty_terminal_frame_cannot_clear_a_failed_old_close() {
        let mut worker = worker();
        worker.session.pointer(MouseAction::Down, pointer(0., 1), 1);
        let old = worker.session.control(Event::Cancel, 2);
        let up = worker.session.pointer(MouseAction::Up, pointer(0., 3), 3);
        worker.session.policy.enabled = false;
        let mut next = worker
            .session
            .pointer(MouseAction::Down, pointer(40., 4), 4);
        worker.session.defer_passthrough_down(&mut next);
        assert!(
            worker
                .session
                .reconcile_failed(
                    old.press_id,
                    old.logical_before,
                    &old.injections[..1],
                    up.release
                )
                .is_some()
        );
        worker.session.replay_close_completed(false);
        worker.error = Some(Error::WorkerStopped);
        worker.finish();
        assert!(
            worker
                .pending
                .iter()
                .all(|(delivery, _)| delivery.injections.is_empty())
        );
        WORKER.with(|slot| *slot.borrow_mut() = Some(worker));
        // No hooks, injector or native input: exercise only empty-frame draining.
        flush();
        let mut worker = WORKER.with(|slot| slot.borrow_mut().take().unwrap());
        assert!(worker.pending.is_empty());
        assert!(worker.session.shutdown(5).injections.is_empty());
    }
}
