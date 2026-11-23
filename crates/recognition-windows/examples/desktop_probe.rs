//! Explicit live-input verification. Run only inside an owned isolated Windows
//! desktop: this installs hooks and injects input into an owned receiver process.

#[cfg(not(windows))]
fn main() {
    panic!("desktop_probe requires an isolated Windows desktop");
}

#[cfg(windows)]
#[path = "desktop_probe/receiver.rs"]
mod receiver;

#[cfg(windows)]
mod probe {
    use super::receiver::Receiver;
    use pointer_gesture_recognition::{
        Configuration, Direction, Effect, InputStage, Matcher, Output, SessionOutcome,
    };
    use pointer_gesture_recognition_windows::{
        ContextOutput, ContextReadResult, DiagnosticPhase, DiagnosticReader, Error, Policy,
        PressContext, ReadResult, Recognizer, SessionContext,
    };
    use std::cell::Cell;
    use std::io::Write;
    use std::os::windows::io::AsRawHandle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            SystemInformation::GetTickCount,
            Threading::{GetCurrentProcess, GetProcessHandleCount, WaitForSingleObject},
        },
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };

    fn send(mouse: Option<u32>, key: Option<(u16, bool)>) {
        let input = if let Some(flag) = mouse {
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0x54455354,
                    },
                },
            }
        } else {
            let (value, down) = key.unwrap();
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: value,
                        wScan: 0,
                        dwFlags: (if down { 0 } else { KEYEVENTF_KEYUP })
                            | if matches!(value, VK_RCONTROL | VK_RMENU | VK_LWIN | VK_RWIN) {
                                KEYEVENTF_EXTENDEDKEY
                            } else {
                                0
                            },
                        time: 0,
                        dwExtraInfo: 0x54455354,
                    },
                },
            }
        };
        assert_eq!(
            unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) },
            1
        );
    }
    fn pump(adapter: &Recognizer, outputs: &mut Vec<Output>, milliseconds: u64) {
        pump_context(adapter, milliseconds, |frame| outputs.push(frame.output));
    }
    fn pump_context(
        adapter: &Recognizer,
        milliseconds: u64,
        mut receive: impl FnMut(ContextOutput),
    ) {
        let until = Instant::now() + Duration::from_millis(milliseconds);
        let notification = adapter.duplicate_notification_handle().unwrap();
        while Instant::now() < until {
            let mut message = unsafe { std::mem::zeroed::<MSG>() };
            unsafe {
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            loop {
                match adapter.try_read_with_context().unwrap() {
                    ContextReadResult::Frame(frame) => {
                        assert!(frame.output.effects.iter().all(|effect| matches!(
                            effect,
                            Effect::Match(_) | Effect::Publish { .. }
                        )));
                        receive(frame);
                    }
                    ContextReadResult::Pending => break,
                    ContextReadResult::Completed => return,
                }
            }
            let remaining = until
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(u128::from(u32::MAX)) as u32;
            let handle = notification.as_raw_handle();
            unsafe {
                MsgWaitForMultipleObjectsEx(
                    1,
                    &handle,
                    remaining,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
    }
    fn expect_counts(
        receiver: &mut Receiver,
        adapter: &Recognizer,
        outputs: &mut Vec<Output>,
        expected: (usize, usize),
        message: &str,
    ) {
        expect_button_counts(receiver, 1, adapter, outputs, expected, message);
    }
    fn expect_button_counts(
        receiver: &mut Receiver,
        button: u32,
        adapter: &Recognizer,
        outputs: &mut Vec<Output>,
        expected: (usize, usize),
        message: &str,
    ) {
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            pump(adapter, outputs, 5);
            if receiver.counts_for(button) == expected
                && outputs.last().is_some_and(|o| o.input_generation == 0)
            {
                break;
            }
        }
        pump(adapter, outputs, 20);
        let actual = receiver.counts_for(button);
        let state = receiver.snapshot();
        assert_eq!(
            actual, expected,
            "{message}; receiver={:?}, receiver capture={:?}, foreground={:?}, events={:?}",
            receiver.window, state.capture, state.foreground, state.events
        );
    }
    fn point_input(x: i32, y: i32, flags: u32, time: u32) -> INPUT {
        let (ox, oy, w, h) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: ((f64::from(x - ox) * 65535.) / f64::from(w - 1)).round() as i32,
                    dy: ((f64::from(y - oy) * 65535.) / f64::from(h - 1)).round() as i32,
                    mouseData: 0,
                    dwFlags: flags
                        | MOUSEEVENTF_MOVE
                        | MOUSEEVENTF_ABSOLUTE
                        | MOUSEEVENTF_VIRTUALDESK,
                    time,
                    dwExtraInfo: 0x54455354,
                },
            },
        }
    }
    fn send_inputs(inputs: &[INPUT]) {
        assert_eq!(
            unsafe {
                SendInput(
                    inputs.len() as u32,
                    inputs.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                )
            },
            inputs.len() as u32,
            "owned fixture input batch was not completely inserted"
        );
    }
    fn move_input(x: i32, y: i32) {
        send_inputs(&[point_input(x, y, 0, 0)]);
    }
    fn point(x: i32, y: i32, adapter: &Recognizer, outputs: &mut Vec<Output>) {
        move_input(x, y);
        pump(adapter, outputs, 20);
    }
    fn button_input(button: u32, down: bool, time: u32) -> INPUT {
        let (flag, data) = match (button, down) {
            (0, true) => (MOUSEEVENTF_LEFTDOWN, 0),
            (0, false) => (MOUSEEVENTF_LEFTUP, 0),
            (1, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
            (1, false) => (MOUSEEVENTF_RIGHTUP, 0),
            (2, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
            (2, false) => (MOUSEEVENTF_MIDDLEUP, 0),
            (3 | 4, true) => (MOUSEEVENTF_XDOWN, button - 2),
            (3 | 4, false) => (MOUSEEVENTF_XUP, button - 2),
            _ => panic!("invalid fixture button"),
        };
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    mouseData: data,
                    dwFlags: flag,
                    time,
                    dwExtraInfo: 0x54455354,
                    ..unsafe { std::mem::zeroed() }
                },
            },
        }
    }
    fn send_button(button: u32, down: bool) -> u32 {
        let time = unsafe { GetTickCount() };
        send_inputs(&[button_input(button, down, time)]);
        time
    }
    fn down(adapter: &Recognizer, outputs: &mut Vec<Output>) {
        button_down(1, adapter, outputs);
    }
    fn button_down(button: u32, adapter: &Recognizer, outputs: &mut Vec<Output>) {
        let previous = outputs.len();
        send_button(button, true);
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            pump(adapter, outputs, 5);
            if outputs[previous..]
                .last()
                .is_some_and(|o| o.input_generation != 0)
            {
                return;
            }
        }
        panic!("button press was not captured by the fixture");
    }
    fn up(adapter: &Recognizer, outputs: &mut Vec<Output>) {
        send(Some(MOUSEEVENTF_RIGHTUP), None);
        pump(adapter, outputs, 200);
    }

    fn expect_target_changed(adapter: &mut Recognizer) {
        let notification = adapter.duplicate_notification_handle().unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        let mut target_changed = false;
        while Instant::now() < until {
            match adapter.try_read() {
                Ok(ReadResult::Frame(_)) => {}
                Ok(ReadResult::Pending) => unsafe {
                    WaitForSingleObject(notification.as_raw_handle(), 2000);
                },
                Ok(ReadResult::Completed) => break,
                Err(Error::ReplayTargetChanged) => {
                    target_changed = true;
                    break;
                }
                Err(error) => panic!("unexpected recovery failure: {error}"),
            }
        }
        assert!(target_changed, "replaced target must be reported");
        assert_eq!(adapter.stop(), Err(Error::ReplayTargetChanged));
    }
    fn handles() -> u32 {
        let mut count = 0;
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
            0
        );
        count
    }
    fn activate(receiver: &Receiver) {
        assert_ne!(
            unsafe {
                SetWindowPos(
                    receiver.window,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                )
            },
            0
        );
        move_input(receiver.center.x, receiver.center.y);
        send_button(0, true);
        send_button(0, false);
        unsafe {
            SetForegroundWindow(receiver.window);
        }
        let until = Instant::now() + Duration::from_secs(3);
        while unsafe { GetForegroundWindow() } != receiver.window && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            unsafe { GetForegroundWindow() },
            receiver.window,
            "owned receiver must finish activation before input"
        );
    }

    fn duplicate_down(receiver: &mut Receiver, matcher: &Matcher, config: Configuration) {
        let contexts = Arc::new(AtomicU32::new(0));
        let resolved = Arc::clone(&contexts);
        let matcher = matcher.clone();
        let mut adapter = Recognizer::start_with_context(config, move |_| SessionContext {
            matcher: matcher.clone(),
            policy: Policy::default(),
            epoch: u64::from(resolved.fetch_add(1, Ordering::SeqCst) + 1),
        })
        .unwrap();
        let mut out = vec![];
        let (x, y) = (receiver.center.x, receiver.center.y);
        point(x, y, &adapter, &mut out);
        receiver.clear();
        out.clear();
        down(&adapter, &mut out);
        let pending = out.last().unwrap();
        assert_eq!(pending.stage, InputStage::Pending);
        let generation = pending.input_generation;
        assert_ne!(generation, 0);
        assert_eq!(contexts.load(Ordering::SeqCst), 1);
        assert_eq!(receiver.counts(), (0, 0));

        let previous = out.len();
        send_button(1, true);
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            pump(&adapter, &mut out, 5);
            if out[previous..].iter().any(|output| {
                output.stage == InputStage::Idle
                    && output.input_generation == 0
                    && output.last_session.as_ref().is_some_and(|report| {
                        report.generation == generation
                            && report.outcome == SessionOutcome::Cancelled
                            && !report.captured
                    })
            }) {
                break;
            }
            assert!(
                Instant::now() < until,
                "duplicate down did not cancel its pending generation"
            );
        }
        assert_eq!(
            contexts.load(Ordering::SeqCst),
            1,
            "duplicate down resolved another host context"
        );
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 0),
            "duplicate pending down restores only the original native down",
        );
        point(x + 60, y, &adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 1),
            "duplicate pending down keeps one balanced native stream",
        );
        assert_eq!(contexts.load(Ordering::SeqCst), 1);
        assert!(
            out[previous..].iter().all(|output| {
                output.input_generation == 0
                    && !output
                        .effects
                        .iter()
                        .any(|effect| matches!(effect, Effect::Match(_)))
            }),
            "duplicate down began another generation or matched before the balancing up"
        );

        point(x, y, &adapter, &mut out);
        out.clear();
        down(&adapter, &mut out);
        assert_eq!(out.last().unwrap().input_generation, generation + 1);
        assert_eq!(contexts.load(Ordering::SeqCst), 2);
        point(x + 60, y, &adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 1),
            "a fresh gesture after duplicate-down recovery stays hidden",
        );
        assert_eq!(
            out.iter()
                .filter(|output| output.effects.contains(&Effect::Match(42)))
                .count(),
            1
        );
        adapter.stop().unwrap();
    }

    #[derive(Clone, Copy, Debug)]
    enum HeldReleaseMode {
        ButtonOnly,
        CompoundSamePoint,
        CompoundMovedPoint,
        SeparateMoveUp,
    }
    impl HeldReleaseMode {
        const ALL: [Self; 4] = [
            Self::ButtonOnly,
            Self::CompoundSamePoint,
            Self::CompoundMovedPoint,
            Self::SeparateMoveUp,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::ButtonOnly => "button-only",
                Self::CompoundSamePoint => "compound-same-point",
                Self::CompoundMovedPoint => "compound-move-1px",
                Self::SeparateMoveUp => "separate-move-1px-up-batch",
            }
        }

        fn release(self, x: i32, y: i32) -> (u32, Duration) {
            let time = unsafe { GetTickCount() };
            let first = match self {
                Self::ButtonOnly => button_input(1, false, time),
                Self::CompoundSamePoint => point_input(x, y, MOUSEEVENTF_RIGHTUP, time),
                Self::CompoundMovedPoint => point_input(x + 1, y, MOUSEEVENTF_RIGHTUP, time),
                // Same one-pixel movement as the compound case, but distinct
                // events in one serial SendInput batch.
                Self::SeparateMoveUp => point_input(x + 1, y, 0, time),
            };
            let inputs = [first, button_input(1, false, time)];
            let count = if matches!(self, Self::SeparateMoveUp) {
                2
            } else {
                1
            };
            // Exclude packet construction and logging: this measures the original
            // sender's synchronous SendInput call, alongside SDK replay timings.
            let began = Instant::now();
            send_inputs(&inputs[..count]);
            (time, began.elapsed())
        }
    }

    struct FailureDiagnostics {
        reader: DiagnosticReader,
        mode: HeldReleaseMode,
        cycle: Cell<usize>,
        held: Cell<bool>,
    }
    impl Drop for FailureDiagnostics {
        fn drop(&mut self) {
            if std::thread::panicking() {
                // Capture the original failure before cleanup emits any input.
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "held-cancel mode={} cycle={} native diagnostics before cleanup: {:?}",
                    self.mode.label(),
                    self.cycle.get(),
                    self.reader.snapshot()
                );
            }
            if self.held.replace(false) {
                let input = button_input(1, false, unsafe { GetTickCount() });
                // Best-effort release during unwind must not replace the original
                // assertion with a second panic or leave a physical button held.
                unsafe {
                    SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
                }
            }
        }
    }

    fn resolver(
        matcher: &Matcher,
        contexts: &Arc<AtomicU32>,
        enabled: bool,
    ) -> impl FnMut(PressContext) -> SessionContext + Send + 'static {
        let matcher = matcher.clone();
        let contexts = Arc::clone(contexts);
        move |_| SessionContext {
            matcher: matcher.clone(),
            policy: Policy {
                enabled,
                required_modifiers: 0,
            },
            epoch: u64::from(contexts.fetch_add(1, Ordering::SeqCst) + 1),
        }
    }

    fn wait_context(
        adapter: &Recognizer,
        frames: &mut Vec<ContextOutput>,
        description: &str,
        mut ready: impl FnMut(&[ContextOutput]) -> bool,
    ) {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            pump_context(adapter, 5, |frame| frames.push(frame));
            if ready(frames) {
                return;
            }
            assert!(
                Instant::now() < until,
                "{description}; last frame={:?}",
                frames.last()
            );
        }
    }

    fn expect_replayed_buttons(
        receiver: &mut Receiver,
        adapter: &Recognizer,
        frames: &mut Vec<ContextOutput>,
        expected: &[(u32, u32)],
        counts: (usize, usize),
    ) {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            pump_context(adapter, 5, |frame| frames.push(frame));
            let state = receiver.snapshot();
            let actual = (
                state
                    .events
                    .iter()
                    .filter(|event| event.message == WM_RBUTTONDOWN)
                    .count(),
                state
                    .events
                    .iter()
                    .filter(|event| event.message == WM_RBUTTONUP)
                    .count(),
            );
            assert!(
                actual.0 <= counts.0 && actual.1 <= counts.1,
                "held-cancel button counts exceeded {counts:?}; receiver={state:?}"
            );
            assert!(
                state.direct_buttons.len() <= expected.len()
                    && state.removed_buttons.len() <= expected.len(),
                "held-cancel receiver queue exceeded {} buttons; receiver={state:?}",
                expected.len()
            );
            for (index, direct) in state.direct_buttons.iter().enumerate() {
                assert_eq!(
                    (direct.message, direct.extra),
                    (expected[index].0, 0x50475257),
                    "held-cancel WH_MOUSE provenance mismatch at {index}; receiver={state:?}"
                );
            }
            for (index, removed) in state.removed_buttons.iter().enumerate() {
                assert_eq!(
                    (removed.message, removed.time),
                    expected[index],
                    "held-cancel PM_REMOVE timestamp mismatch at {index}; receiver={state:?}"
                );
            }
            if actual == counts
                && state.direct_buttons.len() == expected.len()
                && state.removed_buttons.len() == expected.len()
            {
                if counts.0 == counts.1 {
                    assert!(state.capture.is_null(), "held-cancel receiver kept capture");
                    assert_eq!(
                        unsafe { GetAsyncKeyState(i32::from(VK_RBUTTON)) } & i16::MIN,
                        0,
                        "held-cancel left the right button down"
                    );
                }
                return;
            }
            assert!(
                Instant::now() < until,
                "held-cancel did not deliver {counts:?} and {} queued buttons; receiver={state:?}",
                expected.len()
            );
        }
    }

    fn held_cancel(
        receiver: &mut Receiver,
        matcher: &Matcher,
        config: Configuration,
        mode: HeldReleaseMode,
        cycles: usize,
    ) {
        assert!((8..=32).contains(&cycles));
        println!(
            "{{\"heldReleaseMode\":\"{}\",\"startingCycles\":{cycles}}}",
            mode.label()
        );
        std::io::stdout().flush().unwrap();
        let contexts = Arc::new(AtomicU32::new(0));
        let mut adapter =
            Recognizer::start_with_context(config, resolver(matcher, &contexts, true)).unwrap();
        adapter.set_diagnostics_enabled(true);
        let reader = adapter.diagnostic_reader();
        let failure = FailureDiagnostics {
            reader: reader.clone(),
            mode,
            cycle: Cell::new(0),
            held: Cell::new(false),
        };
        let mut frames = vec![];
        let mut expected = vec![];
        let (x, y) = (receiver.center.x, receiver.center.y);
        move_input(x, y);
        pump_context(&adapter, 20, |frame| frames.push(frame));
        receiver.clear();
        for cycle in 1..=cycles {
            failure.cycle.set(cycle);
            move_input(x, y);
            pump_context(&adapter, 20, |frame| frames.push(frame));
            let epoch = u64::from(contexts.load(Ordering::SeqCst) + 1);
            let first = frames.len();
            let down_time = send_button(1, true);
            failure.held.set(true);
            wait_context(
                &adapter,
                &mut frames,
                "fresh held press was not Pending",
                |frames| {
                    frames[first..].iter().any(|frame| {
                        frame.epoch == Some(epoch)
                            && frame.output.stage == InputStage::Pending
                            && frame.output.input_generation != 0
                    })
                },
            );
            let generation = frames[first..]
                .iter()
                .find(|frame| {
                    frame.epoch == Some(epoch) && frame.output.stage == InputStage::Pending
                })
                .unwrap()
                .output
                .input_generation;
            assert_eq!(u64::from(contexts.load(Ordering::SeqCst)), epoch);
            assert_eq!(receiver.counts(), (cycle - 1, cycle - 1));
            adapter.cancel().unwrap();
            adapter
                .set_context_resolver(resolver(matcher, &contexts, false))
                .unwrap();
            wait_context(
                &adapter,
                &mut frames,
                "held cancellation did not complete the original generation",
                |frames| {
                    frames[first..].iter().any(|frame| {
                        frame.epoch == Some(epoch)
                            && frame.output.stage == InputStage::Idle
                            && frame.output.input_generation == 0
                            && frame.output.last_session.as_ref().is_some_and(|report| {
                                report.generation == generation
                                    && report.outcome == SessionOutcome::Cancelled
                                    && !report.captured
                            })
                    })
                },
            );
            expected.push((WM_RBUTTONDOWN, down_time));
            expect_replayed_buttons(
                receiver,
                &adapter,
                &mut frames,
                &expected,
                (cycle, cycle - 1),
            );
            let (up_time, release_duration) = mode.release(x, y);
            failure.held.set(false);
            let _ = writeln!(
                std::io::stderr().lock(),
                "held-release mode={} cycle={cycle} event_time_ms={up_time} send_input_duration_ns={}",
                mode.label(),
                release_duration.as_nanos()
            );
            expected.push((WM_RBUTTONUP, up_time));
            expect_replayed_buttons(receiver, &adapter, &mut frames, &expected, (cycle, cycle));
            assert_eq!(u64::from(contexts.load(Ordering::SeqCst)), epoch);
            assert!(
                frames[first..]
                    .iter()
                    .all(|frame| frame.epoch == Some(epoch)
                        && (frame.output.input_generation == 0
                            || frame.output.input_generation == generation)
                        && !frame
                            .output
                            .effects
                            .iter()
                            .any(|effect| matches!(effect, Effect::Match(_)))),
                "held-cancel cycle {cycle} changed context/generation or matched"
            );

            adapter
                .set_context_resolver(resolver(matcher, &contexts, true))
                .unwrap();
            let next = frames.len();
            send_button(1, true);
            failure.held.set(true);
            wait_context(
                &adapter,
                &mut frames,
                "fresh press after held cancellation did not start",
                |frames| {
                    frames[next..].iter().any(|frame| {
                        frame.epoch == Some(epoch + 1)
                            && frame.output.stage == InputStage::Pending
                            && frame.output.input_generation == generation + 1
                    })
                },
            );
            assert_eq!(u64::from(contexts.load(Ordering::SeqCst)), epoch + 1);
            move_input(x + 60, y);
            wait_context(
                &adapter,
                &mut frames,
                "fresh press after held cancellation did not become Active",
                |frames| {
                    frames[next..].iter().any(|frame| {
                        frame.epoch == Some(epoch + 1)
                            && frame.output.stage == InputStage::Active
                            && frame.output.input_generation == generation + 1
                    })
                },
            );
            send_button(1, false);
            failure.held.set(false);
            wait_context(
                &adapter,
                &mut frames,
                "fresh gesture after held cancellation did not match",
                |frames| {
                    frames[next..].iter().any(|frame| {
                        frame.epoch == Some(epoch + 1)
                            && frame.output.stage == InputStage::Idle
                            && frame.output.last_session.as_ref().is_some_and(|report| {
                                report.generation == generation + 1
                                    && report.outcome == SessionOutcome::Matched
                                    && report.captured
                            })
                            && frame.output.effects.contains(&Effect::Match(42))
                    })
                },
            );
            expect_replayed_buttons(receiver, &adapter, &mut frames, &expected, (cycle, cycle));
            assert_eq!(
                frames[next..]
                    .iter()
                    .filter(|frame| frame.output.effects.contains(&Effect::Match(42)))
                    .count(),
                1,
                "held-cancel cycle {cycle} produced more than one fresh match"
            );
        }
        adapter.stop().unwrap();
        drop(adapter);
        let diagnostics = reader.snapshot();
        assert!(diagnostics.records_written > 64 && diagnostics.overwritten > 0);
        assert_eq!(diagnostics.unavailable, 0);
        assert!(diagnostics.records.len() <= 64);
        let sequence = |phase| {
            diagnostics
                .records
                .iter()
                .find(|record| record.phase == phase)
                .unwrap_or_else(|| {
                    panic!("missing terminal {phase:?}; diagnostics={diagnostics:?}")
                })
                .sequence
        };
        assert!(
            sequence(DiagnosticPhase::HooksRemoving) < sequence(DiagnosticPhase::HooksRemoved)
                && sequence(DiagnosticPhase::HooksRemoved)
                    < sequence(DiagnosticPhase::WorkerFinished),
            "native diagnostics teardown ordering differed: {diagnostics:?}"
        );
        println!(
            "{{\"heldReleaseMode\":\"{}\",\"heldCancelCycles\":{cycles},\"exactReplayButtonPairs\":{cycles},\"freshMatches\":{cycles}}}",
            mode.label()
        );
    }

    fn held_release_matrix(
        receiver: &mut Receiver,
        matcher: &Matcher,
        config: Configuration,
        cycles: usize,
    ) {
        for mode in HeldReleaseMode::ALL {
            held_cancel(receiver, matcher, config, mode, cycles);
        }
    }

    static EXTERNAL_ESCAPE: AtomicU32 = AtomicU32::new(0);
    unsafe extern "system" fn observe_key(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
        if code >= 0 {
            let packet = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
            if packet.vkCode == u32::from(VK_ESCAPE) {
                EXTERNAL_ESCAPE.fetch_or(
                    if matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
                        2
                    } else {
                        1
                    } | if packet.flags & LLKHF_INJECTED != 0 {
                        4
                    } else {
                        0
                    },
                    Ordering::Relaxed,
                );
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, data) }
    }
    struct KeyObserver(HHOOK);
    impl Drop for KeyObserver {
        fn drop(&mut self) {
            unsafe {
                UnhookWindowsHookEx(self.0);
            }
        }
    }
    struct DisplayMode(DEVMODEW);
    impl DisplayMode {
        fn read(index: u32) -> Option<DEVMODEW> {
            let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
            mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            (unsafe { EnumDisplaySettingsW(std::ptr::null(), index, &mut mode) } != 0)
                .then_some(mode)
        }
        fn set(mode: &DEVMODEW, flags: u32) {
            assert_eq!(
                unsafe {
                    ChangeDisplaySettingsExW(
                        std::ptr::null(),
                        mode,
                        std::ptr::null_mut(),
                        flags,
                        std::ptr::null(),
                    )
                },
                DISP_CHANGE_SUCCESSFUL,
                "owned guest display mode change"
            );
        }
    }
    impl Drop for DisplayMode {
        fn drop(&mut self) {
            Self::set(&self.0, CDS_FULLSCREEN);
        }
    }

    // Additional machine-level acceptance is deliberately separate from the
    // ordinary input regression. A controller owns this directory and VM lease,
    // provides real virtual-key events and resumes the same guest process.
    fn acceptance(
        receiver: &mut Receiver,
        matcher: &Matcher,
        mut config: Configuration,
        directory: &str,
    ) {
        let directory = std::path::Path::new(directory);
        let stage = |value: &str| {
            std::fs::write(directory.join("stage.txt"), value).unwrap();
            println!("{{\"stage\":\"{value}\"}}");
            std::io::stdout().flush().unwrap();
        };
        activate(receiver);
        config.idle_duration_ns = 30_000_000_000;
        config.total_duration_ns = 60_000_000_000;
        let mut adapter = Recognizer::start(config, matcher).unwrap();
        let mut out = vec![];
        let observer = KeyObserver(unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(observe_key), std::ptr::null_mut(), 0)
        });
        assert!(!observer.0.is_null());
        receiver.clear();
        down(&adapter, &mut out);
        stage("external-escape");
        let until = Instant::now() + Duration::from_secs(45);
        while EXTERNAL_ESCAPE.load(Ordering::Relaxed) & 3 != 3 && Instant::now() < until {
            pump(&adapter, &mut out, 10);
        }
        assert_eq!(
            EXTERNAL_ESCAPE.load(Ordering::Relaxed),
            3,
            "external key must reach hooks without LLKHF_INJECTED"
        );
        drop(observer);
        up(&adapter, &mut out);
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 1),
            "virtual HID Escape restores pending click",
        );
        assert!(!receiver.snapshot().events.iter().any(|e| matches!(
            e.message,
            WM_KEYDOWN | WM_KEYUP
        ) && e.wparam
            == usize::from(VK_ESCAPE)));
        receiver.clear();
        out.clear();
        down(&adapter, &mut out);
        stage("suspend");
        let until = Instant::now() + Duration::from_secs(60);
        while !directory.join("resumed.txt").exists() && Instant::now() < until {
            pump(&adapter, &mut out, 10);
        }
        assert!(
            directory.join("resumed.txt").exists(),
            "controller did not resume the owned guest"
        );
        up(&adapter, &mut out);
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 1),
            "pending click survives VM suspend/resume",
        );

        let original = DisplayMode::read(ENUM_CURRENT_SETTINGS).unwrap();
        let changed = (0..)
            .map_while(DisplayMode::read)
            .find(|mode| {
                mode.dmBitsPerPel == original.dmBitsPerPel
                    && mode.dmPelsWidth >= 1024
                    && mode.dmPelsHeight >= 720
                    && (mode.dmPelsWidth, mode.dmPelsHeight)
                        != (original.dmPelsWidth, original.dmPelsHeight)
            })
            .expect("owned guest has no alternate display mode");
        DisplayMode::set(&changed, CDS_TEST);
        let restore = DisplayMode(original);
        DisplayMode::set(&changed, CDS_FULLSCREEN);
        pump(&adapter, &mut out, 200);
        assert_eq!(
            unsafe { GetSystemMetrics(SM_CXSCREEN) },
            changed.dmPelsWidth as i32
        );
        receiver.frame(
            10,
            10,
            changed.dmPelsWidth as i32 - 20,
            changed.dmPelsHeight as i32 - 80,
        );
        activate(receiver);
        let (x, y) = (
            changed.dmPelsWidth as i32 - 100,
            changed.dmPelsHeight as i32 - 180,
        );
        point(x, y, &adapter, &mut out);
        receiver.clear();
        down(&adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            receiver,
            &adapter,
            &mut out,
            (1, 1),
            "replay after display resolution change",
        );
        let replay = receiver.snapshot();
        let event = replay
            .events
            .iter()
            .find(|e| e.message == WM_RBUTTONDOWN)
            .unwrap();
        assert!(
            (event.x - x).abs() <= 1 && (event.y - y).abs() <= 1,
            "changed display projection: {event:?}"
        );
        drop(restore);
        pump(&adapter, &mut out, 200);
        receiver.frame(40, 40, 700, 450);
        activate(receiver);
        let (x, y) = (receiver.center.x, receiver.center.y);
        let started = Instant::now();
        let mut matches = 0;
        for index in 0..256 {
            point(x, y, &adapter, &mut out);
            receiver.clear();
            out.clear();
            down(&adapter, &mut out);
            if index % 2 == 1 {
                point(x + 60, y, &adapter, &mut out);
            }
            up(&adapter, &mut out);
            expect_counts(
                receiver,
                &adapter,
                &mut out,
                if index % 2 == 0 { (1, 1) } else { (0, 0) },
                "sustained click/match ordering",
            );
            let matched = out
                .iter()
                .filter(|o| o.effects.contains(&Effect::Match(42)))
                .count();
            assert_eq!(matched, index % 2);
            matches += matched;
        }
        adapter.stop().unwrap();
        println!(
            "{{\"acceptance\":\"passed\",\"externalEscapeInjected\":false,\"hypervisorSuspendResume\":true,\"resolutionBefore\":[{},{}],\"resolutionDuring\":[{},{}],\"soakSessions\":256,\"matches\":{},\"soakMilliseconds\":{}}}",
            original.dmPelsWidth,
            original.dmPelsHeight,
            changed.dmPelsWidth,
            changed.dmPelsHeight,
            matches,
            started.elapsed().as_millis()
        );
        stage("complete");
    }
    pub fn run() {
        unsafe {
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        assert_eq!(
            unsafe { GetAsyncKeyState(i32::from(VK_RBUTTON)) } & i16::MIN,
            0,
            "fixture needs an idle physical button"
        );
        let mut receiver = Receiver::start();
        let (x, y) = (receiver.center.x, receiver.center.y);
        move_input(x, y);
        assert_eq!(
            unsafe { WindowFromPoint(POINT { x, y }) },
            receiver.window,
            "fixture owns the cross-process injection target"
        );
        // Activate only the owned receiver before installing suppression hooks.
        activate(&receiver);
        let mut matcher = Matcher::default();
        matcher.register(&[Direction::Right], 42).unwrap();
        let mut config = Configuration {
            start_distance: 15.,
            change_distance: 5.,
            total_duration_ns: 5_000_000_000,
            idle_duration_ns: 2_000_000_000,
            maximum_points: 256,
            maximum_directions: 64,
            button: 1,
            passes_through_empty_matcher: true,
        };
        if std::env::args().nth(1).as_deref() == Some("--held-release-stress") {
            assert!(
                std::env::args().nth(2).is_none(),
                "--held-release-stress takes no additional arguments"
            );
            // Each mode retains all 32 pairs: the receiver's independent button
            // ledgers have a fixed 64-record budget and are never reset mid-mode.
            held_release_matrix(&mut receiver, &matcher, config, 32);
            println!(
                "{{\"result\":\"passed\",\"heldReleaseStress\":true,\"releaseModes\":4,\"cyclesPerMode\":32,\"ownedReceiverProcess\":true}}"
            );
            return;
        }
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        let mut out = vec![];
        assert!(matches!(
            Recognizer::start(config, &matcher),
            Err(Error::AlreadyRunning)
        ));
        point(x, y, &adapter, &mut out);
        receiver.clear();
        down(&adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "one ordinary click",
        );
        receiver.clear();
        out.clear();
        down(&adapter, &mut out);
        point(x + 60, y, &adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (0, 0),
            "matched gesture has no native click or naked up",
        );
        assert!(out.iter().any(|o| o.effects.contains(&Effect::Match(42))));
        point(x, y, &adapter, &mut out);
        receiver.clear();
        down(&adapter, &mut out);
        point(x - 60, y, &adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "rejected prefix restores held native drag",
        );
        receiver.clear();
        down(&adapter, &mut out);
        adapter.cancel().unwrap();
        pump(&adapter, &mut out, 20);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "pending cancel keeps release balanced",
        );
        receiver.clear();
        down(&adapter, &mut out);
        point(x + 20, y, &adapter, &mut out);
        adapter.cancel().unwrap();
        pump(&adapter, &mut out, 20);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (0, 0),
            "active cancel has no context-menu release",
        );
        receiver.clear();
        down(&adapter, &mut out);
        send(None, Some((VK_ESCAPE, true)));
        send(None, Some((VK_ESCAPE, false)));
        pump(&adapter, &mut out, 20);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "Escape restores pending input",
        );
        let escape = receiver.snapshot();
        assert!(
            !escape
                .events
                .iter()
                .any(|event| matches!(event.message, WM_KEYDOWN | WM_KEYUP)
                    && event.wparam == usize::from(VK_ESCAPE)),
            "captured Escape stays outside target; events={:?}; output={:?}",
            escape.events,
            out.last()
        );
        for (key, right) in [(VK_LCONTROL, false), (VK_RCONTROL, true)] {
            activate(&receiver);
            receiver.clear();
            assert_eq!(unsafe { GetAsyncKeyState(i32::from(key)) } & i16::MIN, 0);
            send(None, Some((key, true)));
            // Global key state does not acknowledge dispatch to the separate
            // receiver's keyboard queue. Establish the original event first.
            let until = Instant::now() + Duration::from_secs(2);
            loop {
                pump(&adapter, &mut out, 5);
                let state = receiver.snapshot();
                if state
                    .control_sides
                    .iter()
                    .filter(|side| **side == right)
                    .count()
                    == 1
                {
                    break;
                }
                assert!(
                    Instant::now() < until,
                    "owned receiver must acknowledge original Control before replay; key={key}, receiver={:?}, state={state:?}",
                    receiver.window
                );
            }
            assert_ne!(unsafe { GetAsyncKeyState(i32::from(key)) } & i16::MIN, 0);
            down(&adapter, &mut out);
            send(None, Some((key, false)));
            pump(&adapter, &mut out, 20);
            up(&adapter, &mut out);
            expect_counts(&mut receiver, &adapter, &mut out, (1, 1), "modified click");
            assert!(
                receiver
                    .snapshot()
                    .events
                    .iter()
                    .any(|event| event.message == WM_RBUTTONDOWN && event.wparam & 8 != 0),
                "replayed down preserves original Control"
            );
            assert_eq!(
                receiver
                    .snapshot()
                    .control_sides
                    .iter()
                    .filter(|side| **side == right)
                    .count(),
                2,
                "original and replayed Control use the same side"
            );
            for key in [VK_LCONTROL, VK_RCONTROL] {
                assert_eq!(
                    unsafe { GetAsyncKeyState(i32::from(key)) } & i16::MIN,
                    0,
                    "latest modifier state restored"
                );
            }
        }
        config.idle_duration_ns = 200_000_000;
        adapter.update_tuning(config).unwrap();
        receiver.clear();
        down(&adapter, &mut out);
        pump(&adapter, &mut out, 300);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "idle deadline recovers held pending input",
        );
        receiver.clear();
        down(&adapter, &mut out);
        adapter.stop().unwrap();
        send(Some(MOUSEEVENTF_RIGHTUP), None);
        pump(&adapter, &mut out, 30);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "stop recovers before unhooked real release",
        );
        adapter.stop().unwrap();
        drop(adapter);
        config.idle_duration_ns = 2_000_000_000;
        config.maximum_directions = 4;
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        adapter.set_recording(true).unwrap();
        pump(&adapter, &mut out, 20);
        receiver.clear();
        out.clear();
        down(&adapter, &mut out);
        for i in 0..8 {
            move_input(x + (i % 2) * 60, y);
            pump(&adapter, &mut out, 10);
        }
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "direction budget restores held input",
        );
        assert!(out.iter().any(|output| {
            output.last_session.as_ref().is_some_and(|report| {
                report.outcome == SessionOutcome::DirectionLimit && report.directions.len() == 4
            })
        }));
        assert!(!out.iter().any(|output| {
            output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        }));
        adapter.stop().unwrap();
        drop(adapter);
        // Queue overflow is a distinct guard: allow enough runs to reach its
        // bounded observation capacity instead of triggering direction recovery.
        config.maximum_directions = 512;
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        adapter.set_recording(true).unwrap();
        pump(&adapter, &mut out, 20);
        receiver.clear();
        down(&adapter, &mut out);
        // Deliberately stop consuming observations. The adapter must fail open
        // and recover input instead of blocking the hook or dropping a match.
        for i in 0..300 {
            move_input(x + (i % 2) * 60, y);
            std::thread::sleep(Duration::from_millis(2));
        }
        send(Some(MOUSEEVENTF_RIGHTUP), None);
        let until = Instant::now() + Duration::from_secs(3);
        let mut lagged = false;
        let notification = adapter.duplicate_notification_handle().unwrap();
        while Instant::now() < until {
            match adapter.try_read() {
                Ok(ReadResult::Frame(_)) => {}
                Ok(ReadResult::Pending) => unsafe {
                    WaitForSingleObject(notification.as_raw_handle(), 2000);
                },
                Ok(ReadResult::Completed) => break,
                Err(Error::ConsumerLagged) => {
                    lagged = true;
                    break;
                }
                Err(error) => panic!("unexpected adapter failure: {error}"),
            }
        }
        assert!(lagged);
        let until = Instant::now() + Duration::from_secs(2);
        while receiver.counts() != (1, 1) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            receiver.counts(),
            (1, 1),
            "queue overflow restores balanced input"
        );
        assert_eq!(adapter.stop(), Err(Error::ConsumerLagged));
        drop(adapter);
        // A different application can cover the original press before recovery.
        // Replaying a fresh down into that new target would trigger an action the
        // user never pressed there. Both receivers are owned by this fixture.
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        point(x, y, &adapter, &mut out);
        receiver.clear();
        down(&adapter, &mut out);
        let mut cover = Receiver::start();
        assert_ne!(cover.pid(), receiver.pid());
        assert_eq!(unsafe { WindowFromPoint(POINT { x, y }) }, cover.window);
        cover.clear();
        send(Some(MOUSEEVENTF_RIGHTUP), None);
        expect_target_changed(&mut adapter);
        assert_eq!(
            cover.counts(),
            (0, 0),
            "a newly covering process must not receive a replayed click"
        );
        drop(cover);
        drop(adapter);
        for change in ["hide", "move"] {
            let mut replacement = Receiver::start();
            activate(&receiver);
            receiver.clear();
            replacement.clear();
            let mut adapter = Recognizer::start(config, &matcher).unwrap();
            down(&adapter, &mut out);
            if change == "hide" {
                receiver.hide();
            } else {
                receiver.move_to(x + 40, 40);
            }
            assert_eq!(
                unsafe { WindowFromPoint(POINT { x, y }) },
                replacement.window
            );
            send_button(1, false);
            expect_target_changed(&mut adapter);
            assert_eq!(
                replacement.counts(),
                (0, 0),
                "{change} must not redirect a retained click"
            );
            receiver.move_to(40, 40);
            drop(replacement);
        }
        receiver.clear();
        let target = Receiver::start();
        activate(&target);
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        down(&adapter, &mut out);
        drop(target);
        send_button(1, false);
        expect_target_changed(&mut adapter);
        assert_eq!(
            receiver.counts(),
            (0, 0),
            "an exited receiver must not redirect a retained click"
        );
        drop(adapter);
        for button in [0, 2, 3, 4] {
            activate(&receiver);
            config.button = button;
            let mut adapter = Recognizer::start(config, &matcher).unwrap();
            receiver.clear();
            out.clear();
            button_down(button, &adapter, &mut out);
            send_button(button, false);
            expect_button_counts(
                &mut receiver,
                button,
                &adapter,
                &mut out,
                (1, 1),
                "configured button click",
            );
            let event = receiver
                .snapshot()
                .events
                .into_iter()
                .find(|event| {
                    matches!(
                        event.message,
                        WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
                    )
                })
                .unwrap();
            assert!(
                (event.x - x).abs() <= 1 && (event.y - y).abs() <= 1,
                "receiver sees physical screen coordinates: {},{} expected {x},{y}",
                event.x,
                event.y
            );
            receiver.clear();
            out.clear();
            button_down(button, &adapter, &mut out);
            point(x + 60, y, &adapter, &mut out);
            send_button(button, false);
            expect_button_counts(
                &mut receiver,
                button,
                &adapter,
                &mut out,
                (0, 0),
                "configured button match suppresses native input",
            );
            assert!(out.iter().any(|o| o.effects.contains(&Effect::Match(42))));
            adapter.stop().unwrap();
        }
        config.button = 1;
        let mut beside = Receiver::start();
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        receiver.frame(10, 40, width / 2 - 20, 400);
        beside.frame(width / 2, 40, width / 2 - 10, 400);
        activate(&receiver);
        let mut adapter = Recognizer::start(config, &matcher).unwrap();
        receiver.clear();
        beside.clear();
        send_button(0, true);
        // The receiver pumps messages in its own process. A fixed delay does not
        // acknowledge that its left-down handler has acquired native capture.
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            pump(&adapter, &mut out, 5);
            let state = receiver.snapshot();
            if state.events.iter().any(|e| e.message == WM_LBUTTONDOWN)
                && state.capture == receiver.window
            {
                break;
            }
            assert!(
                Instant::now() < until,
                "owned left-down must acquire capture before moving outside; receiver={:?}, receiver thread state={state:?}",
                receiver.window
            );
        }
        point(beside.center.x, beside.center.y, &adapter, &mut out);
        assert_eq!(
            unsafe {
                WindowFromPoint(POINT {
                    x: beside.center.x,
                    y: beside.center.y,
                })
            },
            beside.window
        );
        let state = receiver.snapshot();
        assert_eq!(
            state.capture, receiver.window,
            "native capture must remain with the receiver before right-down; receiver thread state={state:?}"
        );
        down(&adapter, &mut out);
        up(&adapter, &mut out);
        expect_counts(
            &mut receiver,
            &adapter,
            &mut out,
            (1, 1),
            "native capture owns replay outside its window",
        );
        assert_eq!(beside.counts(), (0, 0));
        send_button(0, false);
        pump(&adapter, &mut out, 30);
        adapter.stop().unwrap();
        drop(adapter);
        drop(beside);
        receiver.frame(40, 40, 700, 450);
        activate(&receiver);
        duplicate_down(&mut receiver, &matcher, config);
        held_release_matrix(&mut receiver, &matcher, config, 8);
        let handles_before = handles();
        for _ in 0..64 {
            receiver.clear();
            out.clear();
            let mut adapter = Recognizer::start(config, &matcher).unwrap();
            down(&adapter, &mut out);
            up(&adapter, &mut out);
            expect_counts(
                &mut receiver,
                &adapter,
                &mut out,
                (1, 1),
                "repeated stop/start keeps native input balanced",
            );
            adapter.stop().unwrap();
        }
        let handles_after = handles();
        assert!(
            handles_after <= handles_before + 2,
            "native handle leak: {handles_before} -> {handles_after}"
        );
        println!(
            "{{\"result\":\"passed\",\"globalHooks\":true,\"osInputInjection\":true,\"physicalHardware\":false,\"ownedReceiverProcess\":true,\"controllerPID\":{},\"receiverPID\":{},\"dpi\":{},\"cases\":32,\"restartCycles\":64,\"handlesBefore\":{},\"handlesAfter\":{}}}",
            std::process::id(),
            receiver.pid(),
            receiver.dpi,
            handles_before,
            handles_after
        );
        if std::env::args().nth(1).as_deref() == Some("--acceptance") {
            acceptance(
                &mut receiver,
                &matcher,
                config,
                &std::env::args()
                    .nth(2)
                    .expect("owned control directory required"),
            );
        }
    }
}
#[cfg(windows)]
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--receiver") {
        receiver::run();
    } else {
        probe::run();
    }
}
