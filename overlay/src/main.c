// cef-overlay: CEF off-screen rendering into our own transparent Wayland
// surface. OSR preserves per-pixel alpha, which windowed CEF on Wayland
// cannot provide; we own the wl_surface so input region and buffer
// lifecycle stay under PartyDeck's control.
#define _GNU_SOURCE
#include <errno.h>
#include <poll.h>
#include <stdarg.h>
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

#define FALLBACK_W 1280
#define FALLBACK_H 800

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
static int buffer_stale[2];
static int cur_w = FALLBACK_W;
static int cur_h = FALLBACK_H;
static int pending_w;
static int pending_h;
static void* shm_map;
static size_t shm_map_size;
static int configured;
static int running = 1;

static cef_browser_t* g_browser;

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

static void buffer_release(void* data, struct wl_buffer* b) {
    buffer_busy[(intptr_t)data] = 0;
}
static const struct wl_buffer_listener buffer_listener_0 = {buffer_release};

static int create_buffers(void) {
    int stride = cur_w * 4;
    size_t bufsz = (size_t)stride * cur_h;
    int fd = memfd_create("overlay-shm", 0);
    if (fd < 0 || ftruncate(fd, bufsz * 2) < 0) {
        fprintf(stderr, "cef-overlay: shm alloc failed\n");
        return 0;
    }
    void* map = mmap(NULL, bufsz * 2, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (map == MAP_FAILED) {
        close(fd);
        return 0;
    }
    struct wl_shm_pool* pool = wl_shm_create_pool(shm, fd, bufsz * 2);
    for (int i = 0; i < 2; i++) {
        pixels[i] = (char*)map + i * bufsz;
        buffers[i] = wl_shm_pool_create_buffer(pool, i * bufsz, cur_w, cur_h, stride,
                                               WL_SHM_FORMAT_ARGB8888);
        wl_buffer_add_listener(buffers[i], &buffer_listener_0, (void*)(intptr_t)i);
        buffer_busy[i] = 0;
        // Force a full-frame copy on first use; dirty rects from CEF assume
        // the buffer already holds the previous frame.
        buffer_stale[i] = 1;
    }
    wl_shm_pool_destroy(pool);
    close(fd);
    shm_map = map;
    shm_map_size = bufsz * 2;
    return 1;
}

static void destroy_buffers(void) {
    for (int i = 0; i < 2; i++) {
        wl_buffer_destroy(buffers[i]);
        buffers[i] = NULL;
    }
    munmap(shm_map, shm_map_size);
    shm_map = NULL;
}

static void xdg_configure(void* d, struct xdg_surface* s, uint32_t serial) {
    xdg_surface_ack_configure(s, serial);
    configured = 1;
    if (!pending_w || !pending_h || (pending_w == cur_w && pending_h == cur_h)) {
        return;
    }
    cur_w = pending_w;
    cur_h = pending_h;
    fprintf(stderr, "cef-overlay: size %dx%d\n", cur_w, cur_h);
    if (!shm_map) {
        return; // pre-map: wayland_init creates the buffers at cur_w/cur_h
    }
    destroy_buffers();
    if (!create_buffers()) {
        running = 0;
        return;
    }
    if (g_browser) {
        cef_browser_host_t* host = g_browser->get_host(g_browser);
        host->was_resized(host);
        host->base.release(&host->base);
    }
}
static const struct xdg_surface_listener xsurface_listener = {xdg_configure};

static void toplevel_configure(void* d, struct xdg_toplevel* t, int32_t w, int32_t h,
                               struct wl_array* states) {
    if (w > 0 && h > 0) {
        pending_w = w;
        pending_h = h;
    }
}
static void toplevel_close(void* d, struct xdg_toplevel* t) { running = 0; }
static const struct xdg_toplevel_listener toplevel_listener = {toplevel_configure, toplevel_close};

static int wayland_init(void) {
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

    return create_buffers();
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
static char g_ctl_path[256];

static int g_debug;

static void dbg(const char* fmt, ...) {
    if (!g_debug) {
        return;
    }
    va_list ap;
    va_start(ap, fmt);
    fputs("cef-overlay: dbg: ", stderr);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    va_end(ap);
}

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
    if (connect(fd, (struct sockaddr*)&addr, sizeof(addr)) != 0) {
        dbg("push_state: connect failed: %s", strerror(errno));
        close(fd);
        return;
    }
    const char* req = "{\"cmd\":\"get_state\"}\n";
    if (write(fd, req, strlen(req)) <= 0) {
        dbg("push_state: write req failed: %s", strerror(errno));
        close(fd);
        return;
    }

    size_t cap = 65536;
    size_t len = 0;
    char* reply = malloc(cap);
    if (!reply) {
        dbg("push_state: alloc failed: %s", strerror(errno));
        close(fd);
        return;
    }
    const size_t max_reply = 4 * 1024 * 1024;
    int done = 0;
    int terminated_by_newline = 0;
    while (!done) {
        if (len + 1 >= cap) {
            size_t ncap = cap * 2;
            char* grown;
            if (ncap > max_reply + 1) {
                dbg("push_state: reply exceeded max, bailing");
                free(reply);
                close(fd);
                return;
            }
            grown = realloc(reply, ncap);
            if (!grown) {
                dbg("push_state: alloc failed: %s", strerror(errno));
                free(reply);
                close(fd);
                return;
            }
            reply = grown;
            cap = ncap;
        }
        ssize_t n = read(fd, reply + len, cap - len - 1);
        if (n <= 0) {
            if (len == 0) {
                dbg("push_state: read returned no data: %s", strerror(errno));
                free(reply);
                close(fd);
                return;
            }
            break;
        }
        len += (size_t)n;
        if (memchr(reply + len - (size_t)n, '\n', (size_t)n)) {
            terminated_by_newline = 1;
            done = 1;
        }
    }
    close(fd);
    reply[len] = 0;

    static unsigned g_tick;
    int sample = (g_tick++ % 40) == 0;
    if (sample) {
        size_t tail_n = len < 80 ? len : 80;
        dbg("push_state: got %zu bytes, term=%s, head=<%.80s>, tail=<%.*s>",
            len, terminated_by_newline ? "nl" : "eof", reply,
            (int)tail_n, reply + len - tail_n);
    }

    static char* g_last_reply;
    static size_t g_last_reply_len;
    if (g_last_reply && g_last_reply_len == len && memcmp(g_last_reply, reply, len) == 0) {
        if (sample) {
            dbg("push_state: state unchanged, skip inject");
        }
        free(reply);
        return;
    }

    const char* prefix = "window.__pdState && window.__pdState(";
    const char* suffix = ")";
    size_t code_len = strlen(prefix) + len + strlen(suffix);
    char* code = malloc(code_len + 1);
    if (!code) {
        free(reply);
        return;
    }
    memcpy(code, prefix, strlen(prefix));
    memcpy(code + strlen(prefix), reply, len);
    memcpy(code + strlen(prefix) + len, suffix, strlen(suffix) + 1);

    // reply is fully consumed into code; hand it to the dedup cache.
    free(g_last_reply);
    g_last_reply = reply;
    g_last_reply_len = len;

    if (sample) {
        dbg("push_state: injecting %zu byte payload", code_len);
    }

    cef_frame_t* frame = g_browser->get_main_frame(g_browser);
    if (frame) {
        cef_string_t js = {0};
        cef_string_utf8_to_utf16(code, code_len, &js);
        cef_string_t origin = {0};
        frame->execute_java_script(frame, &js, &origin, 0);
        cef_string_clear(&js);
        frame->base.release(&frame->base);
    }
    free(code);
}

static void CEF_CALLBACK get_view_rect(cef_render_handler_t* self, cef_browser_t* browser,
                                       cef_rect_t* rect) {
    rect->x = 0;
    rect->y = 0;
    rect->width = cur_w;
    rect->height = cur_h;
}

static void CEF_CALLBACK on_paint(cef_render_handler_t* self, cef_browser_t* browser,
                                  cef_paint_element_type_t type, size_t n_rects,
                                  const cef_rect_t* rects, const void* buffer,
                                  int width, int height) {
    // Stale-size paints can still arrive after was_resized; drop them and
    // wait for the repaint at the current size.
    if (type != PET_VIEW || width != cur_w || height != cur_h) {
        return;
    }
    int stride = width * 4;
    int idx = !buffer_busy[0] ? 0 : (!buffer_busy[1] ? 1 : -1);
    if (idx < 0) {
        // Both busy: skip and make sure both resync from CEF's next full frame.
        buffer_stale[0] = buffer_stale[1] = 1;
        return;
    }
    // CEF hands us the full BGRA frame each paint, so a buffer that missed
    // frames while busy can always be healed with one full copy.
    if (buffer_stale[idx]) {
        memcpy(pixels[idx], buffer, (size_t)stride * height);
        buffer_stale[idx] = 0;
        buffer_busy[idx] = 1;
        wl_surface_attach(surface, buffers[idx], 0, 0);
        wl_surface_damage_buffer(surface, 0, 0, width, height);
        wl_surface_commit(surface);
        wl_display_flush(dpy);
        buffer_stale[idx ^ 1] = 1;
        return;
    }
    for (size_t i = 0; i < n_rects; i++) {
        const cef_rect_t* r = &rects[i];
        for (int y = r->y; y < r->y + r->height; y++) {
            memcpy((char*)pixels[idx] + y * stride + r->x * 4,
                   (const char*)buffer + y * stride + r->x * 4,
                   (size_t)r->width * 4);
        }
    }
    // The other buffer misses this frame's rects; it must resync before reuse.
    buffer_stale[idx ^ 1] = 1;
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
    g_debug = getenv("PARTYDECK_OVERLAY_DEBUG") != NULL;
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
    // Per-pid cache: a poisoned cache from a crashed run makes the GPU process
    // crashloop on the next start. /tmp is tmpfs on SteamOS; reboot reaps them.
    static char cache_path[64];
    snprintf(cache_path, sizeof(cache_path), "/tmp/cef-overlay-cache-%d", getpid());
    cef_string_utf8_to_utf16(cache_path, strlen(cache_path), &settings.root_cache_path);

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

    printf("cef-overlay: running\n");
    fflush(stdout);

    // External pump: interleave wayland dispatch with CEF work. Crude fixed
    // cadence is fine for an overlay (no interactive input yet).
    struct pollfd pfd = {.fd = wl_display_get_fd(dpy), .events = POLLIN};
    int ticks = 0;
    while (running) {
        if (wl_display_dispatch_pending(dpy) < 0) {
            fprintf(stderr, "cef-overlay: wayland dispatch failed, shutting down\n");
            break;
        }
        wl_display_flush(dpy);
        if (poll(&pfd, 1, 4) > 0) {
            if (pfd.revents & (POLLHUP | POLLERR)) {
                fprintf(stderr, "cef-overlay: compositor gone, shutting down\n");
                break;
            }
            if ((pfd.revents & POLLIN) && wl_display_dispatch(dpy) < 0) {
                fprintf(stderr, "cef-overlay: wayland dispatch failed, shutting down\n");
                break;
            }
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
