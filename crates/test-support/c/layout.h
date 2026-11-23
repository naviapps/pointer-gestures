#ifdef POINTER_GESTURES_REPORT_LAYOUT
#include <stdio.h>
#define REPORT_LAYOUT(type, name, value) printf("%s %s %zu\n", #type, name, value)
#else
#define REPORT_LAYOUT(type, name, value) ((void)0)
#endif

#define CHECK_SIZE(type, size) _Static_assert(sizeof(type) == size, #type " size")

#define CHECK_LAYOUT(type, size, alignment)                                                        \
    CHECK_SIZE(type, size);                                                                        \
    _Static_assert(_Alignof(type) == alignment, #type " alignment");                               \
    REPORT_LAYOUT(type, "size", sizeof(type));                                                     \
    REPORT_LAYOUT(type, "alignment", _Alignof(type))

#define CHECK_OFFSET(type, member, offset)                                                         \
    _Static_assert(offsetof(type, member) == offset, #type "." #member " offset");                 \
    REPORT_LAYOUT(type, #member, offsetof(type, member))
