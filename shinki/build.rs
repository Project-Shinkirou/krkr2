use shinki_emu_build::{configure_cmake, write_core_build_manifest, BuildAsset, CoreBuildManifest};
use std::{env, fs, path::PathBuf};
mod static_link;
mod android_java;

fn build() -> Result<(), Box<dyn std::error::Error>> {
    let target = env::var("TARGET")?;
    let android = target.ends_with("-linux-android");
    let ios = target.contains("apple-ios");
    if !android && !ios {
        return Err(format!("KrKr2 Manager embedding requires Android or iOS, got {target}").into());
    }
    let adapter = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("missing manifest directory")?);
    let root = adapter.parent().ok_or("missing upstream source directory")?;
    let vcpkg = PathBuf::from(env::var_os("VCPKG_ROOT").ok_or("VCPKG_ROOT missing; run cargo xtask deps --core krkr2")?);
    let triplet = env::var("SHINKI_EMU_VCPKG_TRIPLET")?;
    let installed = PathBuf::from(env::var_os("SHINKI_EMU_NATIVE_VCPKG_ROOT").ok_or("native vcpkg root missing")?)
        .join("krkr2").join(format!("installed-{triplet}"));
    let toolchain = vcpkg.join("scripts/buildsystems/vcpkg.cmake");
    if !toolchain.is_file() || !installed.join(&triplet).join("lib").is_dir() {
        return Err("KrKr2 native dependencies missing; run cargo xtask deps --core krkr2 for this target".into());
    }
    let bison = PathBuf::from(env::var_os("BISON_EXECUTABLE").ok_or("BISON_EXECUTABLE missing")?);
    if !bison.is_file() { return Err("Bison executable missing".into()); }
    for name in ["VCPKG_ROOT", "SHINKI_EMU_VCPKG_TRIPLET", "SHINKI_EMU_NATIVE_VCPKG_ROOT", "BISON_EXECUTABLE", "ANDROID_NDK_HOME"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-changed={}", root.join("cpp").display());
    println!("cargo:rerun-if-changed={}", root.join("CMakeLists.txt").display());
    println!("cargo:rerun-if-changed=native");
    let mut definitions = vec![
        ("KRKR2_SHINKI_EMU".into(), "ON".into()),
        ("KRKR2_SHINKI_EMU_STATIC".into(), "ON".into()),
        ("ENABLE_TESTS".into(), "OFF".into()),
        ("BUILD_TOOLS".into(), "OFF".into()),
        ("CMAKE_TOOLCHAIN_FILE".into(), toolchain.to_string_lossy().replace('\\', "/")),
        ("VCPKG_INSTALLED_DIR".into(), installed.to_string_lossy().replace('\\', "/")),
        ("BISON_EXECUTABLE".into(), bison.to_string_lossy().replace('\\', "/")),
    ];
    if android {
        let abi = match env::var("CARGO_CFG_TARGET_ARCH")?.as_str() {
            "aarch64" => "arm64-v8a", "x86_64" => "x86_64",
            other => return Err(format!("unsupported Android architecture {other}").into()),
        };
        definitions.extend([("ANDROID".into(), "ON".into()), ("ANDROID_ABI".into(), abi.into()), ("ANDROID_PLATFORM".into(), "android-28".into())]);
        let ndk = PathBuf::from(env::var_os("ANDROID_NDK_HOME").ok_or("ANDROID_NDK_HOME missing")?);
        definitions.push(("VCPKG_CHAINLOAD_TOOLCHAIN_FILE".into(), ndk.join("build/cmake/android.toolchain.cmake").to_string_lossy().replace('\\', "/")));
    } else {
        definitions.extend([("APPLE".into(), "ON".into()), ("IOS".into(), "ON".into())]);
        let arch = if target.starts_with("aarch64-") { "arm64" } else { "x86_64" };
        let sdk = if target.ends_with("ios-sim") || target.starts_with("x86_64-") {
            "iphonesimulator"
        } else {
            "iphoneos"
        };
        definitions.extend([
            ("CMAKE_SYSTEM_NAME".into(), "iOS".into()),
            ("CMAKE_OSX_ARCHITECTURES".into(), arch.into()),
            ("CMAKE_OSX_SYSROOT".into(), sdk.into()),
            ("CMAKE_OSX_DEPLOYMENT_TARGET".into(), "16.0".into()),
        ]);
    }
    let query = PathBuf::from(env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?).join("native/.cmake/api/v1/query");
    fs::create_dir_all(&query)?;
    fs::write(query.join("codemodel-v2"), "")?;
    let native = configure_cmake(root, &definitions, &["shinki_emu_krkr2"])?;
    if android { android_java::stage(&adapter, &native)?; }
    let directory = native.join("shinki");
    let library = static_link::link(&native, &directory.join("libshinki_emu_krkr2.a"))?;
    let mut manifest = CoreBuildManifest::new("krkr2", "native-static")?;
    manifest.native_library = Some(library);
    manifest.resources.push(BuildAsset { source: directory.join("Resources"), dest: "Resources".into() });
    for name in ["LICENSE", "MODIFICATIONS.md"] {
        manifest.licenses.push(BuildAsset { source: root.join(name), dest: format!("licenses/krkr2/{name}") });
    }
    let share = installed.join(&triplet).join("share");
    for entry in fs::read_dir(share)? {
        let entry = entry?;
        let notice = entry.path().join("copyright");
        if notice.is_file() {
            manifest.licenses.push(BuildAsset { source: notice, dest: format!("licenses/krkr2/vcpkg/{}/copyright", entry.file_name().to_string_lossy()) });
        }
    }
    if android {
        manifest.licenses.push(BuildAsset { source: directory.join("Resources/android-host/licenses"), dest: "licenses/krkr2/android-java".into() });
    }
    write_core_build_manifest(&manifest)?;
    Ok(())
}

fn main() {
    if let Err(error) = build() { panic!("KrKr2 native core build failed: {error}"); }
}
