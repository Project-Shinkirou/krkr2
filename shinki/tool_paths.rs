use std::{env, ffi::OsStr, path::{Path, PathBuf}};

pub fn resolve(name: &str, preferred: Option<&Path>, search_path: &OsStr) -> Option<PathBuf> {
    if let Some(directory) = preferred {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    env::split_paths(search_path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::atomic::{AtomicU64, Ordering}};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = env::temp_dir().join(format!("krkr2 LLVM path test {} {}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn directory(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            path
        }
        fn tool(&self, directory: &Path) -> PathBuf {
            let path = directory.join("llvm-nm");
            fs::write(&path, []).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
    }

    #[test]
    fn rust_sysroot_tool_wins_over_path_tool() {
        let fixture = Fixture::new();
        let preferred = fixture.directory("rust sysroot");
        let fallback = fixture.directory("PATH LLVM");
        let expected = fixture.tool(&preferred);
        fixture.tool(&fallback);
        let search = env::join_paths([fallback]).unwrap();
        assert_eq!(resolve("llvm-nm", Some(&preferred), &search), Some(expected));
    }

    #[test]
    fn missing_sysroot_tool_uses_first_available_path_tool() {
        let fixture = Fixture::new();
        let preferred = fixture.directory("incomplete Rust sysroot");
        let first = fixture.directory("first PATH LLVM");
        let second = fixture.directory("second PATH LLVM");
        let expected = fixture.tool(&first);
        fixture.tool(&second);
        let search = env::join_paths([first, second]).unwrap();
        assert_eq!(resolve("llvm-nm", Some(&preferred), &search), Some(expected));
    }

    #[test]
    fn unavailable_sysroot_uses_path_without_inventing_a_tool() {
        let fixture = Fixture::new();
        let fallback = fixture.directory("PATH only");
        let expected = fixture.tool(&fallback);
        let search = env::join_paths([fallback]).unwrap();
        assert_eq!(resolve("llvm-nm", None, &search), Some(expected));
        assert_eq!(resolve("llvm-objcopy", None, &search), None);
    }

    #[test]
    fn directory_named_like_tool_is_not_an_executable_candidate() {
        let fixture = Fixture::new();
        let preferred = fixture.directory("Rust sysroot");
        fs::create_dir(preferred.join("llvm-nm")).unwrap();
        let fallback = fixture.directory("PATH tools");
        let expected = fixture.tool(&fallback);
        let search = env::join_paths([fallback]).unwrap();
        assert_eq!(resolve("llvm-nm", Some(&preferred), &search), Some(expected));
    }
}
