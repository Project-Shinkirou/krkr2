//! Resolve CMake's real link plan, then namespace the entire native closure.
//! Undefined platform/C++ runtime imports stay untouched; every defined private
//! symbol (and its internal references) receives the same core-specific name.
use serde_json::Value;
use std::{collections::{BTreeMap, BTreeSet}, env, error::Error, fs, path::{Path, PathBuf}, process::Command};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;
const EXPORTS: [&str; 3] = ["shinki_emu_krkr2_query", "shinki_emu_krkr2_set_resource_root", "shinki_emu_krkr2_android_initialize"];
const APPLE_EXPORTS: [&str; 1] = ["_shinki_emu_krkr2_query"];

#[path = "tool_paths.rs"]
mod tool_paths;

pub struct Tools {
    nm: PathBuf,
    objcopy: PathBuf,
    ar: PathBuf,
    ld_lld: Option<PathBuf>,
}

fn required_tool(name: &str, preferred: Option<&Path>, search: &std::ffi::OsStr, allow_path: bool, hint: &str) -> Result<PathBuf> {
    let filename = format!("{name}{}", if cfg!(windows) { ".exe" } else { "" });
    let resolved = if allow_path {
        tool_paths::resolve(&filename, preferred, search)
    } else {
        preferred.map(|directory| directory.join(&filename)).filter(|path| path.is_file())
    };
    resolved.ok_or_else(|| {
        let attempted = preferred.map(|directory| directory.join(&filename)).unwrap_or_else(|| PathBuf::from(&filename));
        format!("required native tool {name} is missing; {hint}; {}",
            crate::command::describe(&Command::new(attempted))).into()
    })
}

pub fn prepare_tools(target: &str) -> Result<Tools> {
    let apple = target.contains("apple-ios");
    let search = env::var_os("PATH").unwrap_or_default();
    let (preferred, discovery) = if apple {
        let discovered = (|| -> Result<PathBuf> {
            let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
            let sysroot = capture(Command::new(rustc).args(["--print", "sysroot"]))?;
            if sysroot.trim().is_empty() { return Err("rustc --print sysroot returned an empty path".into()); }
            let host = env::var("HOST").map_err(|error| format!("reading Cargo HOST for Rust LLVM tools: {error}"))?;
            Ok(PathBuf::from(sysroot.trim()).join("lib/rustlib").join(host).join("bin"))
        })();
        match discovered {
            Ok(directory) => (Some(directory), String::new()),
            Err(error) => (None, format!("Rust sysroot discovery: {error}")),
        }
    } else {
        let ndk = PathBuf::from(env::var_os("ANDROID_NDK_HOME").ok_or("ANDROID_NDK_HOME missing")?);
        let host = if cfg!(windows) { "windows-x86_64" } else if cfg!(target_os = "macos") { "darwin-x86_64" } else { "linux-x86_64" };
        (Some(ndk.join("toolchains/llvm/prebuilt").join(host).join("bin")), String::new())
    };
    let hint = if apple {
        "install matching Rust LLVM tools with `rustup component add llvm-tools-preview`; LLVM tools on PATH may also be used"
    } else {
        "check ANDROID_NDK_HOME and the NDK LLVM prebuilt tool directory"
    };
    let resolve = |name| required_tool(name, preferred.as_deref(), &search, apple, hint)
        .map_err(|error| -> Box<dyn Error> { format!("{error}; {discovery}").into() });
    Ok(Tools {
        nm: resolve("llvm-nm")?,
        objcopy: resolve("llvm-objcopy")?,
        ar: resolve("llvm-ar")?,
        ld_lld: if apple { None } else { Some(resolve("ld.lld")?) },
    })
}

fn io_at<T>(operation: &str, path: &Path, result: std::io::Result<T>) -> Result<T> {
    result.map_err(|error| format!("{operation} {}: {error}", path.display()).into())
}

fn capture(command: &mut Command) -> Result<String> {
    let output = crate::command::output(command)?;
    if !output.status.success() {
        return Err(format!("{}; exited {}; stderr={}", crate::command::describe(command), output.status, String::from_utf8_lossy(&output.stderr)).into());
    }
    String::from_utf8(output.stdout).map_err(|error| format!("{}; exited {}; output was not UTF-8: {error}", crate::command::describe(command), output.status).into())
}
fn json(path: &Path) -> Result<Value> {
    let bytes = io_at("reading CMake JSON", path, fs::read(path))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parsing CMake JSON {}: {error}", path.display()).into())
}

