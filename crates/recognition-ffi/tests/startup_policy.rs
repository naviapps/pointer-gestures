// Exercise the production wire adapter without changing how its cdylib is built.
#[path = "../src/startup.rs"]
mod startup;

use pointer_gesture_recognition::Startup;
use startup::*;
use std::ptr::{null, null_mut};

struct OwnedStartup(*mut Startup);

impl OwnedStartup {
    fn new() -> Self {
        let handle = pointer_gesture_recognition_startup_create();
        assert!(!handle.is_null());
        Self(handle)
    }

    fn view(&self, lifecycle: u32, requested: u32) -> u64 {
        let view = unsafe { pointer_gesture_recognition_startup_view(self.0) };
        assert_eq!((view.lifecycle, view.requested), (lifecycle, requested));
        view.revision
    }

    fn request(&mut self, kind: u32, revision: u64) -> u32 {
        unsafe { pointer_gesture_recognition_startup_request(self.0, kind, revision) }
    }

    fn set_delays(&mut self, delays: &[f64]) -> u32 {
        unsafe {
            pointer_gesture_recognition_startup_set_delays(self.0, delays.as_ptr(), delays.len())
        }
    }

    fn delay(&mut self) -> Option<u64> {
        let mut delay = 89;
        match unsafe { pointer_gesture_recognition_startup_next_delay(self.0, &mut delay) } {
            0 => {
                assert_eq!(delay, 89);
                None
            }
            1 => Some(delay),
            other => panic!("invalid next-delay status: {other}"),
        }
    }
}

impl Drop for OwnedStartup {
    fn drop(&mut self) {
        unsafe { pointer_gesture_recognition_startup_destroy(self.0) }
    }
}

#[test]
fn startup_requests_and_delays_preserve_the_wire_lifecycle() {
    let mut startup = OwnedStartup::new();
    let idle = startup.view(0, 0);
    assert_eq!(startup.delay(), None);
    assert_eq!(startup.request(3, idle), 0);
    assert_eq!(startup.request(u32::MAX, idle), 0);
    assert_eq!(startup.view(0, 0), idle);

    let mut delays = [0.125];
    assert_eq!(startup.set_delays(&delays), 1);
    delays.fill(f64::NAN);
    assert_eq!(startup.set_delays(&delays), 0);
    assert_eq!(startup.request(1, idle), 1);
    let attempt = startup.view(1, 1);
    assert_ne!(attempt, idle);
    assert_eq!(startup.request(5, idle), 0);
    assert_eq!(startup.view(1, 1), attempt);

    assert_eq!(startup.request(6, attempt), 1);
    assert_eq!(startup.view(3, 1), attempt);
    assert_eq!(startup.delay(), Some(125_000_000));
    assert_eq!(startup.request(4, attempt), 1);
    assert_eq!(startup.request(5, attempt), 1);
    assert_eq!(startup.view(2, 1), attempt);

    assert_eq!(startup.request(2, attempt), 1);
    let stopped = startup.view(0, 0);
    assert_ne!(stopped, attempt);
    assert_eq!(startup.delay(), None);
    unsafe { pointer_gesture_recognition_startup_destroy(null_mut()) }
}

#[test]
fn replacing_a_pending_schedule_exposes_failed_and_retrying_states() {
    let mut startup = OwnedStartup::new();
    assert_eq!(startup.set_delays(&[0.125]), 1);
    assert_eq!(startup.request(1, 0), 1);
    let first = startup.view(1, 1);
    assert_eq!(startup.request(6, first), 1);

    assert_eq!(
        unsafe { pointer_gesture_recognition_startup_set_delays(startup.0, null(), 0) },
        2
    );
    let failed = startup.view(4, 1);
    assert_ne!(failed, first);
    assert_eq!(startup.set_delays(&[f64::MAX]), 2);
    let pending = startup.view(3, 1);
    assert_ne!(pending, failed);
    assert_eq!(startup.delay(), Some(u64::MAX));
    assert_eq!(startup.request(3, pending), 1);
    let immediate = startup.view(1, 1);
    assert_ne!(immediate, pending);
}

#[test]
fn retry_validation_reports_the_first_invalid_index() {
    assert_eq!(
        unsafe { pointer_gesture_recognition_validate_retry_delays(null(), 0) },
        usize::MAX
    );
    for (delays, expected) in [
        (&[0.0, 0.5, 0.5][..], usize::MAX),
        (&[f64::NAN, -1.0][..], 0),
        (&[0.0, f64::INFINITY, -1.0][..], 1),
        (&[0.0, -0.0, -1.0][..], 2),
    ] {
        assert_eq!(
            unsafe {
                pointer_gesture_recognition_validate_retry_delays(delays.as_ptr(), delays.len())
            },
            expected
        );
    }
}
