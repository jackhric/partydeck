#ifndef PD_OVERLAY_H
#define PD_OVERLAY_H

#include <stddef.h>

/* Wire contract with partydeck (launcher) and partydeck-comp (control socket). */
#define CTL_SOCKET_ENV "PARTYDECK_CTL_SOCKET"
#define WL_OVERLAY_SUFFIX "-overlay" /* WAYLAND_DISPLAY=<prefix>-overlay */
#define CTL_SOCKET_SUFFIX ".ctl"     /* fallback: $XDG_RUNTIME_DIR/<prefix>.ctl */
#define CTL_GET_STATE "{\"cmd\":\"get_state\"}\n"
#define CTL_JS_PREFIX "window.__pdState && window.__pdState("
#define CTL_JS_SUFFIX ")"

/* ctl.c: control socket client. */
void ctl_init(void);
/* Fetch the get_state reply. Returns a pointer valid until the next call when
   the state changed since the last call, NULL when unchanged or unavailable. */
const char* ctl_poll_state(size_t* len);

/* wl_shm.c: transparent xdg toplevel backed by two wl_shm buffers. */
int wl_init(void);
void wl_fini(void);
/* Dispatch pending events and wait up to timeout_ms for more. Returns 0 once
   the compositor is gone or asked us to close. */
int wl_pump(int timeout_ms);
int wl_width(void);
int wl_height(void);
/* Begin a frame at the current size. Returns the BGRA pixel buffer to write,
   or NULL when no buffer is free. *full_copy is set when the buffer does not
   hold the previous frame and must be rewritten in full. */
void* wl_begin_frame(int* full_copy);
void wl_damage(int x, int y, int w, int h);
void wl_end_frame(void);
/* Implemented by main.c: the buffers were reallocated for a new size. */
void overlay_resized(void);

#endif
