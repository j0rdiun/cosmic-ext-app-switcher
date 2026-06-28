BINARY      := cosmic-app-switcher
INSTALL_DIR := $(HOME)/.local/bin
TARGET      := target/release/$(BINARY)

.PHONY: all build install uninstall enable disable status reinstall check-compat

all: build

build:
	@command -v cargo >/dev/null 2>&1 || { \
		echo "Rust not found. Install with:"; \
		echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; \
		exit 1; \
	}
	cargo build --release

install: build
	@mkdir -p $(INSTALL_DIR)
	install -Dm755 $(TARGET) $(INSTALL_DIR)/$(BINARY)
	@$(MAKE) enable
	@echo ""
	@echo "Installed and enabled. Press Super+Tab or Alt+Tab to try it."

uninstall:
	@$(MAKE) disable
	@rm -f $(INSTALL_DIR)/$(BINARY)
	@echo "Uninstalled. COSMIC default switcher restored."

enable:
	@bash scripts/enable.sh

disable:
	@bash scripts/disable.sh

status:
	@bash scripts/status.sh

reinstall: uninstall install

# Check that the COSMIC environment is compatible with this tool
check-compat:
	@echo "Checking compatibility..."
	@bash scripts/find-config.sh > /dev/null 2>&1; case $$? in \
		0) echo "  COSMIC shortcuts config: found" ;; \
		2) echo "  COSMIC shortcuts config: not created yet (run 'make enable')" ;; \
		*) echo "  COSMIC shortcuts config: NOT found" ;; \
	esac
	@test -f $(INSTALL_DIR)/$(BINARY) && echo "  Binary: installed" || echo "  Binary: not installed"
	@command -v cosmic-comp >/dev/null 2>&1 && echo "  cosmic-comp: found" || echo "  cosmic-comp: not found (is COSMIC running?)"
	@echo "Done."
