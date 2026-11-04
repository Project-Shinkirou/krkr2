//! Host command syntax and structured CMake archive resolution.
use std::{collections::BTreeSet, error::Error, fs, path::{Path, PathBuf}};
type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

pub fn command_directory(root: &Path, target_build: &str, generator: &str) -> Result<PathBuf> {
    match generator {
        "Ninja" | "Ninja Multi-Config" => Ok(root.to_owned()),
        "Unix Makefiles" | "NMake Makefiles" | "NMake Makefiles JOM" | "MinGW Makefiles" | "MSYS Makefiles" => Ok(root.join(target_build)),
        other => Err(format!("unsupported CMake link-command generator: {other}").into()),
    }
}

pub fn resolve_archive(token: &str, directory: &Path, project_artifacts: &BTreeSet<PathBuf>) -> Result<PathBuf> {
    let input = PathBuf::from(token);
    let path = if input.is_absolute() { input.clone() } else { directory.join(&input) };
    if !path.is_file() { return Err(format!("static dependency missing: {}", path.display()).into()); }
    if !input.is_absolute() && !project_artifacts.contains(&fs::canonicalize(&path)?) {
        return Err(format!("relative archive is absent from CMake target artifacts: {token}").into());
    }
    Ok(path)
}

#[cfg(windows)]
pub fn windows_arguments(fragment: &str) -> Result<Vec<String>> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    // A real argv[0] keeps CommandLineToArgvW's special executable-name rules
    // away from the fragment's first ordinary argument.
    let command = format!("cmake-link.exe {fragment}");
    let wide: Vec<u16> = std::ffi::OsStr::new(&command).encode_wide().chain(Some(0)).collect();
    let mut count = 0;
    let arguments = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if arguments.is_null() { return Err(std::io::Error::last_os_error().into()); }
    let result = (1..count).map(|index| {
        let start = unsafe { *arguments.add(index as usize) };
        let mut length = 0;
        while unsafe { *start.add(length) } != 0 { length += 1; }
        String::from_utf16(unsafe { std::slice::from_raw_parts(start, length) })
    }).collect::<std::result::Result<Vec<_>, _>>();
    unsafe { LocalFree(arguments.cast()); }
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Command, sync::atomic::{AtomicU64, Ordering}};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct NativeConsumer {
        root: PathBuf,
        tools: PathBuf,
        archive: PathBuf,
        caller: PathBuf,
    }
    impl Drop for NativeConsumer {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
    }
    fn run(command: &mut Command) -> String {
        let output = command.output().unwrap();
        assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }
    impl NativeConsumer {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("krkr link consumer {} {}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&root).unwrap();
            let ndk = PathBuf::from(std::env::var_os("ANDROID_NDK_HOME").expect("real native consumer tests require ANDROID_NDK_HOME"));
            let host = if cfg!(windows) { "windows-x86_64" } else if cfg!(target_os = "macos") { "darwin-x86_64" } else { "linux-x86_64" };
            let tools = ndk.join("toolchains/llvm/prebuilt").join(host).join("bin");
            let archive = root.join("cpp/plugins/real native archive.a");
            fs::create_dir_all(archive.parent().unwrap()).unwrap();
            fs::create_dir(root.join("shinki")).unwrap();
            let source = root.join("native.c");
            let caller_source = root.join("caller.c");
            let object = root.join("native.o");
            let caller = root.join("caller.o");
            fs::write(&source, "int krkr_path_add(int a, int b) { return a + b; }\n").unwrap();
            fs::write(&caller_source, "extern int krkr_path_add(int, int); int krkr_path_consumer(int x) { return krkr_path_add(x, 7); }\n").unwrap();
            let consumer = Self { root, tools, archive, caller };
            for (input, output) in [(&source, &object), (&caller_source, &consumer.caller)] {
                run(Command::new(consumer.tool("clang")).args(["--target=aarch64-linux-android26", "-fPIC", "-c"]).arg(input).arg("-o").arg(output));
            }
            run(Command::new(consumer.tool("llvm-ar")).arg("crs").arg(&consumer.archive).arg(&object));
            consumer
        }
        fn tool(&self, name: &str) -> PathBuf {
            self.tools.join(format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }))
        }
        fn artifacts(&self) -> BTreeSet<PathBuf> {
            BTreeSet::from([fs::canonicalize(&self.archive).unwrap()])
        }
        fn link(&self, archive: &Path) {
            let linked = self.root.join("resolved consumer.o");
            run(Command::new(self.tool("ld.lld")).arg("-r").arg(&self.caller).arg(archive).arg("-o").arg(&linked));
            let undefined = run(Command::new(self.tool("llvm-nm")).args(["--undefined-only", "--extern-only", "--format=posix"]).arg(&linked));
            assert!(undefined.trim().is_empty(), "real consumer did not resolve its archive: {undefined}");
            let defined = run(Command::new(self.tool("llvm-nm")).args(["--defined-only", "--extern-only", "--format=posix"]).arg(&linked));
            assert!(defined.lines().any(|line| line.starts_with("krkr_path_add T ")));
            assert!(defined.lines().any(|line| line.starts_with("krkr_path_consumer T ")));
        }
    }

    #[test]
    fn makefile_relative_archive_links_from_its_target_directory() {
        let native = NativeConsumer::new();
        let directory = command_directory(&native.root, "shinki", "Unix Makefiles").unwrap();
        let resolved = resolve_archive("../cpp/plugins/real native archive.a", &directory, &native.artifacts()).unwrap();
        native.link(&resolved);
    }

    #[test]
    fn ninja_relative_archive_links_from_the_top_build_directory() {
        let native = NativeConsumer::new();
        let directory = command_directory(&native.root, "shinki", "Ninja").unwrap();
        let resolved = resolve_archive("cpp/plugins/real native archive.a", &directory, &native.artifacts()).unwrap();
        native.link(&resolved);
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_backslashes_and_spaces_reach_the_real_linker() {
        let native = NativeConsumer::new();
        let path = native.archive.to_string_lossy().replace('/', "\\");
        assert!(path.as_bytes().get(1) == Some(&b':') && path.contains(' ') && path.contains('\\'));
        let arguments = windows_arguments(&format!("-Wl,--whole-archive \"{path}\" -Wl,--no-whole-archive")).unwrap();
        assert_eq!(arguments.len(), 3);
        let directory = command_directory(&native.root, "shinki", "Ninja").unwrap();
        let resolved = resolve_archive(&arguments[1], &directory, &native.artifacts()).unwrap();
        native.link(&resolved);
    }

    #[test]
    fn missing_archive_reports_the_actual_command_relative_path() {
        let native = NativeConsumer::new();
        let directory = command_directory(&native.root, "shinki", "Unix Makefiles").unwrap();
        let error = resolve_archive("../cpp/plugins/missing native archive.a", &directory, &native.artifacts()).unwrap_err().to_string();
        assert!(error.contains("static dependency missing:"));
        assert!(error.contains("missing native archive.a"));
    }

    #[test]
    fn undeclared_relative_archive_cannot_enter_the_native_closure() {
        let native = NativeConsumer::new();
        let directory = command_directory(&native.root, "shinki", "Ninja").unwrap();
        let error = resolve_archive("cpp/plugins/real native archive.a", &directory, &BTreeSet::new()).unwrap_err().to_string();
        assert!(error.contains("absent from CMake target artifacts"));
    }
}
