.PHONY: build release test clean clean-haskell clean-rust haskell publish docker

# Build the Haskell FFI shared library and bundle its runtime deps into ext_lib/
haskell:
	cd haskell_duckling_ffi && cabal build --allow-newer -j --ghc-options="-O0 -j +RTS -N -RTS"
	mkdir -p ext_lib
	cp $$(find haskell_duckling_ffi -type f -name libducklingffi.so | head -1) ext_lib/libducklingffi.so
	# Bundle runtime dependencies
	for lib in $$(ldd ext_lib/libducklingffi.so | grep '=>' | grep -v 'not found' \
		| awk '{print $$3}' | grep '^/' \
		| grep -v -e 'libc\.so' -e 'ld-linux' -e 'libdl\.so' -e 'libpthread\.so' \
		       -e 'libm\.so' -e 'librt\.so' -e 'libgcc_s\.so' -e 'libstdc++\.so'); do \
		cp -n "$$lib" ext_lib/ 2>/dev/null || true; \
	done
	# Set RPATH so bundled .so files find each other
	for so in ext_lib/*.so ext_lib/*.so.*; do \
		[ -f "$$so" ] && patchelf --set-rpath '$$ORIGIN' "$$so" 2>/dev/null || true; \
	done

build: haskell
	cargo build

release: haskell
	cargo build --release

test:
	LD_LIBRARY_PATH="$$PWD/ext_lib:$$LD_LIBRARY_PATH" cargo test -- --nocapture

clean-rust:
	cargo clean

clean-haskell:
	cd haskell_duckling_ffi && cabal clean
	rm -f ext_lib/*.so*

clean: clean-rust clean-haskell

rebuild: clean build

docker:
	docker build -t duckling_rust .
