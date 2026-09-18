SWITCHER    := cosmic-ext-app-switcher
APPLET      := cosmic-ext-applet-app-switcher
INSTALL_DIR := $(HOME)/.local/bin
APPS_DIR    := $(HOME)/.local/share/applications
SWITCHER_BIN := target/release/$(SWITCHER)
APPLET_BIN  := target/release/$(APPLET)
DESKTOP     := applet/data/io.github.cosmic-ext-applet-app-switcher.desktop
ICON_SRC    := applet/data/io.github.cosmic-ext-applet-app-switcher-symbolic.svg
ICONS_DIR   := $(HOME)/.local/share/icons/hicolor/scalable/apps

.PHONY: all build install uninstall enable disable status reinstall check-compat

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
	@pkill -f '^$(INSTALL_DIR)/$(SWITCHER)( |$$)' 2>/dev/null || true
	@$(MAKE) enable
	@echo ""
	@echo "Installed and enabled. Press Super+Tab or Alt+Tab to try it."
	@echo "Add the 'App Switcher Settings' applet to your COSMIC panel to change themes."

uninstall:
	@$(MAKE) disable
	@pkill -f '^$(INSTALL_DIR)/$(SWITCHER)( |$$)' 2>/dev/null || true
	@rm -f $(INSTALL_DIR)/$(SWITCHER)
	@rm -f $(INSTALL_DIR)/$(APPLET)
	@rm -f $(APPS_DIR)/io.github.cosmic-ext-applet-app-switcher.desktop
	@rm -f $(ICONS_DIR)/io.github.cosmic-ext-applet-app-switcher-symbolic.svg
	@echo "Uninstalled. COSMIC default switcher restored."

enable:
	@bash scripts/enable.sh

disable:
	@bash scripts/disable.sh

status:
	@bash scripts/status.sh

reinstall: uninstall install

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
