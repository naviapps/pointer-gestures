use crate::window::{REFRESH_LAYOUT, Resources};
use crate::{Configuration, Error};
use pointer_gesture_rendering::{Bounds, OverlayState, OverlayTrace, OverlayUpdate};
use std::{
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Instant,
};
use windows_sys::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

/// Last successfully submitted native state. The borrowed window identifier is
/// invalidated by worker failure or deactivation. Visibility is not a compositor
/// completion fence.
#[derive(Debug, Clone, Copy)]
pub struct WindowState {
    /// Non-owning HWND value; never destroy or reconfigure this window from hosts.
    pub window: usize,
    /// Whether visible geometry has been submitted to the OS.
    pub visible: bool,
    /// Physical desktop pixel rectangle, absent when hidden.
    pub bounds: Option<Bounds>,
}

enum CommandRequest {
    Update(Option<OverlayTrace>),
    Configure(Configuration),
    Clear,
    State,
    Shutdown,
}
struct Request {
    command: CommandRequest,
    reply: SyncSender<Result<WindowState, Error>>,
}
struct Wake(HANDLE);
// A kernel event can be signalled from any thread; Arc keeps it alive through join.
unsafe impl Send for Wake {}
unsafe impl Sync for Wake {}
impl Drop for Wake {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Worker {
    sender: SyncSender<Request>,
    wake: Arc<Wake>,
    thread: JoinHandle<()>,
    failure: Arc<Mutex<Option<Error>>>,
}
impl Worker {
    fn error(&self) -> Error {
        self.failure
            .lock()
            .ok()
            .and_then(|value| *value)
            .unwrap_or(Error::WorkerStopped)
    }
}

/// Explicitly activated overlay with one dedicated per-monitor-DPI-aware owner
/// thread. It never installs input hooks, takes focus or requests permissions.
/// Submit outside input hooks: requests wait for native completion and are bounded.
pub struct Overlay {
    configuration: Configuration,
    worker: Option<Worker>,
}
impl Overlay {
    /// Validate configuration. No thread, window or input capture starts here.
    pub fn new(configuration: Configuration) -> Result<Self, Error> {
        Ok(Self {
            configuration: configuration.validate()?,
            worker: None,
        })
    }
    /// Create a hidden click-through, nonactivating topmost window. Repeated calls
    /// are harmless. Only the new worker's DPI context changes, never the host's.
    pub fn activate(&mut self) -> Result<(), Error> {
        if let Some(worker) = &self.worker {
            return if worker.thread.is_finished() {
                Err(worker.error())
            } else {
                Ok(())
            };
        }
        let wake = unsafe { CreateEventW(null(), 0, 0, null()) };
        if wake.is_null() {
            return Err(system());
        }
        let wake = Arc::new(Wake(wake));
        let (sender, receiver) = mpsc::sync_channel(1);
        let (ready, started) = mpsc::sync_channel(1);
        let signal = Arc::clone(&wake);
        let configuration = self.configuration;
        let failure = Arc::new(Mutex::new(None));
        let failed = Arc::clone(&failure);
        let thread = thread::Builder::new()
            .name("pointer-gesture-overlay".into())
            .spawn(move || {
                let resources = Resources::new();
                match resources {
                    Ok(resources) => {
                        let _ = ready.send(Ok(()));
                        if let Err(error) = run(resources, configuration, &receiver, signal)
                            && let Ok(mut value) = failed.lock()
                        {
                            *value = Some(error);
                        }
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                    }
                }
            })
            .map_err(|_| Error::WorkerStopped)?;
        match started.recv().unwrap_or(Err(Error::WorkerStopped)) {
            Ok(()) => {
                self.worker = Some(Worker {
                    sender,
                    wake,
                    thread,
                    failure,
                });
                Ok(())
            }
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
    /// Submit copied physical-pixel geometry or schedule hiding with the shared
    /// final-frame cadence. At most 4096 directions are accepted. The worker owns
    /// ticks, clipping, rasterization and OS resources; it retains no input history.
    pub fn update(&mut self, trace: Option<OverlayTrace>) -> Result<WindowState, Error> {
        if trace.as_ref().is_some_and(|t| t.directions().len() > 4096) {
            return Err(Error::InvalidConfiguration);
        }
        self.request(CommandRequest::Update(trace))
    }
    /// Replace host appearance/cadence without recreating resources or resetting
    /// visible/pending trace state. Inactive owners retain values for activation.
    /// Invalid configuration leaves the current configuration and window intact.
    pub fn set_configuration(&mut self, configuration: Configuration) -> Result<(), Error> {
        let configuration = configuration.validate()?;
        if self.worker.is_some() {
            self.request(CommandRequest::Configure(configuration))?;
        }
        self.configuration = configuration;
        Ok(())
    }
    /// Immediately discard pending geometry and hide, e.g. on recognition cancel.
    pub fn clear(&mut self) -> Result<WindowState, Error> {
        self.request(CommandRequest::Clear)
    }
    /// Query the owner-thread state without exposing mutable OS resources.
    pub fn state(&mut self) -> Result<WindowState, Error> {
        self.request(CommandRequest::State)
    }
    /// Hide, destroy native resources and join. Repeated calls are harmless;
    /// activation can create a fresh owner afterward.
    pub fn deactivate(&mut self) -> Result<(), Error> {
        if self.worker.is_none() {
            return Ok(());
        }
        let result = self.request(CommandRequest::Shutdown).map(|_| ());
        let worker = self.worker.take().unwrap();
        let joined = worker.thread.join().map_err(|_| Error::WorkerStopped);
        result.and(joined)
    }
    fn request(&mut self, command: CommandRequest) -> Result<WindowState, Error> {
        let worker = self.worker.as_ref().ok_or(Error::Inactive)?;
        let (reply, response) = mpsc::sync_channel(1);
        worker
            .sender
            .send(Request { command, reply })
            .map_err(|_| worker.error())?;
        if unsafe { SetEvent(worker.wake.0) } == 0 {
            return Err(system());
        }
        response.recv().unwrap_or_else(|_| Err(worker.error()))
    }
}
impl Drop for Overlay {
    fn drop(&mut self) {
        let _ = self.deactivate();
    }
}

pub(super) fn system() -> Error {
    Error::System(unsafe { GetLastError() })
}
fn now(origin: Instant) -> u64 {
    origin.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

fn run(
    mut resources: Resources,
    mut configuration: Configuration,
    receiver: &Receiver<Request>,
    wake: Arc<Wake>,
) -> Result<(), Error> {
    let origin = Instant::now();
    let mut state = OverlayState::new(configuration.timing, None, 0);
    let mut deadline = None;
    loop {
        let elapsed = now(origin);
        let timeout = deadline.map_or(INFINITE, |d: u64| {
            d.saturating_sub(elapsed)
                .div_ceil(1_000_000)
                .min(u32::MAX as u64 - 1) as u32
        });
        let wait = unsafe {
            MsgWaitForMultipleObjectsEx(1, &wake.0, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };
        if wait == WAIT_FAILED {
            return Err(system());
        }
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if message.message == REFRESH_LAYOUT
                && message.hwnd as usize == resources.state().window
            {
                let mut update = state.tick(now(origin));
                update.changed = true;
                apply(&mut resources, update, configuration, &mut deadline)?;
                continue;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        while let Ok(request) = receiver.try_recv() {
            let mut shutdown = false;
            let result = match request.command {
                CommandRequest::Update(trace) => apply(
                    &mut resources,
                    state.update(trace, now(origin)),
                    configuration,
                    &mut deadline,
                ),
                CommandRequest::Configure(next) => {
                    configuration = next;
                    let mut update = state.reconfigure(configuration.timing, now(origin));
                    update.changed = true;
                    apply(&mut resources, update, configuration, &mut deadline)
                }
                CommandRequest::Clear => {
                    let update = state.clear(now(origin));
                    deadline = update.deadline_ns;
                    resources.hide();
                    Ok(resources.state())
                }
                CommandRequest::State => Ok(resources.state()),
                CommandRequest::Shutdown => {
                    shutdown = true;
                    resources.hide();
                    Ok(resources.state())
                }
            };
            let error = result.as_ref().err().copied();
            let _ = request.reply.send(result);
            if shutdown {
                return Ok(());
            }
            if let Some(error) = error {
                return Err(error);
            }
        }
        if deadline.is_some_and(|d| d <= now(origin)) {
            apply(
                &mut resources,
                state.tick(now(origin)),
                configuration,
                &mut deadline,
            )?;
        }
    }
}
fn apply(
    resources: &mut Resources,
    update: OverlayUpdate,
    configuration: Configuration,
    deadline: &mut Option<u64>,
) -> Result<WindowState, Error> {
    *deadline = update.deadline_ns;
    if update.changed {
        if let Some(trace) = update.trace {
            if let Err(error) = resources.present(&trace, configuration) {
                resources.hide();
                return Err(error);
            }
        } else {
            resources.hide();
        }
    }
    Ok(resources.state())
}
