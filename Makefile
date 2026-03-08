.PHONY: build release test clean clean-haskell clean-rust clean-all docker

build:
	cargo build

release:
	cargo build --release

test:
	LD_LIBRARY_PATH="$$PWD/ext_lib:$$LD_LIBRARY_PATH" cargo test -- --nocapture

clean-rust:
	cargo clean

clean-haskell:
	cd haskell_duckling_ffi && cabal clean
	rm -f ext_lib/*.so*

clean: clean-rust clean-haskell

# Full rebuild: wipe everything then build from scratch
rebuild: clean build

docker:
	docker build -t duckling_rust .
