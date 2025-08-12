CRATE          := ulcms
CRATE_MANIFEST := core/Cargo.toml
ARTIFACTS      := artifacts
DOCKER_IMAGE   := rust:1-bullseye

.PHONY: all macos-arm64 macos-x86_64 linux-amd64 linux-arm64 wasm rstage clean

all: macos-arm64 macos-x86_64 linux-amd64 linux-arm64 wasm rstage

macos-arm64:
	rustup target add aarch64-apple-darwin
	cargo build --manifest-path $(CRATE_MANIFEST) --release --target aarch64-apple-darwin
	mkdir -p $(ARTIFACTS)/macos-arm64
	cp core/target/aarch64-apple-darwin/release/lib$(CRATE).dylib $(ARTIFACTS)/macos-arm64/

macos-x86_64:
	rustup target add x86_64-apple-darwin
	cargo build --manifest-path $(CRATE_MANIFEST) --release --target x86_64-apple-darwin
	mkdir -p $(ARTIFACTS)/macos-x86_64
	cp core/target/x86_64-apple-darwin/release/lib$(CRATE).dylib $(ARTIFACTS)/macos-x86_64/

linux-amd64:
	docker run --rm --platform=linux/amd64 \
	  -e CARGO_TARGET_DIR=/work/core/target-linux-amd64 \
	  -v $$PWD:/work -w /work \
	  --entrypoint /usr/local/cargo/bin/cargo $(DOCKER_IMAGE) \
	  build --manifest-path $(CRATE_MANIFEST) --release
	mkdir -p $(ARTIFACTS)/linux-x86_64
	cp core/target-linux-amd64/release/lib$(CRATE).so $(ARTIFACTS)/linux-x86_64/

linux-arm64:
	docker run --rm --platform=linux/arm64 \
	  -e CARGO_TARGET_DIR=/work/core/target-linux-arm64 \
	  -v $$PWD:/work -w /work \
	  --entrypoint /usr/local/cargo/bin/cargo $(DOCKER_IMAGE) \
	  build --manifest-path $(CRATE_MANIFEST) --release
	mkdir -p $(ARTIFACTS)/linux-arm64
	cp core/target-linux-arm64/release/lib$(CRATE).so $(ARTIFACTS)/linux-arm64/

wasm:
	rustup target add wasm32-unknown-unknown
	cargo build --manifest-path $(CRATE_MANIFEST) --release --target wasm32-unknown-unknown
	mkdir -p $(ARTIFACTS)/wasm
	cp core/target/wasm32-unknown-unknown/release/$(CRATE).wasm $(ARTIFACTS)/wasm/
	cp core/target/wasm32-unknown-unknown/release/$(CRATE).wasm wrappers/js/src/ 2>/dev/null || true

rstage:
	mkdir -p wrappers/r/inst/libs
	@set -e; \
	for d in $(ARTIFACTS)/macos-* $(ARTIFACTS)/linux-* $(ARTIFACTS)/windows-* ; do \
	  [ -d "$$d" ] || continue; \
	  base=$$(basename "$$d"); \
	  mkdir -p "wrappers/r/inst/libs/$$base"; \
	  cp -f "$$d"/* "wrappers/r/inst/libs/$$base/"; \
	done

clean:
	cargo clean --manifest-path $(CRATE_MANIFEST)
	rm -rf $(ARTIFACTS) wrappers/r/inst/libs core/target-linux-amd64 core/target-linux-arm64
