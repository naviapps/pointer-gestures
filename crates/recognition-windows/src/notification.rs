//! Bounded ordered observations and level-triggered host readiness.
use crate::{
    ContextOutput, ContextReadResult, Error,
    observation::{Buffer, ReadResult},
};
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null_mut,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

struct SignalState {
    buffer: Buffer,
    signalled: bool,
}
struct Signal {
    event: OwnedHandle,
    state: Mutex<SignalState>,
}
impl Signal {
    fn refresh(&self, state: &mut SignalState) {
        // Queue state and event transitions share one lock. Signal only on an
        // edge: batches need no repeated kernel calls while readiness stays true.
        let ready = state.buffer.ready();
        if ready == state.signalled {
            return;
        }
        state.signalled = ready;
        unsafe {
            if ready {
                SetEvent(self.event.as_raw_handle());
            } else {
                ResetEvent(self.event.as_raw_handle());
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct ObservationSender {
    signal: Arc<Signal>,
}
pub(crate) struct ObservationReceiver {
    signal: Arc<Signal>,
}

pub(crate) fn channel(capacity: usize) -> Result<(ObservationSender, ObservationReceiver), Error> {
    let event = unsafe { CreateEventW(null_mut(), 1, 0, std::ptr::null()) };
    if event.is_null() {
        return Err(Error::System(unsafe { GetLastError() }));
    }
    let signal = Arc::new(Signal {
        event: unsafe { OwnedHandle::from_raw_handle(event) },
        state: Mutex::new(SignalState {
            buffer: Buffer::new(capacity),
            signalled: false,
        }),
    });
    Ok((
        ObservationSender {
            signal: Arc::clone(&signal),
        },
        ObservationReceiver { signal },
    ))
}
impl ObservationSender {
    pub(crate) fn send(&self, output: ContextOutput) -> Result<(), Error> {
        let mut state = self.signal.state.lock().unwrap_or_else(|e| e.into_inner());
        let result = state.buffer.send(output);
        self.signal.refresh(&mut state);
        result
    }
    pub(crate) fn finish(&self, outcome: Result<(), Error>) {
        let mut state = self.signal.state.lock().unwrap_or_else(|e| e.into_inner());
        state.buffer.finish(outcome);
        self.signal.refresh(&mut state);
    }
}
impl ObservationReceiver {
    pub(crate) fn outcome(&self) -> Option<Result<(), Error>> {
        self.signal
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .buffer
            .outcome()
    }
    pub(crate) fn try_read(&self) -> Result<ReadResult, Error> {
        self.try_read_with_context().map(|result| match result {
            ContextReadResult::Frame(frame) => ReadResult::Frame(frame.output),
            ContextReadResult::Pending => ReadResult::Pending,
            ContextReadResult::Completed => ReadResult::Completed,
        })
    }
    pub(crate) fn try_read_with_context(&self) -> Result<ContextReadResult, Error> {
        let mut state = self.signal.state.lock().unwrap_or_else(|e| e.into_inner());
        let result = state.buffer.take();
        self.signal.refresh(&mut state);
        result
    }
    pub(crate) fn duplicate_handle(&self) -> Result<OwnedHandle, Error> {
        let mut duplicate = null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.signal.event.as_raw_handle(),
                GetCurrentProcess(),
                &mut duplicate,
                SYNCHRONIZATION_SYNCHRONIZE,
                0,
                0,
            )
        } == 0
        {
            return Err(Error::System(unsafe { GetLastError() }));
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(duplicate) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pointer_gesture_recognition::{Configuration, Engine, Event};
    fn frame() -> ContextOutput {
        let output = Engine::new(Configuration {
            start_distance: 10.,
            change_distance: 5.,
            total_duration_ns: 0,
            idle_duration_ns: 0,
            maximum_points: 8,
            maximum_directions: 8,
            button: 1,
            passes_through_empty_matcher: true,
        })
        .unwrap()
        .step(Event::Start, 0);
        ContextOutput {
            output,
            epoch: None,
        }
    }
    #[test]
    fn bounded_readiness_drains_frames_before_success_or_failure_and_outlives_owner() {
        for result in [Ok(()), Err(Error::ConsumerLagged)] {
            let (sender, receiver) = channel(1).unwrap();
            let notification = receiver.duplicate_handle().unwrap();
            assert_eq!(
                unsafe { WaitForSingleObject(notification.as_raw_handle(), 0) },
                WAIT_TIMEOUT
            );
            sender.send(frame()).unwrap();
            if result.is_err() {
                assert_eq!(sender.send(frame()), Err(Error::ConsumerLagged));
            }
            sender.finish(result);
            assert!(matches!(receiver.try_read().unwrap(), ReadResult::Frame(_)));
            if result.is_err() {
                assert!(matches!(receiver.try_read().unwrap(), ReadResult::Frame(_)));
            }
            assert_eq!(
                receiver
                    .try_read()
                    .map(|o| matches!(o, ReadResult::Completed)),
                result.map(|()| true)
            );
            assert_eq!(
                unsafe { WaitForSingleObject(notification.as_raw_handle(), 0) },
                WAIT_OBJECT_0
            );
            // The exported duplicate grants waiting, not resetting shared state.
            assert_eq!(unsafe { ResetEvent(notification.as_raw_handle()) }, 0);
            drop(receiver);
            drop(sender);
            assert_eq!(
                unsafe { WaitForSingleObject(notification.as_raw_handle(), 0) },
                WAIT_OBJECT_0
            );
        }
    }
    #[test]
    fn notification_cannot_lose_racing_sends_or_terminal_wakeup() {
        let (sender, receiver) = channel(256).unwrap();
        let notification = receiver.duplicate_handle().unwrap();
        let worker = std::thread::spawn(move || {
            for _ in 0..128 {
                sender.send(frame()).unwrap();
                std::thread::yield_now();
            }
            sender.finish(Ok(()));
        });
        let mut read = 0;
        loop {
            assert_eq!(
                unsafe { WaitForSingleObject(notification.as_raw_handle(), 2000) },
                WAIT_OBJECT_0
            );
            loop {
                match receiver.try_read().unwrap() {
                    ReadResult::Frame(_) => read += 1,
                    ReadResult::Pending => break,
                    ReadResult::Completed => {
                        worker.join().unwrap();
                        assert_eq!(read, 128);
                        return;
                    }
                }
            }
        }
    }
}
