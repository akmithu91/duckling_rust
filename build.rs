use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let ext_lib_path = Path::new(&manifest_dir).join("ext_lib");
    let target_so = ext_lib_path.join("libducklingffi.so");
    let haskell_dir = Path::new(&manifest_dir).join("haskell_duckling_ffi");

    // Ensure ext_lib exists
    if !ext_lib_path.exists() {
        fs::create_dir_all(&ext_lib_path).expect("Failed to create ext_lib directory");
    }

    // ── Step 1: Build the Haskell FFI library if the .so is missing ──
    if !target_so.exists() {
        println!("cargo:warning=libducklingffi.so not found — building from Haskell source...");

        // Ensure GHC 9.2.8 is installed and active
        run_cmd("ghcup", &["install", "ghc", "9.2.8"], None);
        run_cmd("ghcup", &["set", "ghc", "9.2.8"], None);

        // Ensure ghcup bin dir is on PATH so cabal/ghc are found.
        // GHCUP_INSTALL_BASE_PREFIX overrides the default $HOME location.
        let ghcup_base = env::var_os("GHCUP_INSTALL_BASE_PREFIX")
            .or_else(|| env::var_os("HOME"));
        if let Some(base) = ghcup_base {
            let ghcup_bin = Path::new(&base).join(".ghcup/bin");
            if ghcup_bin.exists() {
                let current_path = env::var("PATH").unwrap_or_default();
                env::set_var("PATH", format!("{}:{}", ghcup_bin.display(), current_path));
            }
        }

        // Clean and build
        run_cmd("cabal", &["clean"], Some(&haskell_dir));
        run_cmd(
            "cabal",
            &["build", "--allow-newer", "-j", "--ghc-options=-O0 +RTS -N -RTS"],
            Some(&haskell_dir),
        );

        // Find the built .so
        let find_output = Command::new("find")
            .args([
                haskell_dir.to_str().unwrap(),
                "-type",
                "f",
                "-name",
                "libducklingffi.so",
            ])
            .output()
            .expect("Failed to run find");

        let found_path = String::from_utf8_lossy(&find_output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        if found_path.is_empty() {
            panic!(
                "Haskell build succeeded but libducklingffi.so was not found under {}",
                haskell_dir.display()
            );
        }

        println!(
            "cargo:warning=Copying {} -> {}",
            found_path,
            target_so.display()
        );
        fs::copy(&found_path, &target_so).unwrap_or_else(|e| {
            panic!("Failed to copy {} to ext_lib: {}", found_path, e);
        });
    }

    // ── Step 2: Bundle runtime dependencies (Linux only) ──
    if cfg!(target_os = "linux") {
        println!("cargo:warning=Bundling Duckling runtime dependencies...");

        let output = Command::new("ldd")
            .arg(&target_so)
            .output()
            .expect("Failed to run ldd. Is it installed?");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("cargo:warning=ldd returned non-zero exit: {}", stderr);
        }

        let ldd_stdout = String::from_utf8_lossy(&output.stdout);

        // Skip core system libs to avoid portability issues
        let forbidden = [
            "libc.so",
            "ld-linux",
            "libdl.so",
            "libpthread.so",
            "libm.so",
            "librt.so",
            "libgcc_s.so",
            "libstdc++.so",
        ];

        for line in ldd_stdout.lines() {
            if !line.contains("=>") {
                continue;
            }

            let mut parts = line.splitn(2, "=>");
            let _name = parts.next().unwrap_or("").trim();
            let rhs = parts.next().unwrap_or("").trim();

            if rhs.contains("not found") {
                println!("cargo:warning=Missing dependency per ldd: {}", line);
                continue;
            }

            let path_str = match rhs.split_whitespace().next() {
                Some(p) if p.starts_with('/') => p,
                _ => continue,
            };

            let src_path = Path::new(path_str);
            let file_name = match src_path.file_name() {
                Some(f) => f,
                None => continue,
            };

            let name_str = file_name.to_string_lossy();
            if forbidden.iter().any(|f| name_str.contains(f)) {
                continue;
            }

            let dest_path = ext_lib_path.join(file_name);
            if !dest_path.exists() {
                println!(
                    "cargo:warning=Bundling dependency: {} -> {}",
                    src_path.display(),
                    dest_path.display()
                );
                fs::copy(src_path, &dest_path)
                    .unwrap_or_else(|e| panic!("Failed to copy {}: {}", src_path.display(), e));
            }
        }

        // Set RPATH=$ORIGIN on all .so files so they find each other
        for entry in fs::read_dir(&ext_lib_path).unwrap() {
            let path = entry.unwrap().path();
            let s = path.to_string_lossy();

            let is_so = path.extension().map_or(false, |ext| ext == "so");
            let is_versioned_so = s.contains(".so.");

            if is_so || is_versioned_so {
                let status = Command::new("patchelf")
                    .arg("--set-rpath")
                    .arg("$ORIGIN")
                    .arg(&path)
                    .status()
                    .expect("Failed to run patchelf. Install it: sudo apt-get install patchelf");

                if !status.success() {
                    println!("cargo:warning=patchelf failed on {}", path.display());
                }
            }
        }
    }

    // ── Step 3: Link the Haskell FFI library ──
    println!("cargo:rustc-link-search=native={}", ext_lib_path.display());
    println!("cargo:rustc-link-lib=dylib=ducklingffi");

    // Use absolute rpath so both this project's binaries AND consumer
    // project binaries can find the .so regardless of where they live.
    let ext_lib_abs = ext_lib_path
        .canonicalize()
        .unwrap_or_else(|_| ext_lib_path.clone());

    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", ext_lib_abs.display());

    // Propagate lib dir to consumer projects via Cargo metadata.
    // Consumers read DEP_DUCKLINGFFI_LIB_DIR in their own build.rs.
    println!("cargo:lib_dir={}", ext_lib_abs.display());

    println!("cargo:rerun-if-changed=ext_lib/libducklingffi.so");
    println!("cargo:rerun-if-changed=haskell_duckling_ffi/src/DucklingFFI.hs");
    println!("cargo:rerun-if-changed=haskell_duckling_ffi/csrc/wrapper.c");
    println!("cargo:rerun-if-changed=haskell_duckling_ffi/duckling-ffi.cabal");
}

/// Run a command, printing its stdout/stderr, and panic on failure.
fn run_cmd(program: &str, args: &[&str], cwd: Option<&Path>) {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    println!(
        "cargo:warning=Running: {} {}{}",
        program,
        args.join(" "),
        cwd.map_or(String::new(), |d| format!(" (in {})", d.display()))
    );

    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("Failed to run {} {}: {}", program, args.join(" "), e));

    if !status.success() {
        panic!(
            "Command `{} {}` failed with exit code {:?}",
            program,
            args.join(" "),
            status.code()
        );
    }
}
