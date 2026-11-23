//! Private-pipe receiver with input observers confined to its own UI thread.
use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::ClientToScreen,
    System::{LibraryLoader::*, Threading::GetCurrentThreadId},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static CONTROL_SIDES: Mutex<Vec<bool>> = Mutex::new(Vec::new());
static RECEIVER_WINDOW: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static DIRECT_BUTTONS: RefCell<Vec<DirectButton>> = const { RefCell::new(Vec::new()) };
    static REMOVED_BUTTONS: RefCell<Vec<RemovedButton>> = const { RefCell::new(Vec::new()) };
}

fn right_button(message: u32) -> bool {
    matches!(message, WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_RBUTTONUP)
}

struct InputHook(HHOOK);
impl InputHook {
    fn install(kind: i32, procedure: HOOKPROC) -> Self {
        let hook = unsafe {
            SetWindowsHookExW(kind, procedure, std::ptr::null_mut(), GetCurrentThreadId())
        };
        assert!(
            !hook.is_null(),
            "owned receiver input hook installation failed"
        );
        Self(hook)
    }
}
impl Drop for InputHook {
    fn drop(&mut self) {
        unsafe {
            UnhookWindowsHookEx(self.0);
        }
    }
}

unsafe extern "system" fn direct_mouse(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && right_button(message as u32) {
        let event = unsafe { &*(data as *const MOUSEHOOKSTRUCT) };
        if event.hwnd as usize == RECEIVER_WINDOW.load(Ordering::Relaxed) {
            DIRECT_BUTTONS.with(|events| {
                let mut events = events.borrow_mut();
                assert!(events.len() < 64, "receiver direct button budget exceeded");
                events.push(DirectButton {
                    message: message as u32,
                    extra: event.dwExtraInfo,
                });
            });
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, data) }
}

