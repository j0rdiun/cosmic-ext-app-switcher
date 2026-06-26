BINARY     := cosmic-app-switcher
INSTALL_DIR := $(HOME)/.local/bin
TARGET     := target/release/$(BINARY)

.PHONY: all build install uninstall enable disable status reinstall

all: build

build:
	@command -v cargo >/dev/null 2>&1 || { echo "Rust not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; exit 1; }
	cargo build --release

install: build
	@mkdir -p $(INSTALL_DIR)
	install -Dm755 $(TARGET) $(INSTALL_DIR)/$(BINARY)
	@$(MAKE) enable
	@echo "Installed and enabled. Press Super+Tab to test."

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
