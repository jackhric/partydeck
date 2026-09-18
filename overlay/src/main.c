#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "include/capi/cef_app_capi.h"
#include "include/capi/cef_browser_capi.h"
#include "include/capi/cef_client_capi.h"
#include "include/capi/cef_life_span_handler_capi.h"
#include "include/capi/cef_render_handler_capi.h"

#include "overlay.h"

#define PUSH_EVERY_TICKS 50 /* roughly 4-5 state pushes per second at a 4 ms pump */
#define CLOSE_PUMP_ITERS 100

static cef_render_handler_t g_render_handler;
static cef_life_span_handler_t g_life_span_handler;
static cef_client_t g_client;
static cef_browser_t* g_browser;

// ---- statically allocated CEF objects: no-op ref counting ----

static void CEF_CALLBACK noop_add_ref(cef_base_ref_counted_t* s) { (void)s; }
static int CEF_CALLBACK noop_release(cef_base_ref_counted_t* s) {
    (void)s;
    return 0;
}
static int CEF_CALLBACK noop_has_one_ref(cef_base_ref_counted_t* s) {
    (void)s;
    return 1;
}
static int CEF_CALLBACK noop_has_refs(cef_base_ref_counted_t* s) {
    (void)s;
    return 1;
}
static void init_base(cef_base_ref_counted_t* b, size_t size) {
    b->size = size;
    b->add_ref = noop_add_ref;
    b->release = noop_release;
    b->has_one_ref = noop_has_one_ref;
    b->has_at_least_one_ref = noop_has_refs;
}

// ---- life span ----

static void CEF_CALLBACK on_after_created(cef_life_span_handler_t* self, cef_browser_t* browser) {
    (void)self;
    browser->base.add_ref(&browser->base);
    g_browser = browser;
}

static void CEF_CALLBACK on_before_close(cef_life_span_handler_t* self, cef_browser_t* browser) {
    (void)self;
    (void)browser;
    if (g_browser) {
        g_browser->base.release(&g_browser->base);
        g_browser = NULL;
    }
}

// Ask the browser to close and pump CEF until it reports on_before_close, so
// cef_shutdown does not run with a live browser.
static void close_browser(void) {
    if (!g_browser) {
        return;
    }
    cef_browser_host_t* host = g_browser->get_host(g_browser);
    host->close_browser(host, 1);
    host->base.release(&host->base);
    for (int i = 0; i < CLOSE_PUMP_ITERS && g_browser; i++) {
        cef_do_message_loop_work();
        usleep(10 * 1000);
    }
}

// ---- state push ----

// Hand the raw state JSON to the page as window.__pdState(<state>). The reply
// is already JSON, so the C side never parses it.
static void push_state(void) {
    if (!g_browser) {
        return;
    }
    size_t len = 0;
    const char* reply = ctl_poll_state(&len);
    if (!reply) {
        return;
    }
    size_t code_len = strlen(CTL_JS_PREFIX) + len + strlen(CTL_JS_SUFFIX);
    char* code = malloc(code_len + 1);
    if (!code) {
        return;
    }
    memcpy(code, CTL_JS_PREFIX, strlen(CTL_JS_PREFIX));
    memcpy(code + strlen(CTL_JS_PREFIX), reply, len);
    memcpy(code + strlen(CTL_JS_PREFIX) + len, CTL_JS_SUFFIX, strlen(CTL_JS_SUFFIX) + 1);

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

// ---- off-screen rendering ----

static void CEF_CALLBACK get_view_rect(cef_render_handler_t* self, cef_browser_t* browser,
                                       cef_rect_t* rect) {
    (void)self;
    (void)browser;
    rect->x = 0;
    rect->y = 0;
    rect->width = wl_width();
    rect->height = wl_height();
}

static void CEF_CALLBACK on_paint(cef_render_handler_t* self, cef_browser_t* browser,
                                  cef_paint_element_type_t type, size_t n_rects,
                                  const cef_rect_t* rects, const void* buffer,
                                  int width, int height) {
    (void)self;
    (void)browser;
    // Stale-size paints can still arrive after was_resized; drop them and
    // wait for the repaint at the current size.
    if (type != PET_VIEW || width != wl_width() || height != wl_height()) {
        return;
    }
    int full = 0;
    char* dst = wl_begin_frame(&full);
    if (!dst) {
        return;
    }
    size_t stride = (size_t)width * 4;
    if (full) {
        // CEF hands us the full BGRA frame each paint, so a buffer that missed
        // frames while busy is healed with one full copy.
        memcpy(dst, buffer, stride * (size_t)height);
        wl_damage(0, 0, width, height);
    } else {
        for (size_t i = 0; i < n_rects; i++) {
            const cef_rect_t* r = &rects[i];
            for (int y = r->y; y < r->y + r->height; y++) {
                memcpy(dst + y * stride + (size_t)r->x * 4,
                       (const char*)buffer + y * stride + (size_t)r->x * 4,
                       (size_t)r->width * 4);
            }
            wl_damage(r->x, r->y, r->width, r->height);
        }
    }
    wl_end_frame();
}

void overlay_resized(void) {
    if (!g_browser) {
        return;
    }
    cef_browser_host_t* host = g_browser->get_host(g_browser);
    host->was_resized(host);
    host->base.release(&host->base);
}

static cef_render_handler_t* CEF_CALLBACK get_render_handler(cef_client_t* self) {
    (void)self;
    return &g_render_handler;
}

static cef_life_span_handler_t* CEF_CALLBACK get_life_span_handler(cef_client_t* self) {
    (void)self;
    return &g_life_span_handler;
}

// ---- main ----

int main(int argc, char** argv) {
    cef_api_hash(CEF_API_VERSION, 0);

    cef_main_args_t main_args = {argc, argv};
    int code = cef_execute_process(&main_args, NULL, NULL);
    if (code >= 0) {
        return code; // CEF helper process
    }

    if (!wl_init()) {
        return 1;
    }
    ctl_init();

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
        fprintf(stderr, "cef-overlay: cef_initialize failed\n");
        wl_fini();
        return 1;
    }

    init_base(&g_render_handler.base, sizeof(g_render_handler));
    g_render_handler.get_view_rect = get_view_rect;
    g_render_handler.on_paint = on_paint;
    init_base(&g_life_span_handler.base, sizeof(g_life_span_handler));
    g_life_span_handler.on_after_created = on_after_created;
    g_life_span_handler.on_before_close = on_before_close;
    init_base(&g_client.base, sizeof(g_client));
    g_client.get_render_handler = get_render_handler;
    g_client.get_life_span_handler = get_life_span_handler;

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

    int created = cef_browser_host_create_browser(&wi, &g_client, &cef_url, &bs, NULL, NULL);
    cef_string_clear(&cef_url);
    if (!created) {
        fprintf(stderr, "cef-overlay: create_browser failed\n");
        cef_shutdown();
        wl_fini();
        return 1;
    }
    fprintf(stderr, "cef-overlay: running\n");

    // External pump: interleave wayland dispatch with CEF work. A fixed
    // cadence is fine for an overlay with no interactive input.
    int ticks = 0;
    while (wl_pump(4)) {
        cef_do_message_loop_work();
        if (++ticks >= PUSH_EVERY_TICKS) {
            ticks = 0;
            push_state();
        }
    }

    close_browser();
    cef_shutdown();
    wl_fini();
    return 0;
}
