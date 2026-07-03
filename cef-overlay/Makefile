CEF_VERSION := 144.0.29+g0b1a012+chromium-144.0.7559.256
CEF_SHA256  := aafa3b996b748119920c9d0d7a7e3af8ffa2b9a49654e67dd7681b3cb6395a09
CEF_URL     := https://cef-builds.spotifycdn.com/cef_binary_$(subst +,%2B,$(CEF_VERSION))_linux64_minimal.tar.bz2

MAKEFILE_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
REPO_ROOT    := $(abspath $(MAKEFILE_DIR)/..)
CACHE_DIR    := $(REPO_ROOT)/dist/build_generated/cef
TARBALL      := $(CACHE_DIR)/cef.tar.bz2
DIST_DIR     := $(CACHE_DIR)/dist
LIBCEF       := $(DIST_DIR)/Release/libcef.so
STRIPPED     := $(CACHE_DIR)/libcef-stripped.so

BUILD_DIR := $(MAKEFILE_DIR)/build
BIN       := $(BUILD_DIR)/cef-overlay
OUT       ?= $(BUILD_DIR)/pkg
XDG_XML   := /usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml

DECK_HOST ?= deck@steamdeck.internal
DECK_PORT ?= 22
DECK_DIR  := /home/deck/homebrew/data/PartyDeck/bin/cef-overlay
HOLO_OUT  := dist/build_generated/holo/release/bin/cef-overlay

.PHONY: shell cef-dist package deploy deploy-ui clean

shell: $(BIN)

$(BUILD_DIR)/xdg-shell-client-protocol.h: $(XDG_XML)
	mkdir -p $(BUILD_DIR)
	wayland-scanner client-header $(XDG_XML) $@

$(BUILD_DIR)/xdg-shell-protocol.c: $(XDG_XML)
	mkdir -p $(BUILD_DIR)
	wayland-scanner private-code $(XDG_XML) $@

# The Deck resolves libcef's deps at runtime, the build host does not
# (hence --allow-shlib-undefined).
$(BIN): $(MAKEFILE_DIR)/src/main.c $(BUILD_DIR)/xdg-shell-protocol.c $(BUILD_DIR)/xdg-shell-client-protocol.h | cef-dist
	gcc -O2 -o $@ $(MAKEFILE_DIR)/src/main.c $(BUILD_DIR)/xdg-shell-protocol.c \
	    -I$(BUILD_DIR) -I$(DIST_DIR) -L$(DIST_DIR)/Release -lcef -lwayland-client \
	    -Wl,--allow-shlib-undefined -Wl,-rpath,'$$ORIGIN'

cef-dist:
	@mkdir -p $(CACHE_DIR)
	@if [ ! -f $(TARBALL) ] || ! printf '%s  %s\n' $(CEF_SHA256) $(TARBALL) | sha256sum -c --status; then \
	    curl -fL -o $(TARBALL) '$(CEF_URL)'; \
	    printf '%s  %s\n' $(CEF_SHA256) $(TARBALL) | sha256sum -c --status \
	        || { echo 'ERROR: cef.tar.bz2 sha256 mismatch' >&2; exit 1; }; \
	fi
	@if [ ! -e $(DIST_DIR)/.extracted ]; then \
	    rm -rf $(DIST_DIR); \
	    mkdir -p $(DIST_DIR); \
	    tar -xjf $(TARBALL) -C $(DIST_DIR) --strip-components=1; \
	    touch $(DIST_DIR)/.extracted; \
	fi
	@if [ ! -f $(STRIPPED) ] || [ $(LIBCEF) -nt $(STRIPPED) ]; then \
	    strip -o $(STRIPPED) $(LIBCEF); \
	fi

# Excluded on purpose: libEGL/libGLESv2/libvk_swiftshader/libvulkan (we run
# --disable-gpu), chrome-sandbox (--no-sandbox), the 219 other locales.
package: shell
	rm -rf $(OUT)
	mkdir -p $(OUT)/locales
	cp $(BIN) $(OUT)/cef-overlay
	cp $(STRIPPED) $(OUT)/libcef.so
	cp $(MAKEFILE_DIR)/ui/overlay.html $(OUT)/overlay.html
	cp $(DIST_DIR)/Release/v8_context_snapshot.bin \
	   $(DIST_DIR)/Resources/icudtl.dat \
	   $(DIST_DIR)/Resources/resources.pak \
	   $(DIST_DIR)/Resources/chrome_100_percent.pak \
	   $(DIST_DIR)/Resources/chrome_200_percent.pak \
	   $(OUT)/
	cp $(DIST_DIR)/Resources/locales/en-US.pak $(OUT)/locales/en-US.pak
	cp $(DIST_DIR)/LICENSE.txt $(OUT)/LICENSE.cef.txt
	du -sh $(OUT)

deploy:
	docker run --rm -v $(REPO_ROOT):/workspace partydeck-build \
	    make -C cef-overlay package OUT=/workspace/$(HOLO_OUT)
	rsync -a --delete -e 'ssh -p $(DECK_PORT)' \
	    $(REPO_ROOT)/$(HOLO_OUT)/ $(DECK_HOST):$(DECK_DIR)/

deploy-ui:
	rsync -e 'ssh -p $(DECK_PORT)' \
	    $(MAKEFILE_DIR)/ui/overlay.html $(DECK_HOST):$(DECK_DIR)/overlay.html

clean:
	rm -rf $(BUILD_DIR)
