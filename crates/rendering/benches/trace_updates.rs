//! Allocation and CPU comparison for immutable tail updates. Run `cargo bench
//! -p pointer-gesture-rendering --bench trace_updates`; this is not a UI latency claim.
use pointer_gesture_rendering::{Direction, OverlayTrace, Point};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
struct CountAlloc;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountAlloc = CountAlloc;
fn measure(iterations: usize, mut update: impl FnMut(usize) -> OverlayTrace) -> (u128, usize) {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let started = Instant::now();
    for i in 0..iterations {
        black_box(update(black_box(i)));
    }
    (
        started.elapsed().as_nanos() / iterations as u128,
        (ALLOCATIONS.load(Ordering::Relaxed) - before) / iterations,
    )
}
fn main() {
    for count in [4, 64, 4096] {
        let directions: Vec<_> = (0..count)
            .map(|i| {
                if i % 2 == 0 {
                    Direction::Right
                } else {
                    Direction::Down
                }
            })
            .collect();
        let points: Vec<_> = (0..=count)
            .map(|i| Point {
                x: i as f64,
                y: i as f64,
            })
            .collect();
        let original = OverlayTrace::new(&directions, &points, None).unwrap();
        let tail = |i| {
            Some(Point {
                x: i as f64,
                y: i as f64,
            })
        };
        let mut copies = Vec::new();
        let mut shared = Vec::new();
        for _ in 0..3 {
            copies.push(measure(20_000, |i| {
                OverlayTrace::new(black_box(&directions), black_box(&points), tail(i)).unwrap()
            }));
            shared.push(measure(20_000, |i| {
                black_box(&original).with_tail(tail(i)).unwrap()
            }));
        }
        copies.sort();
        shared.sort();
        println!(
            "{{\"directions\":{count},\"iterations\":20000,\"copy_ns\":{},\"shared_ns\":{},\"copy_allocations\":{},\"shared_allocations\":{}}}",
            copies[1].0, shared[1].0, copies[1].1, shared[1].1
        );
        assert_eq!(copies[1].1, 2);
        assert_eq!(shared[1].1, 0);
    }
}
