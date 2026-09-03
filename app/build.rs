use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=NOTEPAD_SCINTILLA_DIR");
    println!("cargo:rerun-if-env-changed=NOTEPAD_SCINTILLA_STATIC");

    let windows = env::var_os("CARGO_CFG_WINDOWS").is_some();
    if !windows {
        if let Ok(directory) = env::var("NOTEPAD_SCINTILLA_DIR") {
            let directory = PathBuf::from(directory);
            if directory.is_dir() {
                let mut files = Vec::new();
                for part in ["src", "lexlib", "lexers"] {
                    collect(&directory.join(part), &mut files);
                }
                if !files.is_empty() {
                    let mut build = cc::Build::new();
                    build
                        .cpp(true)
                        .warnings(false)
                        .flag_if_supported("-std=c++17")
                        .include(&directory)
                        .include(directory.join("include"))
                        .include(directory.join("src"))
                        .include(directory.join("lexlib"));
                    for file in files {
                        build.file(file);
                    }
                    build.compile("notepad_scintilla");
                }
            }
        }
    }

    if let Ok(library) = env::var("NOTEPAD_SCINTILLA_STATIC") {
        if !windows {
            if let Some(directory) = Path::new(&library).parent() {
                println!("cargo:rustc-link-search=native={}", directory.display());
            }
            if let Some(name) = Path::new(&library).file_stem().and_then(|name| name.to_str()) {
                println!(
                    "cargo:rustc-link-lib=static={}",
                    name.strip_prefix("lib").unwrap_or(name)
                );
            }
        }
    }

    if windows {
        for library in ["Imm32", "Ole32", "Uuid", "Gdi32", "User32", "Msimg32"] {
            println!("cargo:rustc-link-lib=dylib={library}");
        }
    }
}

fn collect(path: &Path, output: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, output);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "cxx" || extension == "cpp" || extension == "cc")
            {
                output.push(path);
            }
        }
    }
}
