SWITCHER    := cosmic-ext-app-switcher
APPLET      := cosmic-ext-applet-app-switcher
INSTALL_DIR := $(HOME)/.local/bin
APPS_DIR    := $(HOME)/.local/share/applications
SWITCHER_BIN := target/release/$(SWITCHER)
APPLET_BIN  := target/release/$(APPLET)
DESKTOP     := applet/data/io.github.cosmic-ext-applet-app-switcher.desktop

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
	@mkdir -p $(INSTALL_DIR) $(APPS_DIR)
	install -Dm755 $(SWITCHER_BIN) $(INSTALL_DIR)/$(SWITCHER)
	install -Dm755 $(APPLET_BIN) $(INSTALL_DIR)/$(APPLET)
	install -Dm644 $(DESKTOP) $(APPS_DIR)/io.github.cosmic-ext-applet-app-switcher.desktop
	@$(MAKE) enable
	@echo ""
	@echo "Installed and enabled. Press Super+Tab or Alt+Tab to try it."
	@echo "Add the 'App Switcher Settings' applet to your COSMIC panel to change themes."

uninstall:
	@$(MAKE) disable
	@rm -f $(INSTALL_DIR)/$(SWITCHER)
	@rm -f $(INSTALL_DIR)/$(APPLET)
	@rm -f $(APPS_DIR)/io.github.cosmic-ext-applet-app-switcher.desktop
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
	@echo "Done."
