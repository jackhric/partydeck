// cef-shell v2: CEF off-screen rendering into our own transparent Wayland
// surface. OSR preserves per-pixel alpha, which windowed CEF on Wayland
// cannot provide; we own the wl_surface so input region and buffer
// lifecycle stay under PartyDeck's control.
#define _GNU_SOURCE
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <wayland-client.h>
#include "xdg-shell-client-protocol.h"

#include <sys/socket.h>
#include <sys/un.h>

#include "include/capi/cef_app_capi.h"
#include "include/capi/cef_browser_capi.h"
#include "include/capi/cef_client_capi.h"
#include "include/capi/cef_life_span_handler_capi.h"
#include "include/capi/cef_render_handler_capi.h"

#define W 1280
#define H 800
#define STRIDE (W * 4)
#define BUFSZ (STRIDE * H)

// ── wayland state ────────────────────────────────────────────────────
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
static int configured;
static int running = 1;

static void registry_global(void* d, struct wl_registry* reg, uint32_t name,
                            const char* iface, uint32_t ver) {
    if (!strcmp(iface, wl_compositor_interface.name)) {
        compositor = wl_registry_bind(reg, name, &wl_compositor_interface, 4);
    } else if (!strcmp(iface, wl_shm_interface.name)) {
        shm = wl_registry_bind(reg, name, &wl_shm_interface, 1);
    } else if (!strcmp(iface, xdg_wm_base_interface.name)) {
        wm_base = wl_registry_bind(reg, name, &xdg_wm_base_interface, 1);
    }
}
static void registry_remove(void* d, struct wl_registry* r, uint32_t n) {}
static const struct wl_registry_listener registry_listener = {registry_global, registry_remove};

static void wm_ping(void* d, struct xdg_wm_base* b, uint32_t serial) {
    xdg_wm_base_pong(b, serial);
}
static const struct xdg_wm_base_listener wm_listener = {wm_ping};

static void xdg_configure(void* d, struct xdg_surface* s, uint32_t serial) {
    xdg_surface_ack_configure(s, serial);
    configured = 1;
}
static const struct xdg_surface_listener xsurface_listener = {xdg_configure};

static void toplevel_configure(void* d, struct xdg_toplevel* t, int32_t w, int32_t h,
                               struct wl_array* states) {}
static void toplevel_close(void* d, struct xdg_toplevel* t) { running = 0; }
static const struct xdg_toplevel_listener toplevel_listener = {toplevel_configure, toplevel_close};

static void buffer_release(void* data, struct wl_buffer* b) {
    buffer_busy[(intptr_t)data] = 0;
}
static const struct wl_buffer_listener buffer_listener_0 = {buffer_release};