#[path = "static_link_plan.rs"]
mod link_plan;
enum LinkInput { Archive(usize), Whole(bool) }
pub fn link(native: &Path, adapter: &Path, tools: &Tools) -> Result<PathBuf> {
    let target = env::var("TARGET")?;
    let apple = target.contains("apple-ios");
    let exports: &[&str] = if apple { &APPLE_EXPORTS } else { &EXPORTS };
    let namespace = if apple { "_shinki_krkr2_" } else { "shinki_krkr2_" };
    let reply = native.join(".cmake/api/v1/reply");
    let entries = io_at("reading CMake File API reply directory", &reply, fs::read_dir(&reply))?;
    let mut indices = io_at("enumerating CMake File API replies", &reply, entries.collect::<std::io::Result<Vec<_>>>())?;
    indices.retain(|entry| entry.file_name().to_string_lossy().starts_with("index-"));
    indices.sort_by_key(|entry| entry.file_name());
    let index = json(&indices.last().ok_or("CMake File API index missing")?.path())?;
    let model = json(&reply.join(index["reply"]["codemodel-v2"]["jsonFile"].as_str().ok_or("CMake codemodel missing")?))?;
    let targets = model["configurations"][0]["targets"].as_array().ok_or("CMake targets missing")?;
    let plan = targets.iter().find(|target| target["name"] == "shinki_krkr2_link_plan").ok_or("Krkr link plan missing")?;
    let plan = json(&reply.join(plan["jsonFile"].as_str().ok_or("link plan JSON missing")?))?;
    let generator = index["cmake"]["generator"]["name"].as_str().ok_or("CMake generator missing")?;
    let directory = link_plan::command_directory(native, plan["paths"]["build"].as_str().ok_or("link plan build directory missing")?, generator)?;
    let mut project_artifacts = BTreeSet::new();
    for target in targets {
        let target = json(&reply.join(target["jsonFile"].as_str().ok_or("CMake target file missing")?))?;
        if target["type"] != "STATIC_LIBRARY" { continue; }
        for artifact in target["artifacts"].as_array().ok_or("CMake archive artifacts missing")? {
            let path = native.join(artifact["path"].as_str().ok_or("CMake artifact path missing")?);
            if path.is_file() { project_artifacts.insert(io_at("canonicalizing CMake archive", &path, fs::canonicalize(&path))?); }
        }
    }
    let resolve = |token: &str| link_plan::resolve_archive(token, &directory, &project_artifacts);
    let mut tokens = Vec::new();
    for fragment in plan["link"]["commandFragments"].as_array().ok_or("CMake link fragments missing")? {
        let text = fragment["fragment"].as_str().ok_or("link fragment missing")?;
        #[cfg(windows)]
        let arguments = link_plan::windows_arguments(text)?;
        #[cfg(not(windows))]
        let arguments = shlex::split(text).ok_or("invalid POSIX CMake link fragment")?;
        for argument in arguments {
            if apple && argument == "-Xlinker" {
                continue;
            }
            if apple && argument.starts_with("-Wl,") {
                tokens.extend(argument[4..].split(',').map(str::to_owned));
            } else {
                tokens.push(argument);
            }
        }
    }
    let mut search = Vec::new();
    for token in &tokens {
        if let Some(path) = token.strip_prefix("-L") {
            let path = PathBuf::from(path);
            if !path.is_absolute() || !path.is_dir() { return Err(format!("invalid CMake library directory: {}", path.display()).into()); }
            if !search.contains(&path) { search.push(path); }
        }
    }
    for token in tokens.iter().filter(|token| token.ends_with(".a")) {
        let path = resolve(token)?;
        let parent = path.parent().ok_or("archive parent missing")?.to_owned();
        if !search.contains(&parent) { search.push(parent); }
    }
    let mut archives = vec![adapter.to_owned()];
    let mut archive_ids = BTreeMap::from([(io_at("canonicalizing adapter archive", adapter, fs::canonicalize(adapter))?, 0)]);
    let mut inputs = vec![LinkInput::Archive(0)];
    let mut system = BTreeSet::new();
    let mut frameworks = BTreeSet::new();
    let mut force_next = false;
    let mut apple_load_options = BTreeSet::new();
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        if apple && matches!(token.as_str(), "-ObjC" | "-all_load") {
            apple_load_options.insert(token);
            continue;
        }
        if apple && matches!(token.as_str(), "-framework" | "-weak_framework") {
            frameworks.insert(tokens.next().ok_or("Apple framework name missing")?);
            continue;
        }
        if apple && token == "-force_load" {
            force_next = true;
            continue;
        }
        if matches!(token.as_str(), "-Wl,--whole-archive" | "--whole-archive") {
            inputs.push(LinkInput::Whole(true));
            continue;
        }
        if matches!(token.as_str(), "-Wl,--no-whole-archive" | "--no-whole-archive") {
            inputs.push(LinkInput::Whole(false));
            continue;
        }
        let path = if token.ends_with(".a") {
            resolve(&token)?
        } else if let Some(name) = token.strip_prefix("-l") {
            if matches!(name, "android" | "log" | "dl" | "m" | "c" | "atomic" | "EGL" | "GLESv1_CM" | "GLESv2" | "OpenSLES" | "c++_shared" | "omp")
                || (apple && matches!(name, "c++" | "c++abi" | "System" | "objc" | "pthread" | "iconv" | "resolv")) {
                system.insert(name.to_owned());
                continue;
            }
            // pkg-config emits some transitive native libraries as -l names.
            // They belong inside the same private static closure, not in Rust's
            // process-global/system link set.
            search.iter().map(|dir| dir.join(format!("lib{name}.a"))).find(|path| path.is_file())
                .ok_or_else(|| format!("CMake native library has no static archive: {name}"))?
        } else if apple && token.ends_with(".framework") {
            frameworks.insert(Path::new(&token).file_stem().and_then(|s| s.to_str()).ok_or("framework name")?.to_owned());
            continue;
        } else if apple && (token.ends_with(".tbd") || token.ends_with(".dylib")) {
            if !token.contains(".sdk/usr/lib/") {
                return Err(format!("non-static Krkr Apple dependency: {token}").into());
            }
            let name = Path::new(&token).file_name().and_then(|s| s.to_str()).ok_or("SDK library name")?;
            system.insert(name.strip_prefix("lib").ok_or("SDK library prefix")?.split('.').next().ok_or("SDK library stem")?.to_owned());
            continue;
        } else if token.ends_with(".so") {
            let name = Path::new(&token).file_name().and_then(|s| s.to_str()).ok_or("runtime filename")?;
            if !matches!(name, "libc++_shared.so" | "libomp.so") { return Err(format!("non-static Krkr dependency: {token}").into()); }
            println!("cargo:rustc-link-search=native={}", Path::new(&token).parent().unwrap().display());
            system.insert(name.trim_start_matches("lib").trim_end_matches(".so").to_owned());
            continue;
        } else { continue; };
        let identity = io_at("canonicalizing native dependency archive", &path, fs::canonicalize(&path))?;
        let index = if let Some(&index) = archive_ids.get(&identity) {
            index
        } else {
            let index = archives.len();
            archives.push(path);
            archive_ids.insert(identity, index);
            index
        };
        if force_next { inputs.push(LinkInput::Whole(true)); }
        inputs.push(LinkInput::Archive(index));
        if force_next {
            inputs.push(LinkInput::Whole(false));
            force_next = false;
        }
    }
    let object = native.join("shinki/krkr-closure.o");
    let response = native.join("shinki/krkr-closure.rsp");
    let mut args = vec!["-r".to_owned()];
    if apple {
        let simulator = target == "x86_64-apple-ios" || target.ends_with("ios-sim");
        let sdk = if simulator { "iphonesimulator" } else { "iphoneos" };
        let sdk_path = capture(Command::new("xcrun").args(["--sdk", sdk, "--show-sdk-path"]))?;
        let sdk_version = capture(Command::new("xcrun").args(["--sdk", sdk, "--show-sdk-version"]))?;
        let architecture = if target.starts_with("aarch64-") { "arm64" } else { "x86_64" };
        args.extend(["-arch".into(), architecture.into(), "-platform_version".into(),
            if simulator { "ios-simulator".into() } else { "ios".into() },
            "16.0".into(), sdk_version.trim().into(), "-syslibroot".into(), format!("\"{}\"", sdk_path.trim())]);
        args.extend(apple_load_options);
    } else {
        args.push("--start-group".into());
    }
    let mut whole = false;
    for input in inputs {
        match input {
            LinkInput::Archive(index) => {
                if apple && whole { args.push("-force_load".into()); }
                args.push(format!("\"{}\"", archives[index].to_string_lossy().replace('\\', "/")));
            }
            LinkInput::Whole(value) => {
                whole = value;
                if !apple { args.push(if value { "--whole-archive" } else { "--no-whole-archive" }.into()); }
            }
        }
    }
    if !apple { args.push("--end-group".into()); }
    for export in exports {
        if apple { args.extend(["-u".into(), (*export).into()]); }
        else { args.push(format!("--undefined={export}")); }
    }
    args.extend(["-o".into(), format!("\"{}\"", object.to_string_lossy().replace('\\', "/"))]);
    io_at("writing native linker response", &response, fs::write(&response, args.join("\n")))?;
    if apple {
        capture(Command::new("xcrun").arg("ld").arg(format!("@{}", response.display())))?;
    } else {
        let linker = tools.ld_lld.as_ref().ok_or("Android LLVM linker was not prepared")?;
        capture(Command::new(linker).arg(format!("@{}", response.display())))?;
    }
    if !apple {
    // Keep private Java class names and native RegisterNatives tables aligned.
    // Equal-width replacements preserve ELF section offsets and relocations.
    let mut bytes = io_at("reading native closure for JNI isolation", &object, fs::read(&object))?;
    for (before, after) in [
        (b"com/enhance".as_slice(), b"com/krkr2xx".as_slice()),
        (b"com.enhance".as_slice(), b"com.krkr2xx".as_slice()),
        (b"org/tvp/kirikiri2".as_slice(), b"org/tvp/krkr2core".as_slice()),
        (b"org.tvp.kirikiri2".as_slice(), b"org.tvp.krkr2core".as_slice()),
        (b"org/cocos2dx".as_slice(), b"org/kr2rc2dx".as_slice()),
        (b"org.cocos2dx".as_slice(), b"org.kr2rc2dx".as_slice()),
    ] {
        assert_eq!(before.len(), after.len());
        let mut offset = 0;
        while let Some(index) = bytes[offset..].windows(before.len()).position(|window| window == before) {
            let start = offset + index;
            bytes[start..start + before.len()].copy_from_slice(after);
            offset = start + before.len();
        }
    }
    io_at("writing JNI-isolated native closure", &object, fs::write(&object, bytes))?;
    }
    let symbols = capture(Command::new(&tools.nm).args(["--defined-only", "--extern-only", "--format=posix"]).arg(&object))?;
    let mut renames = BTreeSet::new();
    for line in symbols.lines() {
        let symbol = line.split_whitespace().next().ok_or("invalid nm symbol")?;
        if !exports.contains(&symbol) {
            let private = if apple { symbol.strip_prefix('_').unwrap_or(symbol) } else { symbol };
            renames.insert(format!("{symbol} {namespace}{private}"));
        }
    }
    let map = native.join("shinki/krkr-symbols.map");
    io_at("writing native symbol rename map", &map, fs::write(&map, renames.into_iter().collect::<Vec<_>>().join("\n") + "\n"))?;
    let isolated = native.join("shinki/krkr-isolated.o");
    capture(Command::new(&tools.objcopy).arg(format!("--redefine-syms={}", map.display())).arg(&object).arg(&isolated))?;
    let isolated_symbols = capture(Command::new(&tools.nm).args(["--defined-only", "--extern-only", "--format=posix"]).arg(&isolated))?;
    let names: BTreeSet<_> = isolated_symbols.lines().filter_map(|line| line.split_whitespace().next()).collect();
    if exports.iter().any(|export| !names.contains(export))
        || names.iter().any(|name| !exports.contains(name) && !name.starts_with(namespace)) {
        return Err("Krkr static closure leaked an unisolated symbol or lost its query/resource entry".into());
    }
    let archive = native.join("shinki/libshinki_krkr2_closure.a");
    if archive.exists() { io_at("removing previous isolated archive", &archive, fs::remove_file(&archive))?; }
    capture(Command::new(&tools.ar).arg("crs").arg(&archive).arg(&isolated))?;
    let inputs_path = native.join("shinki/krkr-static-inputs.json");
    io_at("writing native closure input receipt", &inputs_path, fs::write(&inputs_path, serde_json::to_vec_pretty(&archives)?))?;
    println!("cargo:rustc-link-search=native={}", archive.parent().unwrap().display());
    println!("cargo:rustc-link-lib=static=shinki_krkr2_closure");
    if apple {
        system.extend(["c++", "objc", "System"].map(str::to_owned));
        frameworks.insert("Foundation".into());
    } else {
        system.extend(["c++_shared", "android", "log", "dl", "m", "EGL", "GLESv2", "GLESv1_CM", "OpenSLES"].map(str::to_owned));
    }
    for name in system { println!("cargo:rustc-link-lib={name}"); }
    for name in frameworks { println!("cargo:rustc-link-lib=framework={name}"); }
    Ok(archive)
}
