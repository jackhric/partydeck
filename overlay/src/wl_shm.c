#define _GNU_SOURCE
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <wayland-client.h>
#include "xdg-shell-client-protocol.h"

#include "overlay.h"

#define FALLBACK_W 1280
#define FALLBACK_H 800

static struct wl_display* dpy;
static struct wl_compositor* compositor;
static struct wl_shm* shm;
static struct xdg_wm_base* wm_base;
static struct wl_surface* surface;
static struct xdg_surface* xsurface;
static struct xdg_toplevel* toplevel;

static struct wl_buffer* buffers[2];
static void* pixels[2];
static int buffer_busy[2];
static int buffer_stale[2];
static int frame_idx;
static void* shm_map;
static size_t shm_map_size;

static int cur_w = FALLBACK_W;
static int cur_h = FALLBACK_H;
static int pending_w;
static int pending_h;
static int configured;
static int running = 1;

// ---- globals and shell listeners ----

static void registry_global(void* d, struct wl_registry* reg, uint32_t name,
                            const char* iface, uint32_t ver) {
    (void)d;
    (void)ver;
    if (!strcmp(iface, wl_compositor_interface.name)) {
        compositor = wl_registry_bind(reg, name, &wl_compositor_interface, 4);
    } else if (!strcmp(iface, wl_shm_interface.name)) {
        shm = wl_registry_bind(reg, name, &wl_shm_interface, 1);
    } else if (!strcmp(iface, xdg_wm_base_interface.name)) {
        wm_base = wl_registry_bind(reg, name, &xdg_wm_base_interface, 1);
    }
}
static void registry_remove(void* d, struct wl_registry* r, uint32_t n) {
    (void)d;
    (void)r;
    (void)n;
}
static const struct wl_registry_listener registry_listener = {
    .global = registry_global,
    .global_remove = registry_remove,
};

static void wm_ping(void* d, struct xdg_wm_base* b, uint32_t serial) {
    (void)d;
    xdg_wm_base_pong(b, serial);
}
static const struct xdg_wm_base_listener wm_listener = {.ping = wm_ping};

// ---- shm buffers ----

static void buffer_release(void* data, struct wl_buffer* b) {
    (void)b;
    buffer_busy[(intptr_t)data] = 0;
}
static const struct wl_buffer_listener buffer_listener = {.release = buffer_release};

static void destroy_buffers(void) {
    for (int i = 0; i < 2; i++) {
        if (buffers[i]) {
            wl_buffer_destroy(buffers[i]);
        }
        buffers[i] = NULL;
        pixels[i] = NULL;
    }
    if (shm_map) {
        munmap(shm_map, shm_map_size);
    }
    shm_map = NULL;
    shm_map_size = 0;
}