static int wayland_init(void) {
    dpy = wl_display_connect(NULL);
    if (!dpy) {
        fprintf(stderr, "cef-shell2: cannot connect wayland display\n");
        return 0;
    }
    struct wl_registry* reg = wl_display_get_registry(dpy);
    wl_registry_add_listener(reg, &registry_listener, NULL);
    wl_display_roundtrip(dpy);
    if (!compositor || !shm || !wm_base) {
        fprintf(stderr, "cef-shell2: missing globals\n");
        return 0;
    }
    xdg_wm_base_add_listener(wm_base, &wm_listener, NULL);

    surface = wl_compositor_create_surface(compositor);
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

    while (!configured) {
        wl_display_dispatch(dpy);
    }

    int fd = memfd_create("overlay-shm", 0);
    if (fd < 0 || ftruncate(fd, BUFSZ * 2) < 0) {
        fprintf(stderr, "cef-shell2: shm alloc failed\n");
        return 0;
    }
    void* map = mmap(NULL, BUFSZ * 2, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (map == MAP_FAILED) {
        return 0;
    }
    struct wl_shm_pool* pool = wl_shm_create_pool(shm, fd, BUFSZ * 2);
    for (int i = 0; i < 2; i++) {
        pixels[i] = (char*)map + i * BUFSZ;
        memset(pixels[i], 0, BUFSZ);
        buffers[i] = wl_shm_pool_create_buffer(pool, i * BUFSZ, W, H, STRIDE, WL_SHM_FORMAT_ARGB8888);
        wl_buffer_add_listener(buffers[i], &buffer_listener_0, (void*)(intptr_t)i);
    }
    return 1;
}

// ── CEF glue ─────────────────────────────────────────────────────────
static void CEF_CALLBACK noop_add_ref(cef_base_ref_counted_t* s) {}
static int CEF_CALLBACK noop_release(cef_base_ref_counted_t* s) { return 0; }
static int CEF_CALLBACK noop_has_one_ref(cef_base_ref_counted_t* s) { return 1; }
static int CEF_CALLBACK noop_has_refs(cef_base_ref_counted_t* s) { return 1; }
static void init_base(cef_base_ref_counted_t* b, size_t size) {
    b->size = size;
    b->add_ref = noop_add_ref;
    b->release = noop_release;
    b->has_one_ref = noop_has_one_ref;
    b->has_at_least_one_ref = noop_has_refs;
}

static cef_render_handler_t g_render_handler;
static cef_life_span_handler_t g_life_span_handler;
static cef_client_t g_client;
static cef_browser_t* g_browser;
static char g_ctl_path[256];

static void CEF_CALLBACK on_after_created(cef_life_span_handler_t* self, cef_browser_t* browser) {
    browser->base.add_ref(&browser->base);
    g_browser = browser;
}

// Poll the compositor's control socket and hand the raw state JSON to the
// page: window.__pdState(<state>). The reply is already JSON, so the C side
// never parses it.
static void push_state(void) {
    if (!g_browser || !g_ctl_path[0]) {
        return;
    }
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        return;
    }
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    strncpy(addr.sun_path, g_ctl_path, sizeof(addr.sun_path) - 1);
    struct timeval tv = {.tv_sec = 1};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    static char reply[16384];
    ssize_t n = -1;
    if (connect(fd, (struct sockaddr*)&addr, sizeof(addr)) == 0) {
        const char* req = "{\"cmd\":\"get_state\"}\n";
        if (write(fd, req, strlen(req)) > 0) {
            n = read(fd, reply, sizeof(reply) - 1);
        }
    }
    close(fd);
    if (n <= 0) {
        return;
    }
    reply[n] = 0;
    static char code[17000];
    snprintf(code, sizeof(code), "window.__pdState && window.__pdState(%s)", reply);
    cef_frame_t* frame = g_browser->get_main_frame(g_browser);
    if (frame) {
        cef_string_t js = {0};
        cef_string_utf8_to_utf16(code, strlen(code), &js);
        cef_string_t origin = {0};
        frame->execute_java_script(frame, &js, &origin, 0);
        cef_string_clear(&js);
        frame->base.release(&frame->base);
    }
}

static void CEF_CALLBACK get_view_rect(cef_render_handler_t* self, cef_browser_t* browser,
                                       cef_rect_t* rect) {
    rect->x = 0;
    rect->y = 0;
    rect->width = W;
    rect->height = H;
}

static void CEF_CALLBACK on_paint(cef_render_handler_t* self, cef_browser_t* browser,
                                  cef_paint_element_type_t type, size_t n_rects,
                                  const cef_rect_t* rects, const void* buffer,
                                  int width, int height) {
    if (type != PET_VIEW || width != W || height != H) {
        return;
    }
    int idx = !buffer_busy[0] ? 0 : (!buffer_busy[1] ? 1 : -1);
    if (idx < 0) {
        return; // both busy; skip this frame, CEF will repaint
    }
    // CEF gives the full BGRA frame; copy only the damaged rows per rect.
    for (size_t i = 0; i < n_rects; i++) {
        const cef_rect_t* r = &rects[i];
        for (int y = r->y; y < r->y + r->height; y++) {
            memcpy((char*)pixels[idx] + y * STRIDE + r->x * 4,
                   (const char*)buffer + y * STRIDE + r->x * 4,
                   (size_t)r->width * 4);
        }
    }
    // The other buffer may hold older content; sync the damage there too on
    // the next flip via full copy of this frame's rects into both.
    int other = idx ^ 1;
    if (!buffer_busy[other]) {
        for (size_t i = 0; i < n_rects; i++) {
            const cef_rect_t* r = &rects[i];
            for (int y = r->y; y < r->y + r->height; y++) {
                memcpy((char*)pixels[other] + y * STRIDE + r->x * 4,
                       (const char*)buffer + y * STRIDE + r->x * 4,
                       (size_t)r->width * 4);
            }
        }
    }
    buffer_busy[idx] = 1;
    wl_surface_attach(surface, buffers[idx], 0, 0);
    for (size_t i = 0; i < n_rects; i++) {
        wl_surface_damage_buffer(surface, rects[i].x, rects[i].y, rects[i].width, rects[i].height);
    }
    wl_surface_commit(surface);
    wl_display_flush(dpy);
}

