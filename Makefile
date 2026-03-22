CARGO := cargo
RUSTFLAGS_STATIC := -C target-feature=+crt-static
TARGET_DIR := target
DIST_DIR := dist
VERSION := $(shell grep '^version' crates/harpoon-app/Cargo.toml | head -1 | cut -d'"' -f2)
ARCH := $(shell uname -m)
OS := $(shell uname -s | tr '[:upper:]' '[:lower:]')
TRIPLE := $(ARCH)-unknown-$(OS)-musl

.PHONY: all clean build-web minimal standard full dist install help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Build Web UI ──────────────────────────────────────────────

build-web: ## Build Svelte web UI
	@echo "Building web UI..."
	@if [ -f scripts/build-web-local.sh ]; then \
		./scripts/build-web-local.sh; \
	else \
		echo "Error: build-web-local.sh not found"; exit 1; \
	fi

build-web-docker: ## Build Svelte web UI via Docker
	@echo "Building web UI (Docker)..."
	./scripts/build-web.sh

# ── Rust Builds ───────────────────────────────────────────────

minimal: ## Build minimal binary (no features, dynamic)
	@echo "Building harpoon-minimal..."
	$(CARGO) build --release --no-default-features
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/release/harpoon $(DIST_DIR)/harpoon-minimal
	@echo "Built: $(DIST_DIR)/harpoon-minimal"

standard: build-web ## Build standard binary (web UI)
	@echo "Building harpoon..."
	$(CARGO) build --release --features web
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/release/harpoon $(DIST_DIR)/harpoon
	@echo "Built: $(DIST_DIR)/harpoon"

full: build-web ## Build full-featured binary (web + tls + regex + transparent-udp)
	@echo "Building harpoon-full..."
	$(CARGO) build --release --features "web,tls,regex-filter,transparent-udp,http2"
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/release/harpoon $(DIST_DIR)/harpoon-full
	@echo "Built: $(DIST_DIR)/harpoon-full"

all: minimal standard full ## Build all variants

# ── Static Builds (musl) ─────────────────────────────────────

static-minimal: ## Build static minimal binary (musl)
	@echo "Building static harpoon-minimal..."
	RUSTFLAGS="$(RUSTFLAGS_STATIC)" $(CARGO) build --release --target $(TRIPLE) --no-default-features
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/$(TRIPLE)/release/harpoon $(DIST_DIR)/harpoon-minimal-static
	@echo "Built: $(DIST_DIR)/harpoon-minimal-static"

static-standard: build-web ## Build static standard binary (musl, web UI)
	@echo "Building static harpoon..."
	RUSTFLAGS="$(RUSTFLAGS_STATIC)" $(CARGO) build --release --target $(TRIPLE) --features web
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/$(TRIPLE)/release/harpoon $(DIST_DIR)/harpoon-static
	@echo "Built: $(DIST_DIR)/harpoon-static"

static-full: build-web ## Build static full-featured binary (musl, all features)
	@echo "Building static harpoon-full..."
	RUSTFLAGS="$(RUSTFLAGS_STATIC)" $(CARGO) build --release --target $(TRIPLE) --features "web,tls,regex-filter,transparent-udp"
	@mkdir -p $(DIST_DIR)
	cp $(TARGET_DIR)/$(TRIPLE)/release/harpoon $(DIST_DIR)/harpoon-full-static
	@echo "Built: $(DIST_DIR)/harpoon-full-static"

static-all: static-minimal static-standard static-full ## Build all static variants

# ── Distribution Package ──────────────────────────────────────

dist: all ## Build all variants and create release archive
	@echo "Creating distribution package..."
	@mkdir -p $(DIST_DIR)
	cp docs/example-config.toml $(DIST_DIR)/config.toml.example
	cp README.md $(DIST_DIR)/
	cp -r docs $(DIST_DIR)/docs
	cd $(DIST_DIR) && tar czf harpoon-$(VERSION)-$(OS)-$(ARCH).tar.gz \
		harpoon-minimal harpoon harpoon-full \
		config.toml.example README.md docs/
	@echo ""
	@echo "Distribution: $(DIST_DIR)/harpoon-$(VERSION)-$(OS)-$(ARCH).tar.gz"
	@ls -lh $(DIST_DIR)/harpoon-$(VERSION)-$(OS)-$(ARCH).tar.gz

# ── Install ───────────────────────────────────────────────────

install: standard ## Install harpoon to /usr/local/bin
	@echo "Installing harpoon to /usr/local/bin..."
	install -m 755 $(DIST_DIR)/harpoon /usr/local/bin/harpoon
	@echo "Installed: /usr/local/bin/harpoon"

install-full: full ## Install harpoon-full to /usr/local/bin
	@echo "Installing harpoon-full to /usr/local/bin..."
	install -m 755 $(DIST_DIR)/harpoon-full /usr/local/bin/harpoon
	@echo "Installed: /usr/local/bin/harpoon"

# ── Development ───────────────────────────────────────────────

dev: ## Build debug with web UI (fast iteration)
	$(CARGO) build --features web

dev-run: dev ## Build and run with example config
	$(CARGO) run --features web -- run -c docs/example-config.toml

test: ## Run all tests
	$(CARGO) test

check: ## Cargo check all features
	$(CARGO) check --features "web,tls,regex-filter,transparent-udp,http2"

clippy: ## Run clippy on all features
	$(CARGO) clippy --features "web,tls,regex-filter,transparent-udp,http2" -- -D warnings

# ── Cleanup ───────────────────────────────────────────────────

clean: ## Clean build artifacts
	$(CARGO) clean
	rm -rf $(DIST_DIR)
