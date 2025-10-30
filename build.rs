use std::env;
use std::process::Command;

fn main() {
    let mut build = cc::Build::new();
    let mut using_zig = false;
    
    // Try to use zig cc if available
    // First check ZIG_PATH environment variable
    if let Ok(zig_path) = env::var("ZIG_PATH") {
        let zig_exe = if cfg!(target_os = "windows") {
            format!("{}/zig.exe", zig_path)
        } else {
            format!("{}/zig", zig_path)
        };
        if Command::new(&zig_exe).arg("version").output().is_ok() {
            println!("cargo:warning=Using Zig compiler from ZIG_PATH: {}", zig_exe);
            build.compiler(&zig_exe);
            build.flag("cc");
            build.flag("-target");
            build.flag("native-native");
            using_zig = true;
        }
    }
    
    if !using_zig {
        // Try to find zig in PATH
        let zig_cmd = if cfg!(target_os = "windows") { "zig.exe" } else { "zig" };
        if Command::new(zig_cmd).arg("version").output().is_ok() {
            println!("cargo:warning=Using Zig compiler from PATH");
            build.compiler(zig_cmd);
            build.flag("cc");
            build.flag("-target");
            build.flag("native-native");
            using_zig = true;
        }
    }
    
    if !using_zig {
        // Fall back to system C compiler (not mingw)
        println!("cargo:warning=Zig not found, using system C compiler (gcc/clang)");
    }
    
    build
        .file("src/phase2/c/opm.c")
        .include("src/phase2/c")
        .compile("opm");
    
    println!("cargo:rerun-if-changed=src/phase2/c/opm.c");
    println!("cargo:rerun-if-changed=src/phase2/c/opm.h");
}