static int create_buffers(void) {
    int stride = cur_w * 4;
    size_t bufsz = (size_t)stride * cur_h;
    size_t total = bufsz * 2;
    int fd = memfd_create("overlay-shm", 0);
    if (fd < 0) {
        fprintf(stderr, "cef-overlay: memfd_create failed\n");
        return 0;
    }
    if (ftruncate(fd, (off_t)total) < 0) {
        fprintf(stderr, "cef-overlay: shm alloc of %zu bytes failed\n", total);
        close(fd);
        return 0;
    }
    void* map = mmap(NULL, total, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (map == MAP_FAILED) {
        close(fd);
        return 0;
    }
    struct wl_shm_pool* pool = wl_shm_create_pool(shm, fd, (int32_t)total);
    close(fd);
    if (!pool) {
        munmap(map, total);
        return 0;
    }
    shm_map = map;
    shm_map_size = total;
    for (int i = 0; i < 2; i++) {
        pixels[i] = (char*)map + i * bufsz;
        buffers[i] = wl_shm_pool_create_buffer(pool, (int32_t)(i * bufsz), cur_w, cur_h, stride,
                                               WL_SHM_FORMAT_ARGB8888);
        if (!buffers[i]) {
            wl_shm_pool_destroy(pool);
            destroy_buffers();
            return 0;
        }
        wl_buffer_add_listener(buffers[i], &buffer_listener, (void*)(intptr_t)i);
        buffer_busy[i] = 0;
        // Force a full-frame copy on first use; dirty rects from CEF assume
        // the buffer already holds the previous frame.
        buffer_stale[i] = 1;
    }
    wl_shm_pool_destroy(pool);
    return 1;
}

// ---- configure / resize ----

static void xdg_configure(void* d, struct xdg_surface* s, uint32_t serial) {
    (void)d;
    xdg_surface_ack_configure(s, serial);
    configured = 1;
    if (!pending_w || !pending_h || (pending_w == cur_w && pending_h == cur_h)) {
        return;
    }
    cur_w = pending_w;
    cur_h = pending_h;
    fprintf(stderr, "cef-overlay: size %dx%d\n", cur_w, cur_h);
    if (!shm_map) {
        return; // before the first map: wl_init creates the buffers at cur_w/cur_h
    }
    destroy_buffers();
    if (!create_buffers()) {
        // pixels[]/buffers[] are NULL now; wl_begin_frame refuses paints until exit.
        fprintf(stderr, "cef-overlay: cannot allocate buffers, shutting down\n");
        running = 0;
        return;
    }
    overlay_resized();
}
static const struct xdg_surface_listener xsurface_listener = {.configure = xdg_configure};

static void toplevel_configure(void* d, struct xdg_toplevel* t, int32_t w, int32_t h,
                               struct wl_array* states) {
    (void)d;
    (void)t;
    (void)states;
    if (w > 0 && h > 0) {
        pending_w = w;
        pending_h = h;
    }
}
static void toplevel_close(void* d, struct xdg_toplevel* t) {
    (void)d;
    (void)t;
    running = 0;
}
static const struct xdg_toplevel_listener toplevel_listener = {
    .configure = toplevel_configure,
    .close = toplevel_close,
};

// ---- public API ----

int wl_init(void) {
    dpy = wl_display_connect(NULL);
    if (!dpy) {
        fprintf(stderr, "cef-overlay: cannot connect wayland display\n");
        return 0;
    }
    struct wl_registry* reg = wl_display_get_registry(dpy);
    wl_registry_add_listener(reg, &registry_listener, NULL);
    wl_display_roundtrip(dpy);
    if (!compositor || !shm || !wm_base) {
        fprintf(stderr, "cef-overlay: missing globals\n");
        return 0;
    }
    xdg_wm_base_add_listener(wm_base, &wm_listener, NULL);

    surface = wl_compositor_create_surface(compositor);
    // Empty input region: the overlay is display-only, all input must reach
    // the game surfaces underneath.
    struct wl_region* empty = wl_compositor_create_region(compositor);
    wl_surface_set_input_region(surface, empty);
    wl_region_destroy(empty);

    xsurface = xdg_wm_base_get_xdg_surface(wm_base, surface);
    xdg_surface_add_listener(xsurface, &xsurface_listener, NULL);
    toplevel = xdg_surface_get_toplevel(xsurface);
    xdg_toplevel_add_listener(toplevel, &toplevel_listener, NULL);
    xdg_toplevel_set_title(toplevel, "partydeck-overlay");
    xdg_toplevel_set_app_id(toplevel, "partydeck-overlay");
    wl_surface_commit(surface);

    // Wait for the first configure so the buffers are allocated at the size
    // the compositor assigns, not the fallback.
    while (!configured) {
        if (wl_display_dispatch(dpy) < 0) {
            fprintf(stderr, "cef-overlay: wayland dispatch failed before first configure\n");
            return 0;
        }
    }
    return create_buffers();
}

void wl_fini(void) {
    if (!dpy) {
        return;
    }
    destroy_buffers();
    if (toplevel) {
        xdg_toplevel_destroy(toplevel);
    }
    if (xsurface) {
        xdg_surface_destroy(xsurface);
    }
    if (surface) {
        wl_surface_destroy(surface);
    }
    wl_display_flush(dpy);
    wl_display_disconnect(dpy);
    dpy = NULL;
}

int wl_pump(int timeout_ms) {
    if (wl_display_dispatch_pending(dpy) < 0) {
        fprintf(stderr, "cef-overlay: wayland dispatch failed, shutting down\n");
        return 0;
    }
    wl_display_flush(dpy);
    struct pollfd pfd = {.fd = wl_display_get_fd(dpy), .events = POLLIN};
    if (poll(&pfd, 1, timeout_ms) > 0) {
        if (pfd.revents & (POLLHUP | POLLERR)) {
            fprintf(stderr, "cef-overlay: compositor gone, shutting down\n");
            return 0;
        }
        if ((pfd.revents & POLLIN) && wl_display_dispatch(dpy) < 0) {
            fprintf(stderr, "cef-overlay: wayland dispatch failed, shutting down\n");
            return 0;
        }
    }
    return running;
}

int wl_width(void) { return cur_w; }
int wl_height(void) { return cur_h; }

void* wl_begin_frame(int* full_copy) {
    if (!buffers[0] || !buffers[1]) {
        return NULL;
    }
    int idx = !buffer_busy[0] ? 0 : (!buffer_busy[1] ? 1 : -1);
    if (idx < 0) {
        // Both held by the compositor: skip this frame and make sure both
        // resync from CEF's next full frame.
        buffer_stale[0] = buffer_stale[1] = 1;
        return NULL;
    }
    frame_idx = idx;
    *full_copy = buffer_stale[idx];
    return pixels[idx];
}

void wl_damage(int x, int y, int w, int h) {
    wl_surface_damage_buffer(surface, x, y, w, h);
}

void wl_end_frame(void) {
    int idx = frame_idx;
    buffer_stale[idx] = 0;
    // The other buffer missed this frame's rects; it must resync before reuse.
    buffer_stale[idx ^ 1] = 1;
    buffer_busy[idx] = 1;
    wl_surface_attach(surface, buffers[idx], 0, 0);
    wl_surface_commit(surface);
    wl_display_flush(dpy);
}
