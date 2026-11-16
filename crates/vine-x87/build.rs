use std::{env, fs, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    let output = command
        .output()
        .expect("cannot launch x87 C toolchain (set VINE_X87_CC/VINE_X87_AR)");
    assert!(
        output.status.success(),
        "x87 C toolchain failed: {}",
        String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(16_384)])
    );
}

fn main() {
    for name in ["VINE_X87_CC", "VINE_X87_AR"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=src/softfloat_bridge.c");
    println!("cargo:rerun-if-changed=src/logarithm.c");
    println!("cargo:rerun-if-changed=src/arctangent.c");
    println!("cargo:rerun-if-changed=src/cosine.c");
    println!("cargo:rerun-if-changed=src/exponential.c");
    println!("cargo:rerun-if-changed=src/native_oracle.c");
    let target = env::var("TARGET").unwrap();
    assert!(
        target.starts_with("wasm32-") || target.starts_with("x86_64-"),
        "unsupported vine-x87 target: {target}"
    );
    let c_target = if target == "wasm32-unknown-unknown" {
        "wasm32-unknown-unknown"
    } else {
        &target
    };
    let cc = env::var_os("VINE_X87_CC").unwrap_or_else(|| {
        // Some Windows MinGW distributions omit the Wasm target. Prefer the
        // standard LLVM installation when building browser arithmetic.
        let installed = env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .map(|base| base.join("LLVM/bin/clang.exe"));
        if target.starts_with("wasm32-")
            && let Some(path) = installed.filter(|path| path.is_file())
        {
            path.into_os_string()
        } else {
            "clang".into()
        }
    });
    let ar = env::var_os("VINE_X87_AR").unwrap_or_else(|| "llvm-ar".into());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut sources: Vec<_> = fs::read_to_string("vendor/sources.txt")
        .unwrap()
        .lines()
        .map(|line| PathBuf::from("vendor").join(line))
        .collect();
    sources.push("src/softfloat_bridge.c".into());
    sources.push("src/logarithm.c".into());
    sources.push("src/arctangent.c".into());
    sources.push("src/cosine.c".into());
    sources.push("src/exponential.c".into());
    if target.starts_with("x86_64-") {
        sources.push("src/native_oracle.c".into());
    }
    let mut objects = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let object = out.join(format!("x87_{index}.o"));
        run(Command::new(&cc)
            .arg(format!("--target={c_target}"))
            .args([
                "-std=c11",
                "-O2",
                "-ffreestanding",
                "-fno-strict-aliasing",
                "-fno-builtin",
                "-DSOFTFLOAT_FAST_INT64",
                "-DINLINE_LEVEL=5",
                "-Ivendor",
                "-Ivendor/include",
                "-Ivendor/8086",
                "-c",
            ])
            .arg(source)
            .arg("-o")
            .arg(&object));
        objects.push(object);
    }
    let archive = out.join("libvine_x87_softfloat.a");
    if archive.exists() {
        fs::remove_file(&archive).expect("cannot replace x87 archive in Cargo OUT_DIR");
    }
    run(Command::new(ar).arg("crs").arg(&archive).args(objects));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=vine_x87_softfloat");
}
