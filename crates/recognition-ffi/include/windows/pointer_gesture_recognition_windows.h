#ifndef POINTER_GESTURE_RECOGNITION_WINDOWS_H
#define POINTER_GESTURE_RECOGNITION_WINDOWS_H
#include "../pointer_gesture_recognition.h"
#define POINTER_GESTURE_RECOGNITION_WINDOWS_ABI_VERSION 3U
#ifdef _WIN32
#ifdef __cplusplus
extern "C" {
#endif
typedef struct PointerGestureRecognitionWindowsRecognizer
    PointerGestureRecognitionWindowsRecognizer;
/* kind: none=0, configuration=1, already running=2, system=3, short input
   injection=4, consumer lag=5, worker stopped=6, replay target changed=7. sent/expected describe a
   short SendInput batch. system may be zero even when injection was blocked. */
typedef struct {
    uint32_t kind, system, sent, expected;
} PointerGestureRecognitionWindowsError;
typedef struct {
    PointerGestureRecognitionPoint point;
    uint32_t recipient_process_id;
    uint32_t foreground_process_id; /* zero if unavailable */
} PointerGestureRecognitionWindowsPressContext;
typedef struct {
    uint64_t value;
    uint32_t present; /* zero if absent; a present value may be zero */
} PointerGestureRecognitionWindowsEpoch;
typedef struct PointerGestureRecognitionWindowsContextSelection
    PointerGestureRecognitionWindowsContextSelection;
typedef void (*PointerGestureRecognitionWindowsResolveContext)(
    uintptr_t user, PointerGestureRecognitionWindowsPressContext press,
    PointerGestureRecognitionWindowsContextSelection *selection);
uint32_t pointer_gesture_recognition_windows_abi_version(void);
/* Exactly one adapter per process. Button IDs are primary=0, secondary=1,
   middle=2, X1=3, X2=4. Physical pixel coordinates; maximum_points <= 4096.
   This explicitly installs global mouse/Escape hooks on an interactive desktop.
   Matcher is copied. All error/frame output pointers must be writable/non-null.
   Serialize every call on one adapter. Failure returns NULL and fills error. */
PointerGestureRecognitionWindowsRecognizer *
pointer_gesture_recognition_windows_start(PointerGestureRecognitionConfig config,
                                          const PointerGestureRecognitionMatcher *matcher,
                                          PointerGestureRecognitionWindowsError *error);
/* Resolve each new eligible press on the hook owner thread. Call select_context exactly
   once and return normally. Use immutable lookups. Do not block, call the adapter, execute
   effects or retain the selection pointer. Missing or invalid selection stops capture
   before suppressing the original press. The callback may run before start returns. */
PointerGestureRecognitionWindowsRecognizer *pointer_gesture_recognition_windows_start_with_context(
    PointerGestureRecognitionConfig config, PointerGestureRecognitionWindowsResolveContext resolve,
    uintptr_t user, PointerGestureRecognitionWindowsError *error);
/* Clone a live, immutable matcher during the current callback. Return 1 on acceptance.
   The host may free its matcher after this call. The selected policy is authoritative
   for the new press, and the epoch identifies the host's command snapshot. */
uint32_t pointer_gesture_recognition_windows_select_context(
    PointerGestureRecognitionWindowsContextSelection *selection,
    const PointerGestureRecognitionMatcher *matcher, uint32_t enabled, uint32_t modifiers,
    uint64_t epoch);
/* Nonblocking: empty=0, owned frame=1, error=2, normally stopped/drained=3. Free returned frames
   using pointer_gesture_recognition_frame_destroy. Only Match/Publish effects remain: native input
   recovery was already applied. Never inject input again from these observations. Drain promptly;
   256 ordinary frames trigger stop/recovery on overflow. Two reserved slots retain the triggering
   observation and final shutdown, before the consumer-lag error is returned. No hook waits for the
   consumer. */
uint32_t
pointer_gesture_recognition_windows_read(const PointerGestureRecognitionWindowsRecognizer *adapter,
                                         PointerGestureRecognitionFrame **frame,
                                         PointerGestureRecognitionWindowsError *error);
/* Drain the same queue as read and copy each frame's captured host epoch.
   Epoch is absent before the first resolved press and whenever no frame was returned. */
uint32_t pointer_gesture_recognition_windows_read_with_context(
    const PointerGestureRecognitionWindowsRecognizer *adapter,
    PointerGestureRecognitionFrame **frame, PointerGestureRecognitionWindowsEpoch *epoch,
    PointerGestureRecognitionWindowsError *error);
/* Duplicate an owned SYNCHRONIZE-only Windows event HANDLE; zero on error.
   Wait for data or completion, then read until empty/stopped/error. No polling is
   required. Only the adapter resets readiness; CloseHandle the duplicate once.
   The duplicate remains valid after adapter destruction and is signalled on stop. */
uintptr_t pointer_gesture_recognition_windows_duplicate_notification_handle(
    const PointerGestureRecognitionWindowsRecognizer *adapter,
    PointerGestureRecognitionWindowsError *error);
uint64_t pointer_gesture_recognition_windows_now_ns(
    const PointerGestureRecognitionWindowsRecognizer *adapter);
/* Mutators return 1 on success, 0 on error. Commands run on the owner thread.
   Future input uses new matcher/policy/recording; active snapshots remain valid.
   Modifier groups: Shift=1, Control=2, Alt=4, Windows=8. */
uint32_t
pointer_gesture_recognition_windows_use_matcher(PointerGestureRecognitionWindowsRecognizer *adapter,
                                                const PointerGestureRecognitionMatcher *matcher,
                                                PointerGestureRecognitionWindowsError *error);
uint32_t
pointer_gesture_recognition_windows_set_policy(PointerGestureRecognitionWindowsRecognizer *adapter,
                                               uint32_t enabled, uint32_t modifiers,
                                               PointerGestureRecognitionWindowsError *error);
/* On success replace the resolver for future presses. Held input and queued frames
   keep their captured matcher and epoch. Keep callback/user data valid until successful
   replacement or joined stop/destroy. After a failed replacement, keep both inputs
   valid until joined stop/destroy. */
uint32_t pointer_gesture_recognition_windows_set_context_resolver(
    PointerGestureRecognitionWindowsRecognizer *adapter,
    PointerGestureRecognitionWindowsResolveContext resolve, uintptr_t user,
    PointerGestureRecognitionWindowsError *error);
uint32_t pointer_gesture_recognition_windows_set_recording(
    PointerGestureRecognitionWindowsRecognizer *adapter, uint32_t enabled,
    PointerGestureRecognitionWindowsError *error);
uint32_t pointer_gesture_recognition_windows_update_tuning(
    PointerGestureRecognitionWindowsRecognizer *adapter, PointerGestureRecognitionConfig config,
    PointerGestureRecognitionWindowsError *error);
uint32_t
pointer_gesture_recognition_windows_cancel(PointerGestureRecognitionWindowsRecognizer *adapter,
                                           PointerGestureRecognitionWindowsError *error);
/* Stop unhooks first, recovers held input, then joins. Repeated calls are safe.
   Destroy also stops/joins, but cannot return a recovery error; call stop first
   when failure reporting is needed. Polling final frames after stop is allowed. */
uint32_t
pointer_gesture_recognition_windows_stop(PointerGestureRecognitionWindowsRecognizer *adapter,
                                         PointerGestureRecognitionWindowsError *error);
void pointer_gesture_recognition_windows_destroy(
    PointerGestureRecognitionWindowsRecognizer *adapter);
#ifdef __cplusplus
}
#endif
#endif
#endif