unsafe extern "system" fn removed_message(code: i32, removed: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && removed == PM_REMOVE as WPARAM {
        let message = unsafe { &*(data as *const MSG) };
        if message.hwnd as usize == RECEIVER_WINDOW.load(Ordering::Relaxed)
            && right_button(message.message)
        {
            REMOVED_BUTTONS.with(|events| {
                let mut events = events.borrow_mut();
                assert!(events.len() < 64, "receiver removed button budget exceeded");
                events.push(RemovedButton {
                    message: message.message,
                    time: message.time,
                });
            });
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, removed, data) }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

unsafe extern "system" fn procedure(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if matches!(
        message,
        WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_MOUSEMOVE
            | WM_KEYDOWN
            | WM_KEYUP
    ) {
        let mut events = EVENTS.lock().unwrap();
        assert!(events.len() < 8192, "receiver event budget exceeded");
        let mut point = POINT { x: 0, y: 0 };
        if !matches!(message, WM_KEYDOWN | WM_KEYUP) {
            point = POINT {
                x: l as u16 as i16 as i32,
                y: (l as usize >> 16) as u16 as i16 as i32,
            };
            unsafe {
                ClientToScreen(window, &mut point);
            }
        }
        events.push(Event {
            message,
            wparam: w,
            x: point.x,
            y: point.y,
        });
    }
    if message == WM_KEYDOWN && w == usize::from(VK_CONTROL) {
        CONTROL_SIDES
            .lock()
            .unwrap()
            .push(l as usize & (1 << 24) != 0);
    }
    unsafe {
        if matches!(
            message,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        ) {
            SetCapture(window);
        }
        // Keep capture while another native button remains held (MK_* button bits).
        if matches!(
            message,
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP
        ) && w & 0x73 == 0
        {
            ReleaseCapture();
        }
        DefWindowProcW(window, message, w, l)
    }
}

fn messages() {
    let mut message = unsafe { std::mem::zeroed::<MSG>() };
    unsafe {
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub fn run() {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let class = wide("PointerGestureInputReceiver");
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let definition = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    assert_ne!(unsafe { RegisterClassW(&definition) }, 0);
    let window = unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW,
            class.as_ptr(),
            wide("Pointer input: owned receiver process").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            40,
            40,
            700,
            450,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    assert!(!window.is_null());
    RECEIVER_WINDOW.store(window as usize, Ordering::Relaxed);
    DIRECT_BUTTONS.with(|events| events.borrow_mut().reserve_exact(64));
    REMOVED_BUTTONS.with(|events| events.borrow_mut().reserve_exact(64));
    let _direct = InputHook::install(WH_MOUSE, Some(direct_mouse));
    let _removed = InputHook::install(WH_GETMESSAGE, Some(removed_message));
    assert_ne!(
        unsafe {
            SetWindowPos(
                window,
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
    let mut client = unsafe { std::mem::zeroed::<RECT>() };
    assert_ne!(unsafe { GetClientRect(window, &mut client) }, 0);
    let mut center = POINT {
        x: (client.left + client.right) / 2,
        y: (client.top + client.bottom) / 2,
    };
    assert_ne!(unsafe { ClientToScreen(window, &mut center) }, 0);
    println!(
        "READY {} {} {} {} {}",
        std::process::id(),
        window as usize,
        center.x,
        center.y,
        unsafe { GetDpiForWindow(window) }
    );
    std::io::stdout().flush().unwrap();
    let (send, commands) = mpsc::sync_channel(4);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(300);
    'running: while Instant::now() < deadline {
        messages();
        match commands.try_recv() {
            Ok(command) => {
                match command.as_str() {
                    "HIDE" => {
                        unsafe {
                            ShowWindow(window, SW_HIDE);
                        }
                        println!("HIDDEN");
                    }
                    "RESET" => {
                        EVENTS.lock().unwrap().clear();
                        CONTROL_SIDES.lock().unwrap().clear();
                        DIRECT_BUTTONS.with(|events| events.borrow_mut().clear());
                        REMOVED_BUTTONS.with(|events| events.borrow_mut().clear());
                        println!("RESET");
                    }
                    "STATE" => {
                        let events = EVENTS
                            .lock()
                            .unwrap()
                            .iter()
                            .map(|e| format!("{}:{}:{}:{}", e.message, e.wparam, e.x, e.y))
                            .collect::<Vec<_>>()
                            .join(",");
                        let sides = CONTROL_SIDES
                            .lock()
                            .unwrap()
                            .iter()
                            .map(|right| if *right { "1" } else { "0" })
                            .collect::<Vec<_>>()
                            .join(",");
                        let direct = DIRECT_BUTTONS.with(|events| {
                            events
                                .borrow()
                                .iter()
                                .map(|event| format!("{}:{}", event.message, event.extra))
                                .collect::<Vec<_>>()
                                .join(",")
                        });
                        let removed = REMOVED_BUTTONS.with(|events| {
                            events
                                .borrow()
                                .iter()
                                .map(|event| format!("{}:{}", event.message, event.time))
                                .collect::<Vec<_>>()
                                .join(",")
                        });
                        println!(
                            "STATE {} {} {} {} {} {}",
                            if events.is_empty() { "-" } else { &events },
                            if sides.is_empty() { "-" } else { &sides },
                            unsafe { GetCapture() } as usize,
                            unsafe { GetForegroundWindow() } as usize,
                            if direct.is_empty() { "-" } else { &direct },
                            if removed.is_empty() { "-" } else { &removed },
                        );
                    }
                    "QUIT" => break 'running,
                    command if command.starts_with("MOVE ") => {
                        let values: Vec<i32> = command[5..]
                            .split_whitespace()
                            .map(|v| v.parse().unwrap())
                            .collect();
                        assert_eq!(values.len(), 2);
                        assert_ne!(
                            unsafe {
                                SetWindowPos(
                                    window,
                                    std::ptr::null_mut(),
                                    values[0],
                                    values[1],
                                    0,
                                    0,
                                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                                )
                            },
                            0
                        );
                        println!("MOVED");
                    }
                    command if command.starts_with("FRAME ") => {
                        let values: Vec<i32> = command[6..]
                            .split_whitespace()
                            .map(|v| v.parse().unwrap())
                            .collect();
                        assert_eq!(values.len(), 4);
                        assert_ne!(
                            unsafe {
                                SetWindowPos(
                                    window,
                                    std::ptr::null_mut(),
                                    values[0],
                                    values[1],
                                    values[2],
                                    values[3],
                                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                                )
                            },
                            0
                        );
                        let mut client = unsafe { std::mem::zeroed::<RECT>() };
                        assert_ne!(unsafe { GetClientRect(window, &mut client) }, 0);
                        let mut center = POINT {
                            x: (client.left + client.right) / 2,
                            y: (client.top + client.bottom) / 2,
                        };
                        assert_ne!(unsafe { ClientToScreen(window, &mut center) }, 0);
                        println!("FRAME {} {}", center.x, center.y);
                    }
                    _ => panic!("unknown receiver command"),
                }
                std::io::stdout().flush().unwrap();
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    unsafe {
        DestroyWindow(window);
    }
}

#[derive(Debug)]
pub struct Event {
    pub message: u32,
    pub wparam: usize,
    pub x: i32,
    pub y: i32,
}
#[derive(Debug)]
pub struct Snapshot {
    pub events: Vec<Event>,
    pub control_sides: Vec<bool>,
    pub capture: HWND,
    pub foreground: HWND,
    pub direct_buttons: Vec<DirectButton>,
    pub removed_buttons: Vec<RemovedButton>,
}

#[derive(Debug)]
pub struct DirectButton {
    pub message: u32,
    pub extra: usize,
}
#[derive(Debug)]
pub struct RemovedButton {
    pub message: u32,
    pub time: u32,
}

/// A bounded request/reply pipe and a uniquely owned child. Drop reaps it even
/// when a fixture assertion fails; no unrelated process/window is ever stopped.
pub struct Receiver {
    child: Child,
    input: Option<ChildStdin>,
    output: mpsc::Receiver<String>,
    reader: Option<JoinHandle<()>>,
    cursor: POINT,
    pub window: HWND,
    pub center: POINT,
    pub dpi: u32,
}
impl Receiver {
    pub fn start() -> Self {
        let mut cursor = unsafe { std::mem::zeroed::<POINT>() };
        assert_ne!(unsafe { GetCursorPos(&mut cursor) }, 0);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--receiver")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (send, output) = mpsc::sync_channel(4);
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut receiver = Self {
            child,
            input,
            output,
            reader: Some(reader),
            cursor,
            window: std::ptr::null_mut(),
            center: POINT { x: 0, y: 0 },
            dpi: 0,
        };
        let ready = receiver.line();
        let tokens: Vec<_> = ready.split_whitespace().collect();
        assert_eq!(tokens.len(), 6, "receiver startup: {ready}");
        assert_eq!(tokens[0], "READY");
        assert_eq!(tokens[1].parse::<u32>().unwrap(), receiver.child.id());
        assert_ne!(receiver.child.id(), std::process::id());
        receiver.window = tokens[2].parse::<usize>().unwrap() as HWND;
        receiver.center = POINT {
            x: tokens[3].parse().unwrap(),
            y: tokens[4].parse().unwrap(),
        };
        receiver.dpi = tokens[5].parse().unwrap();
        let mut owner = 0;
        unsafe {
            GetWindowThreadProcessId(receiver.window, &mut owner);
        }
        assert_eq!(
            owner,
            receiver.child.id(),
            "receiver must own the native target"
        );
        receiver
    }
    fn line(&self) -> String {
        self.output
            .recv_timeout(Duration::from_secs(5))
            .expect("receiver response timed out or exited")
    }
    fn request(&mut self, command: &str) -> String {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{command}").unwrap();
        input.flush().unwrap();
        self.line()
    }
    pub fn clear(&mut self) {
        assert_eq!(self.request("RESET"), "RESET");
    }
    pub fn hide(&mut self) {
        assert_eq!(self.request("HIDE"), "HIDDEN");
    }
    pub fn move_to(&mut self, x: i32, y: i32) {
        assert_eq!(self.request(&format!("MOVE {x} {y}")), "MOVED");
    }
    pub fn frame(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let response = self.request(&format!("FRAME {x} {y} {width} {height}"));
        let tokens: Vec<_> = response.split_whitespace().collect();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], "FRAME");
        self.center = POINT {
            x: tokens[1].parse().unwrap(),
            y: tokens[2].parse().unwrap(),
        };
    }
    pub fn snapshot(&mut self) -> Snapshot {
        let line = self.request("STATE");
        let tokens: Vec<_> = line.split_whitespace().collect();
        assert_eq!(tokens.len(), 7, "receiver state: {line}");
        assert_eq!(tokens[0], "STATE");
        let events = if tokens[1] == "-" {
            vec![]
        } else {
            tokens[1]
                .split(',')
                .map(|event| {
                    let fields: Vec<_> = event.split(':').collect();
                    assert_eq!(fields.len(), 4);
                    Event {
                        message: fields[0].parse().unwrap(),
                        wparam: fields[1].parse().unwrap(),
                        x: fields[2].parse().unwrap(),
                        y: fields[3].parse().unwrap(),
                    }
                })
                .collect()
        };
        let control_sides = if tokens[2] == "-" {
            vec![]
        } else {
            tokens[2]
                .split(',')
                .map(|side| match side {
                    "0" => false,
                    "1" => true,
                    _ => panic!("invalid modifier side"),
                })
                .collect()
        };
        let direct_buttons = if tokens[5] == "-" {
            vec![]
        } else {
            tokens[5]
                .split(',')
                .map(|event| {
                    let (message, extra) = event.split_once(':').unwrap();
                    DirectButton {
                        message: message.parse().unwrap(),
                        extra: extra.parse().unwrap(),
                    }
                })
                .collect()
        };
        let removed_buttons = if tokens[6] == "-" {
            vec![]
        } else {
            tokens[6]
                .split(',')
                .map(|event| {
                    let (message, time) = event.split_once(':').unwrap();
                    RemovedButton {
                        message: message.parse().unwrap(),
                        time: time.parse().unwrap(),
                    }
                })
                .collect()
        };
        Snapshot {
            events,
            control_sides,
            capture: tokens[3].parse::<usize>().unwrap() as HWND,
            foreground: tokens[4].parse::<usize>().unwrap() as HWND,
            direct_buttons,
            removed_buttons,
        }
    }
    pub fn counts(&mut self) -> (usize, usize) {
        self.counts_for(1)
    }
    pub fn counts_for(&mut self, button: u32) -> (usize, usize) {
        let snapshot = self.snapshot();
        let (down, up) = match button {
            0 => (WM_LBUTTONDOWN, WM_LBUTTONUP),
            1 => (WM_RBUTTONDOWN, WM_RBUTTONUP),
            2 => (WM_MBUTTONDOWN, WM_MBUTTONUP),
            3 | 4 => (WM_XBUTTONDOWN, WM_XBUTTONUP),
            _ => panic!("unsupported test button"),
        };
        (
            snapshot
                .events
                .iter()
                .filter(|e| {
                    e.message == down && (button < 3 || e.wparam >> 16 == (button - 2) as usize)
                })
                .count(),
            snapshot
                .events
                .iter()
                .filter(|e| {
                    e.message == up && (button < 3 || e.wparam >> 16 == (button - 2) as usize)
                })
                .count(),
        )
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        if let Some(mut input) = self.input.take() {
            let _ = writeln!(input, "QUIT");
        }
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until && matches!(self.child.try_wait(), Ok(None)) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        unsafe {
            SetCursorPos(self.cursor.x, self.cursor.y);
        }
    }
}
