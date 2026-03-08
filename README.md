inside haskell

ghcup install ghc 9.2.8
ghcup set ghc 9.2.8

cabal clean
cabal build --allow-newer -j8 --ghc-options="-O0"


inside rust
# copy this file and put it in the rust folder it will be like find haskell_duckling_ffi/dist-newstyle -type f -name "libducklingffi.so"
find haskell_duckling_ffi/dist-newstyle -type f -name "libducklingffi.so"
cp libducklingffi.so lib_ext

LD_LIBRARY_PATH="$PWD/ext_lib:$LD_LIBRARY_PATH" cargo test -- --nocapture
cargo build --release