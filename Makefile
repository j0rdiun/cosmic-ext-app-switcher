SWITCHER    := cosmic-ext-app-switcher
APPLET      := cosmic-ext-applet-app-switcher
INSTALL_DIR := $(HOME)/.local/bin
APPS_DIR    := $(HOME)/.local/share/applications
SWITCHER_BIN := target/release/$(SWITCHER)
APPLET_BIN  := target/release/$(APPLET)
DESKTOP     := applet/data/io.github.cosmic-ext-applet-app-switcher.desktop
ICON_SRC    := applet/data/io.github.cosmic-ext-applet-app-switcher-symbolic.svg
ICONS_DIR   := $(HOME)/.local/share/icons/hicolor/scalable/apps
GESTURES    := $(HOME)/.config/autostart/io.github.cosmic-ext-app-switcher-gestures.desktop
# The [c] keeps pkill from matching the shell that runs it, whose command line holds the pattern.
GESTURES_PS := [c]osmic-ext-app-switcher --gestures

.PHONY: all build install uninstall enable disable status check-compat enable-gestures disable-gestures

all: build

build:
	@command -v cargo >/dev/null 2>&1 || { \
		echo "Rust not found. Install with:"; \
		echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; \
		exit 1; \
	}
	cargo build --release --workspace

install: build
	@mkdir -p $(INSTALL_DIR) $(APPS_DIR) $(ICONS_DIR)
	install -Dm755 $(SWITCHER_BIN) $(INSTALL_DIR)/$(SWITCHER)
	install -Dm755 $(APPLET_BIN) $(INSTALL_DIR)/$(APPLET)
	install -Dm644 $(DESKTOP) $(APPS_DIR)/io.github.cosmic-ext-applet-app-switcher.desktop
	install -Dm644 $(ICON_SRC) $(ICONS_DIR)/io.github.cosmic-ext-applet-app-switcher-symbolic.svg
	@$(MAKE) enable
	@echo ""
	@echo "Installed and enabled. Press Super+Tab or Alt+Tab to try it."
	@echo "Add the 'App Switcher Settings' applet to your COSMIC panel to change themes."

uninstall:
	@$(MAKE) disable
	@$(MAKE) disable-gestures
	@rm -f $(INSTALL_DIR)/$(SWITCHER)
	@rm -f $(INSTALL_DIR)/$(APPLET)
	@rm -f $(APPS_DIR)/io.github.cosmic-ext-applet-app-switcher.desktop
	@rm -f $(ICONS_DIR)/io.github.cosmic-ext-applet-app-switcher-symbolic.svg
	@echo "Uninstalled. COSMIC default switcher restored."

enable:
	@bash scripts/enable.sh

disable:
	@bash scripts/disable.sh

enable-gestures:
	@test -x $(INSTALL_DIR)/$(SWITCHER) || { echo "Run 'make install' first."; exit 1; }
	@mkdir -p $(dir $(GESTURES))
	@printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=App Switcher Gestures' \
		'Comment=Three-finger swipe up shows the workspace overview, down hides it' \
		'Exec=$(INSTALL_DIR)/$(SWITCHER) --gestures' 'NoDisplay=true' > $(GESTURES)
	@pkill -f '$(GESTURES_PS)' || true
	@setsid -f $(INSTALL_DIR)/$(SWITCHER) --gestures >/dev/null 2>&1
	@sleep 0.5; pgrep -f '$(GESTURES_PS)' >/dev/null \
		&& echo "Gestures on: three-finger swipe up shows the workspace overview, down hides it." \
		|| { echo "Gesture daemon exited. Run '$(INSTALL_DIR)/$(SWITCHER) --gestures' to see why."; exit 1; }

disable-gestures:
	@rm -f $(GESTURES)
	@pkill -f '$(GESTURES_PS)' && echo "Gestures off." || true

status:
	@bash scripts/status.sh

check-compat:
	@echo "Checking compatibility..."
	@bash scripts/find-config.sh > /dev/null 2>&1; case $$? in \
		0) echo "  COSMIC shortcuts config: found" ;; \
		2) echo "  COSMIC shortcuts config: not created yet (run 'make enable')" ;; \
		*) echo "  COSMIC shortcuts config: NOT found" ;; \
	esac
	@test -f $(INSTALL_DIR)/$(SWITCHER) && echo "  Switcher binary: installed" || echo "  Switcher binary: not installed"
	@test -f $(INSTALL_DIR)/$(APPLET)   && echo "  Applet binary:   installed" || echo "  Applet binary:   not installed"
	@command -v cosmic-comp >/dev/null 2>&1 && echo "  cosmic-comp: found" || echo "  cosmic-comp: not found (is COSMIC running?)"
	@bin=$(SWITCHER_BIN); [ -x "$$bin" ] || bin=$(INSTALL_DIR)/$(SWITCHER); \
	if [ ! -x "$$bin" ]; then \
		echo "  Wayland interfaces: not checked (build or install the switcher first)"; \
	else \
		out=$$("$$bin" --check-compat 2>&1); rc=$$?; \
		if [ $$rc -eq 2 ]; then \
			echo "  Wayland interfaces: not checked ($$bin predates --check-compat, rebuild it)"; \
		else \
			echo "  Wayland interfaces:"; \
			echo "$$out" | sed 's/^/    /'; \
		fi; \
	fi
	@echo "Done."
