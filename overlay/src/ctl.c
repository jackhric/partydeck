#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <unistd.h>

#include "overlay.h"

#define MAX_REPLY (4 * 1024 * 1024)

static char g_path[sizeof(((struct sockaddr_un*)0)->sun_path)];
static char* g_last;
static size_t g_last_len;

void ctl_init(void) {
    const char* explicit_path = getenv(CTL_SOCKET_ENV);
    if (explicit_path && *explicit_path) {
        snprintf(g_path, sizeof(g_path), "%s", explicit_path);
        return;
    }
    // Older launchers only set WAYLAND_DISPLAY=<prefix>-overlay.
    const char* wl = getenv("WAYLAND_DISPLAY");
    const char* rt = getenv("XDG_RUNTIME_DIR");
    const char* suffix = wl ? strstr(wl, WL_OVERLAY_SUFFIX) : NULL;
    if (rt && suffix) {
        snprintf(g_path, sizeof(g_path), "%s/%.*s" CTL_SOCKET_SUFFIX, rt, (int)(suffix - wl), wl);
    }
    if (!g_path[0]) {
        fprintf(stderr, "cef-overlay: no control socket (set %s); state will not be pushed\n",
                CTL_SOCKET_ENV);
    }
}

// Read one newline-terminated reply (or up to EOF). Returns a malloc'd,
// NUL-terminated buffer or NULL.
static char* read_reply(int fd, size_t* out_len) {
    size_t cap = 65536;
    size_t len = 0;
    char* buf = malloc(cap);
    if (!buf) {
        return NULL;
    }
    for (;;) {
        if (len + 1 >= cap) {
            size_t ncap = cap * 2;
            if (ncap > MAX_REPLY + 1) {
                fprintf(stderr, "cef-overlay: get_state reply exceeds %d bytes, dropped\n", MAX_REPLY);
                free(buf);
                return NULL;
            }
            char* grown = realloc(buf, ncap);
            if (!grown) {
                free(buf);
                return NULL;
            }
            buf = grown;
            cap = ncap;
        }
        ssize_t n = read(fd, buf + len, cap - len - 1);
        if (n <= 0) {
            break;
        }
        len += (size_t)n;
        if (memchr(buf + len - (size_t)n, '\n', (size_t)n)) {
            break;
        }
    }
    if (len == 0) {
        free(buf);
        return NULL;
    }
    buf[len] = 0;
    *out_len = len;
    return buf;
}

const char* ctl_poll_state(size_t* len) {
    if (!g_path[0]) {
        return NULL;
    }
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        return NULL;
    }
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    memcpy(addr.sun_path, g_path, sizeof(addr.sun_path));
    struct timeval tv = {.tv_sec = 1};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    if (connect(fd, (struct sockaddr*)&addr, sizeof(addr)) != 0 ||
        write(fd, CTL_GET_STATE, strlen(CTL_GET_STATE)) <= 0) {
        close(fd);
        return NULL;
    }
    size_t n = 0;
    char* reply = read_reply(fd, &n);
    close(fd);
    if (!reply) {
        return NULL;
    }
    if (g_last && g_last_len == n && memcmp(g_last, reply, n) == 0) {
        free(reply);
        return NULL;
    }
    free(g_last);
    g_last = reply;
    g_last_len = n;
    *len = n;
    return reply;
}
