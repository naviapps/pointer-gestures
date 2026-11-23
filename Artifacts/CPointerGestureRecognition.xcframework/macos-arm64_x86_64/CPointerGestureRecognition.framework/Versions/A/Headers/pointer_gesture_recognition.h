#ifndef POINTER_GESTURE_RECOGNITION_H
#define POINTER_GESTURE_RECOGNITION_H
#include <stdint.h>
#include <stddef.h>
#define POINTER_GESTURE_RECOGNITION_ABI_VERSION 8U
#ifdef __cplusplus
extern "C" {
#endif

/* All non-null handles must be live. Engine access is exclusive; immutable frame
   views and matcher reads may be shared. The caller owns OS effect delivery.
   Boolean outputs use 0/1. Direction codes: up=1, down=2, left=3, right=4;
   movement direction=0 means equal axis deltas; valid=0 means non-finite input.
   Points use one host coordinate space, increasing rightward and downward. */
typedef struct {
    uint32_t valid;
    uint32_t direction;
    double maximum_axis_delta;
} PointerGestureRecognitionMovement;
uint32_t pointer_gesture_recognition_abi_version(void);
/* Saturating ceil to nanoseconds; non-positive/non-finite seconds return zero. */
uint64_t pointer_gesture_recognition_duration_nanoseconds(double seconds);
/* Elapsed components: clamp negative values, truncate fractions, saturate overflow. */
uint64_t pointer_gesture_recognition_elapsed_nanoseconds(int64_t seconds, int64_t attoseconds);
/* First invalid field: start=1, change=2, total=3, idle=4, points=5, flags=6.
   durations_present bits: total=1, idle=2. Absent durations are ignored. */
uint32_t pointer_gesture_recognition_validate_tuning(double start, double change, double total,
                                                     double idle, uint32_t durations_present,
                                                     int64_t points);
/* Retry delay validation returns SIZE_MAX on success, otherwise the first invalid
   zero-based index. NULL delays is allowed only when count is zero. */
size_t pointer_gesture_recognition_validate_retry_delays(const double *delays, size_t count);
/* Host-neutral source-start lifecycle; exclusive handle access. It owns policy,
   revisions and retry selection, never OS resources, sleeps or callbacks. */
typedef struct PointerGestureRecognitionStartup PointerGestureRecognitionStartup;
typedef struct {
    uint64_t revision;
    uint32_t lifecycle, requested;
} PointerGestureRecognitionStartupView;
PointerGestureRecognitionStartup *pointer_gesture_recognition_startup_create(void);
void pointer_gesture_recognition_startup_destroy(PointerGestureRecognitionStartup *handle);
/* Lifecycle: idle=0, starting=1, ready=2, retrying=3, failed=4. */
PointerGestureRecognitionStartupView
pointer_gesture_recognition_startup_view(const PointerGestureRecognitionStartup *handle);
/* Kinds: start=1, stop=2, retry-now=3, scheduled-retry=4, success=5, failure=6.
   Kinds 4..6 require the revision of the attempt; stale requests are ignored.
   Returns 1 if accepted, 0 if ignored/unknown. */
uint32_t pointer_gesture_recognition_startup_request(PointerGestureRecognitionStartup *handle,
                                                     uint32_t kind, uint64_t revision);
/* Invalid=0 (unchanged), accepted=1, accepted and pending work replaced=2. */
uint32_t pointer_gesture_recognition_startup_set_delays(PointerGestureRecognitionStartup *handle,
                                                        const double *delays, size_t count);
/* Returns 1 and writes a delay when retrying; 0 otherwise. Zero delay means yield.
   Each call advances the schedule; repeated failures repeat its last delay. */
uint32_t pointer_gesture_recognition_startup_next_delay(PointerGestureRecognitionStartup *handle,
                                                        uint64_t *delay_ns);
/* Finite endpoints can yield an infinite maximum_axis_delta and a cardinal direction. */
PointerGestureRecognitionMovement pointer_gesture_recognition_movement(double sx, double sy,
                                                                       double ex, double ey);
/* Distances must be finite/nonnegative and maximum_points positive. Raw traces
   obey that limit; replay retains at least the origin and latest location.
   Zero durations disable limits. pass_empty must be 0 or 1. button and
   pass_empty are immutable after construction; button is a host-defined ID. */
typedef struct {
    double start_distance, change_distance;
    uint64_t total_ns, idle_ns;
    size_t maximum_points;
    uint32_t button, pass_empty;
    size_t maximum_directions;
} PointerGestureRecognitionConfig;
/* Direction runs are bounded to 1..4096 and captured per button-down session. */
uint32_t pointer_gesture_recognition_validate_direction_limit(int64_t maximum_directions);
#define POINTER_GESTURE_RECOGNITION_PREPARE_REPLACEMENT_PRESS 11
/* Kinds: down=1, move=2, up=3, cancel=4, stop=5, start=6, recording=7,
   host policy=8, snapshot=9, timer-tick=10, prepare replacement press=11.
   Control events ignore pointer coordinates. Preparation expires elapsed deadlines
   and recovers any remaining input as superseded without admitting a new press.
   Deliver recovery before resolving new policy and submitting the button-down.
   Non-finite down/move points pass through. A non-finite up for retained input
   consumes the release, interrupts without matching and recovers from finite
   history. Unretained or unrelated-button releases pass through.
   Unknown kinds return NULL without changing the engine.
   now_ns is relative monotonic time, in nanoseconds. Zero durations disable limits.
   value: recording uses nonzero=true; policy bits are enabled=1, modifiers=2.
   Tick expires elapsed deadlines. Snapshot reads state without expiring input. */
typedef struct {
    uint32_t kind, button;
    double x, y;
    uint64_t now_ns;
    uint32_t value;
} PointerGestureRecognitionEvent;
typedef struct {
    double x, y;
} PointerGestureRecognitionPoint;
/* Uniformly sampled replay path, retaining both endpoints (minimum limit two).
   Returns required count. Writes only when capacity is sufficient and count is
   nonzero. Input/output must not overlap. A NULL output with capacity zero queries
   size without accessing input; empty input permits NULL points. */
size_t pointer_gesture_recognition_replay_points(const PointerGestureRecognitionPoint *points,
                                                 size_t count, size_t maximum,
                                                 PointerGestureRecognitionPoint *output,
                                                 size_t capacity);
typedef struct {
    const uint32_t *data;
    size_t count;
} PointerGestureRecognitionDirectionsView;
typedef struct {
    const PointerGestureRecognitionPoint *data;
    size_t count;
} PointerGestureRecognitionPointsView;
/* disposition: pass-through=0, consume=1, control/no physical input=2.
   stage: idle=0, pending threshold=1, capturing=2.
   failure: none=0, disabled=1, modifiers=2, expired=3.
   input_generation is zero when idle, otherwise a nonzero wrapping session ID.
   discard_visible clears a host-coalesced visible update after interruption.
   When deadline_present is set, schedule a tick at absolute deadline_ns. */
typedef struct {
    uint32_t disposition, ready, stage, capturing, recording, visible, failure;
    uint64_t input_generation;
    uint32_t discard_visible;
    PointerGestureRecognitionDirectionsView directions;
    PointerGestureRecognitionPointsView endpoints, raw;
    PointerGestureRecognitionDirectionsView recorded;
    uint32_t tail_present;
    PointerGestureRecognitionPoint tail;
    size_t effect_count;
    uint32_t deadline_present;
    uint64_t deadline_ns;
} PointerGestureRecognitionFrameView;
typedef struct {
    uint32_t visible;
    PointerGestureRecognitionDirectionsView directions;
    PointerGestureRecognitionPointsView endpoints, raw;
    uint32_t tail_present;
    PointerGestureRecognitionPoint tail;
} PointerGestureRecognitionTraceView;
/* Deliver in order: click=1, complete drag=2, held drag start=3, release=4,
   host command match=5, trace/status publication=6. A held drag start leaves the
   real release to pass through. match_value accepts every uint64_t, including zero.
   Only fields belonging to kind are meaningful. Publication status may differ
   from the final frame. Trace views share their containing frame's ownership. */
typedef struct {
    uint32_t kind, button;
    uint64_t match_value;
    PointerGestureRecognitionPointsView points;
    PointerGestureRecognitionTraceView trace;
    uint32_t ready, capturing, recording, failure;
    PointerGestureRecognitionDirectionsView recorded;
} PointerGestureRecognitionEffectView;
typedef struct PointerGestureRecognitionEngine PointerGestureRecognitionEngine;
typedef struct PointerGestureRecognitionFrame PointerGestureRecognitionFrame;
/* Last completed retained session. Normal clicks are below-threshold, not errors.
   outcome: matched=1, recorded=2, below-threshold=3, unmatched=4,
   unmatched-prefix=5, cancelled=6, stopped=7, superseded=8, total-deadline=9,
   idle-deadline=10, invalid-release=11, direction-limit=12.
   Timings and sample count describe admitted finite input, not successful OS
   recovery/command execution. Directions borrow the containing frame; no pointer
   coordinates are retained. Generation is engine-local and wraps, never zero.
   A rejected_direction of zero means none. The last report persists until another
   retained session completes, including across snapshots and new button-downs. */
typedef struct {
    uint32_t present, outcome, captured, recording, rejected_direction;
    uint64_t generation, elapsed_ns, idle_ns, point_count;
    double maximum_axis_distance;
    PointerGestureRecognitionDirectionsView directions;
} PointerGestureRecognitionSessionReportView;
PointerGestureRecognitionSessionReportView
pointer_gesture_recognition_frame_session_report(const PointerGestureRecognitionFrame *frame);
/* Null means invalid configuration. Destroy each owned engine exactly once. */
PointerGestureRecognitionEngine *
pointer_gesture_recognition_engine_create(PointerGestureRecognitionConfig config);
/* Frees a null or live engine without producing recovery or requiring a clock.
   For retained input, obtain Stop (event kind 5) output and deliver its recovery
   before tearing down the host input adapter. */
void pointer_gesture_recognition_engine_destroy(PointerGestureRecognitionEngine *engine);
/* Frames own their storage. They remain valid after later steps and engine destruction.
   Views borrow read-only memory from their frame; never free individual views. */
PointerGestureRecognitionFrame *
pointer_gesture_recognition_engine_step(PointerGestureRecognitionEngine *engine,
                                        PointerGestureRecognitionEvent event);
PointerGestureRecognitionFrameView
pointer_gesture_recognition_frame_view(const PointerGestureRecognitionFrame *frame);
PointerGestureRecognitionEffectView
pointer_gesture_recognition_frame_effect(const PointerGestureRecognitionFrame *frame, size_t index);
void pointer_gesture_recognition_frame_destroy(PointerGestureRecognitionFrame *frame);
typedef struct PointerGestureRecognitionMatcher PointerGestureRecognitionMatcher;
typedef struct {
    uint32_t present;
    uint64_t value;
} PointerGestureRecognitionLookup;
/* Handles clone in O(1). Reads may run concurrently. Mutations require an exclusive
   handle and copy shared trie storage. Every handle is destroyed exactly once. */
PointerGestureRecognitionMatcher *pointer_gesture_recognition_matcher_create(void);
PointerGestureRecognitionMatcher *
pointer_gesture_recognition_matcher_clone(const PointerGestureRecognitionMatcher *matcher);
void pointer_gesture_recognition_matcher_destroy(PointerGestureRecognitionMatcher *matcher);
/* Empty/invalid patterns fail. Pattern buffers are copied into trie structure, never retained. */
/* Validation codes: valid=0, empty=1, too long=2, consecutive equal=3,
   invalid direction=4, null/unaligned buffer=5. Index is zero-based for codes 3/4,
   UINT32_MAX otherwise. Length is checked before reading buffers; then invalid
   direction codes precede repeated runs. Rejection never mutates the matcher. */
typedef struct {
    uint32_t code;
    uint32_t index;
} PointerGestureRecognitionPatternValidation;
PointerGestureRecognitionPatternValidation
pointer_gesture_recognition_pattern_validate(const uint32_t *directions, size_t count);
PointerGestureRecognitionPatternValidation
pointer_gesture_recognition_matcher_register(PointerGestureRecognitionMatcher *matcher,
                                             const uint32_t *directions, size_t count,
                                             uint64_t value);
PointerGestureRecognitionLookup
pointer_gesture_recognition_matcher_match(const PointerGestureRecognitionMatcher *matcher,
                                          const uint32_t *directions, size_t count);
uint32_t
pointer_gesture_recognition_matcher_is_empty(const PointerGestureRecognitionMatcher *matcher);
/* Remove one exact registration. Absent/invalid patterns return present=0.
   Clear releases mutable storage. Captured matcher/engine snapshots remain valid. */
PointerGestureRecognitionLookup
pointer_gesture_recognition_matcher_remove(PointerGestureRecognitionMatcher *matcher,
                                           const uint32_t *directions, size_t count);
void pointer_gesture_recognition_matcher_clear(PointerGestureRecognitionMatcher *matcher);
/* Retains the immutable trie in O(1). Existing input retains its captured matcher even after
 * replacement. */
void pointer_gesture_recognition_engine_use_matcher(
    PointerGestureRecognitionEngine *engine, const PointerGestureRecognitionMatcher *matcher);
/* Result: unchanged=0, updated=1, invalid configuration/immutable policy change=2. */
uint32_t pointer_gesture_recognition_engine_update_tuning(PointerGestureRecognitionEngine *engine,
                                                          PointerGestureRecognitionConfig config,
                                                          uint64_t now_ns,
                                                          uint32_t reset_unchanged);
#ifdef __cplusplus
}
#endif
#endif
