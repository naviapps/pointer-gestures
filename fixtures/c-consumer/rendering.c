#include <CPointerGestureRendering/pointer_gesture_rendering.h>
#include <assert.h>
#include <stdio.h>

int main(void) {
    assert(pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION);

    const uint32_t directions[] = {4, 2}; /* Right, down. */
    const PointerGestureRenderingPoint points[] = {{-10, 5}, {30, 5}, {30, 25}};
    const PointerGestureRenderingResult result = pointer_gesture_rendering_live_trace(
        directions, 2, points, 3, 0, (PointerGestureRenderingPoint){0, 0}, 0);
    assert(result.code == 0 && result.geometry != NULL);

    const PointerGestureRenderingGeometryView view =
        pointer_gesture_rendering_geometry_view(result.geometry);
    assert(view.path != NULL && view.path_count == 3 && view.arrowhead_count == 0);
    const PointerGestureRenderingCommand commands[] = {view.path[0], view.path[1], view.path[2]};
    pointer_gesture_rendering_geometry_destroy(result.geometry);

    /* The client owns the copied commands after the framework releases its geometry. */
    assert(commands[0].kind == 1 && commands[1].kind == 2 && commands[2].kind == 2);
    assert(commands[1].point.x == 30 && commands[1].point.y == 5);
    const PointerGestureRenderingPoint vertices[] = {commands[0].point, commands[1].point,
                                                     commands[2].point};
    const PointerGestureRenderingBoundsResult extents =
        pointer_gesture_rendering_bounds(vertices, 3);
    assert(extents.valid == 1);
    assert(extents.bounds.min_x == -10 && extents.bounds.min_y == 5);
    assert(extents.bounds.max_x == 30 && extents.bounds.max_y == 25);
    puts("Standalone installed C PointerGestures rendering consumer passed owned geometry and "
         "bounds.");
    return 0;
}