static cef_render_handler_t* CEF_CALLBACK get_render_handler(cef_client_t* self) {
    return &g_render_handler;
}

static cef_life_span_handler_t* CEF_CALLBACK get_life_span_handler(cef_client_t* self) {
    return &g_life_span_handler;
}

int main(int argc, char** argv) {
    cef_api_hash(CEF_API_VERSION, 0);

    cef_main_args_t main_args = {argc, argv};
    int code = cef_execute_process(&main_args, NULL, NULL);
    if (code >= 0) {
        return code;
    }

    if (!wayland_init()) {
        return 1;
    }

    cef_settings_t settings;
    memset(&settings, 0, sizeof(settings));
    settings.size = sizeof(settings);
    settings.no_sandbox = 1;
    settings.windowless_rendering_enabled = 1;
    settings.external_message_pump = 1;
    settings.log_severity = LOGSEVERITY_WARNING;
    settings.background_color = 0x00000000;
    cef_string_utf8_to_utf16("/tmp/cef-overlay-cache", strlen("/tmp/cef-overlay-cache"),
                             &settings.root_cache_path);

    if (!cef_initialize(&main_args, &settings, NULL, NULL)) {
        fprintf(stderr, "cef_initialize failed\n");
        return 1;
    }

    init_base(&g_render_handler.base, sizeof(g_render_handler));
    g_render_handler.get_view_rect = get_view_rect;
    g_render_handler.on_paint = on_paint;
    init_base(&g_life_span_handler.base, sizeof(g_life_span_handler));
    g_life_span_handler.on_after_created = on_after_created;
    init_base(&g_client.base, sizeof(g_client));
    g_client.get_render_handler = get_render_handler;
    g_client.get_life_span_handler = get_life_span_handler;

    // <prefix>-overlay -> <runtime dir>/<prefix>.ctl
    const char* wl = getenv("WAYLAND_DISPLAY");
    const char* rt = getenv("XDG_RUNTIME_DIR");
    if (wl && rt) {
        const char* suffix = strstr(wl, "-overlay");
        if (suffix) {
            snprintf(g_ctl_path, sizeof(g_ctl_path), "%s/%.*s.ctl", rt, (int)(suffix - wl), wl);
        }
    }

    cef_window_info_t wi;
    memset(&wi, 0, sizeof(wi));
    wi.size = sizeof(wi);
    wi.windowless_rendering_enabled = 1;
    wi.runtime_style = CEF_RUNTIME_STYLE_ALLOY;

    const char* url = getenv("OVERLAY_URL");
    if (!url || !*url) {
        url = "about:blank";
    }
    cef_string_t cef_url = {0};
    cef_string_utf8_to_utf16(url, strlen(url), &cef_url);

    cef_browser_settings_t bs;
    memset(&bs, 0, sizeof(bs));
    bs.size = sizeof(bs);
    bs.windowless_frame_rate = 60;
    bs.background_color = 0x00000000;

    if (!cef_browser_host_create_browser(&wi, &g_client, &cef_url, &bs, NULL, NULL)) {
        fprintf(stderr, "create_browser failed\n");
        return 1;
    }

    printf("cef-shell2: running\n");
    fflush(stdout);

    // External pump: interleave wayland dispatch with CEF work. Crude fixed
    // cadence is fine for an overlay (no interactive input yet).
    struct pollfd pfd = {.fd = wl_display_get_fd(dpy), .events = POLLIN};
    int ticks = 0;
    while (running) {
        wl_display_dispatch_pending(dpy);
        wl_display_flush(dpy);
        if (poll(&pfd, 1, 4) > 0 && (pfd.revents & POLLIN)) {
            wl_display_dispatch(dpy);
        }
        cef_do_message_loop_work();
        if (++ticks >= 50) { // roughly 4-5 state pushes per second
            ticks = 0;
            push_state();
        }
    }
    cef_shutdown();
    return 0;
}
