//! Stage the Java consumers of the pinned static SDL/Cocos archives.
use flate2::read::GzDecoder;
use sha2::{Digest, Sha512};
use std::{env, error::Error, fs, io::Read, path::{Path, PathBuf}};
type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

fn replace(source: &mut String, before: &str, after: &str) -> Result<()> {
    if source.matches(before).count() != 1 { return Err(format!("pinned Java source drift: {before}").into()); }
    *source = source.replacen(before, after, 1);
    Ok(())
}
fn helper(source: &mut String) -> Result<()> {
    replace(source, "(Cocos2dxHelperListener)activity", "new org.shinkirou.krk2.KrkrHost()")?;
    replace(source, "((Cocos2dxActivity)sActivity).runOnGLThread(r);", "sCocos2dxHelperListener.runOnGLThread(r);")?;
    replace(source, "((Cocos2dxActivity)sActivity).setKeepScreenOn(value);", "sActivity.runOnUiThread(() -> { if (value) sActivity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON); else sActivity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON); });")?;
    *source = source.replace("Cocos2dxActivity.getContext()", "sActivity");
    replace(source, "Cocos2dxBitmap.setContext(activity);", "Cocos2dxBitmap.setContext(activity.getApplicationContext());")?;
    replace(source, "new Cocos2dxMusic(activity)", "new Cocos2dxMusic(activity.getApplicationContext())")?;
    replace(source, "nativeSetContext((Context)activity,", "nativeSetContext(activity.getApplicationContext(),")?;
    replace(source, "new Cocos2dxAccelerometer(sActivity)", "new Cocos2dxAccelerometer(sActivity.getApplicationContext())")?;
    replace(source, "new Cocos2dxSound(sActivity)", "new Cocos2dxSound(sActivity.getApplicationContext())")?;
    replace(source, "android.os.Process.killProcess(android.os.Process.myPid());", "org.shinkirou.krk2.KrkrHost.requestExit();")?;
    replace(source, "boolean suc = activity.getApplicationContext().bindService", "serviceBound = activity.getApplicationContext().bindService")?;
    replace(source, "private static boolean sInited = false;", r#"private static boolean sInited = false;
    private static boolean serviceBound;
    public static void releaseHost() {
        if (sCocos2dxAccelerometer != null) sCocos2dxAccelerometer.disable();
        if (sCocos2dMusic != null) sCocos2dMusic.end();
        if (sCocos2dSound != null) sCocos2dSound.end();
        if (serviceBound) sActivity.getApplicationContext().unbindService(connection);
        serviceBound = false;
        sCocos2dxAccelerometer = null;
        sCocos2dMusic = null;
        sCocos2dSound = null;
        sCocos2dxHelperListener = null;
        sActivity = null;
        sAssetManager = null;
        sVibrateService = null;
        sOBBFile = null;
        sAssetsPath = "";
        sInited = false;
    }
    public static void resetOwner() {
        sAccelerometerEnabled = false;
        sCompassEnabled = false;
        if (sCocos2dxAccelerometer != null) sCocos2dxAccelerometer.disable();
    }"#)?;
    if source.contains("Cocos2dxActivity") { return Err("unadapted Cocos Activity dependency".into()); }
    Ok(())
}

pub fn stage(adapter: &Path, native: &Path) -> Result<PathBuf> {
    let downloads = PathBuf::from(env::var_os("SHINKI_EMU_NATIVE_DOWNLOADS").ok_or("native downloads missing")?);
    let cache = downloads.parent().ok_or("native work root missing")?.join("vcpkg-downloads");
    let output = native.join("shinki/Resources/android-host");
    let sources = output.join("java");
    fs::create_dir_all(&sources)?;
    let mut contracts = Vec::new();
    let cocos_classes = ["Cocos2dxHelper.java", "Cocos2dxBitmap.java", "Cocos2dxTypefaces.java", "Cocos2dxMusic.java", "Cocos2dxSound.java", "Cocos2dxAccelerometer.java", "Cocos2dxRenderer.java", "Cocos2dxReflectionHelper.java"];
    for (file, hash, prefix, cocos) in [
        ("cocos2d-x-3.17.2.tar.gz", "b2d5ac968231892c39a953d82e9791c2182b0dbceca5791647bb2daad258134725386c9eb1d32de148465d88d2d932b29f241af0f5f4b4e6d9d80d9684f531fa", "cocos2d-x-cocos2d-x-3.17.2/cocos/platform/android/java/", true),
    ] {
        let archive = cache.join(file);
        println!("cargo:rerun-if-changed={}", archive.display());
        let bytes = fs::read(&archive)?;
        let digest = Sha512::digest(&bytes);
        let hex = b"0123456789abcdef";
        if !digest.iter().flat_map(|&byte| [hex[usize::from(byte >> 4)], hex[usize::from(byte & 15)]]).eq(hash.bytes()) {
            return Err(format!("pinned Java archive SHA512 mismatch: {file}").into());
        }
        let mut archive = tar::Archive::new(GzDecoder::new(bytes.as_slice()));
        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() { continue; }
            let path = entry.path()?.to_string_lossy().replace('\\', "/");
            let notice = path.strip_prefix("cocos2d-x-cocos2d-x-3.17.2/licenses/")
                .filter(|name| matches!(*name, "LICENSE_cocos2d-x.txt" | "LICENSE_cocosdenshion.txt" | "LICENSE_com.android.vending.expansion.zipfile.txt"));
            if let Some(name) = notice {
                fs::create_dir_all(output.join("licenses"))?;
                entry.unpack(output.join("licenses").join(name))?;
                continue;
            }
            let Some(relative) = path.strip_prefix(prefix) else { continue; };
            if cocos && relative == "libs/com.android.vending.expansion.zipfile.jar" {
                fs::create_dir_all(output.join("libs"))?;
                entry.unpack(output.join("libs/com.android.vending.expansion.zipfile.jar"))?;
                continue;
            }
            let relative = if cocos {
                let Some(relative) = relative.strip_prefix("src/") else { continue; };
                if !cocos_classes.iter().any(|name| relative == format!("org/cocos2dx/lib/{name}")) && relative != "com/enhance/gameservice/IGameTuningService.aidl" { continue; }
                relative
            } else { relative };
            if !relative.ends_with(".java") && !relative.ends_with(".aidl") { continue; }
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            if relative.ends_with("Cocos2dxHelper.java") { helper(&mut text)?; }
            if relative.ends_with("Cocos2dxAccelerometer.java") {
                replace(&mut text, "Cocos2dxGLSurfaceView.queueAccelerometer(x,y,z,sensorEvent.timestamp);", "final float ax = x, ay = y, az = z; final long timestamp = sensorEvent.timestamp; org.shinkirou.krk2.KrkrHost.queue(() -> onSensorChanged(ax, ay, az, timestamp));")?;
            }
            text = text.replace("com.enhance", "com.krkr2xx").replace("com/enhance", "com/krkr2xx").replace("org.cocos2dx", "org.kr2rc2dx").replace("org/cocos2dx", "org/kr2rc2dx").replace("org.libsdl.app", "org.krkrsd.app").replace("org/libsdl/app", "org/krkrsd/app");
            let destination = sources.join(relative.replace("com/enhance", "com/krkr2xx").replace("org/cocos2dx", "org/kr2rc2dx").replace("org/libsdl/app", "org/krkrsd/app"));
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::write(destination, text)?;
        }
    }
    for (source, destination) in [("android/KrkrHost.java", "org/shinkirou/krk2/KrkrHost.java"), ("android/KR2Activity.java", "org/tvp/krkr2core/KR2Activity.java")] {
        let destination = sources.join(destination);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::copy(adapter.join(source), destination)?;
    }
    for name in [
        "java/org/kr2rc2dx/lib/Cocos2dxHelper.java",
        "java/org/kr2rc2dx/lib/Cocos2dxBitmap.java",
        "java/com/krkr2xx/gameservice/IGameTuningService.aidl",
        "libs/com.android.vending.expansion.zipfile.jar",
        "licenses/LICENSE_cocos2d-x.txt",
        "licenses/LICENSE_cocosdenshion.txt",
        "licenses/LICENSE_com.android.vending.expansion.zipfile.txt",
    ] {
        if !output.join(name).is_file() { return Err(format!("required pinned Java input missing: {name}").into()); }
    }
    for (class, name, signature, native) in [
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "nativeSetContext", "(Landroid/content/Context;Landroid/content/res/AssetManager;)V", true),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "nativeSetAudioDeviceInfo", "(ZII)V", true),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "nativeSetEditTextDialogResult", "([B)V", true),
        ("org/kr2rc2dx/lib/Cocos2dxBitmap", "nativeInitBitmapDC", "(II[B)V", true),
        ("org/kr2rc2dx/lib/Cocos2dxAccelerometer", "onSensorChanged", "(FFFJ)V", true),
        ("org/shinkirou/krk2/KrkrHost", "nativeRequestExit", "()V", true),
        ("org/tvp/krkr2core/KR2Activity", "nativeCommitText", "(Ljava/lang/String;)V", true),
        ("org/shinkirou/krk2/KrkrHost", "initialize", "(Landroid/app/Activity;)V", false),
        ("org/shinkirou/krk2/KrkrHost", "ownerStarted", "()V", false),
        ("org/shinkirou/krk2/KrkrHost", "drain", "()V", false),
        ("org/shinkirou/krk2/KrkrHost", "ownerStopped", "()V", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "init", "(Landroid/app/Activity;)V", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "getAssetsPath", "()Ljava/lang/String;", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "getDPI", "()I", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "getCurrentLanguage", "()Ljava/lang/String;", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "getVersion", "()Ljava/lang/String;", false),
        ("org/kr2rc2dx/lib/Cocos2dxHelper", "openURL", "(Ljava/lang/String;)Z", false),
        ("org/kr2rc2dx/lib/Cocos2dxRenderer", "setAnimationInterval", "(F)V", false),
        ("org/kr2rc2dx/lib/Cocos2dxBitmap", "createTextBitmapShadowStroke", "([BLjava/lang/String;IIIIIIIIFZFFFFZIIIIFZI)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "updateMemoryInfo", "()V", false),
        ("org/tvp/krkr2core/KR2Activity", "getAvailMemory", "()J", false),
        ("org/tvp/krkr2core/KR2Activity", "getUsedMemory", "()J", false),
        ("org/tvp/krkr2core/KR2Activity", "getAndroidDeviceId", "()Ljava/lang/String;", false),
        ("org/tvp/krkr2core/KR2Activity", "GetInstance", "()Lorg/tvp/krkr2core/KR2Activity;", false),
        ("org/tvp/krkr2core/KR2Activity", "GetVersion", "()Ljava/lang/String;", false),
        ("org/tvp/krkr2core/KR2Activity", "getStoragePath", "()[Ljava/lang/String;", false),
        ("org/tvp/krkr2core/KR2Activity", "getLocaleName", "()Ljava/lang/String;", false),
        ("org/tvp/krkr2core/KR2Activity", "isWritableNormalOrSaf", "(Ljava/lang/String;)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "CreateFolders", "(Ljava/lang/String;)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "WriteFile", "(Ljava/lang/String;[B)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "DeleteFile", "(Ljava/lang/String;)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "RenameFile", "(Ljava/lang/String;Ljava/lang/String;)Z", false),
        ("org/tvp/krkr2core/KR2Activity", "showTextInput", "(IIII)V", false),
        ("org/tvp/krkr2core/KR2Activity", "hideTextInput", "()V", false),
    ] {
        contracts.push(serde_json::json!({
            "class": format!("L{class};"), "name": name, "signature": signature, "native": native,
        }));
    }
    fs::write(output.join("jni-contract.json"), serde_json::to_vec_pretty(&contracts)?)?;
    Ok(output)
}
