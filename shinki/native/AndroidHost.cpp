#include "AndroidHost.h"
#include "include/shinki_emu.h"
#include "platform/android/jni/JniHelper.h"
#include "environ/EmbeddedSession.h"
#include "Platform.h"
#include "environ/cocos2d/MainScene.h"
#include <jni.h>
#include <stdexcept>
#include <string>
#include <atomic>
#include <mutex>

extern "C" {
// This name is applied to SDL's original loader before closure resolution.
// Cocos' Activity loader is never forced into the embedded link.
void Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetContext(JNIEnv *, jobject, jobject, jobject);
void Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetAudioDeviceInfo(JNIEnv *, jobject, jboolean, jint, jint);
void Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetEditTextDialogResult(JNIEnv *, jobject, jbyteArray);
void Java_org_cocos2dx_lib_Cocos2dxBitmap_nativeInitBitmapDC(JNIEnv *, jobject, jint, jint, jbyteArray);
void Java_org_cocos2dx_lib_Cocos2dxAccelerometer_onSensorChanged(JNIEnv *, jobject, jfloat, jfloat, jfloat, jlong);
}

namespace {
JavaVM *host_vm = nullptr;
jclass host_class = nullptr;
jmethodID start_method = nullptr, drain_method = nullptr, stop_method = nullptr;
std::atomic<bool> ready{false};
std::mutex initialize_mutex;
class LocalFrame {
    JNIEnv *env_;
public:
    explicit LocalFrame(JNIEnv *env) : env_(env) {
        if(env_->PushLocalFrame(32) != JNI_OK) throw std::runtime_error("KRKR_JNI_LOCAL_FRAME");
    }
    ~LocalFrame() { env_->PopLocalFrame(nullptr); }
};

void check(JNIEnv *env, const char *code) {
    if(env->ExceptionCheck()) {
        env->ExceptionDescribe();
        env->ExceptionClear();
        throw std::runtime_error(code);
    }
}
void throw_java(JNIEnv *env, const char *code) noexcept {
    if(env->ExceptionCheck()) return;
    const auto type = env->FindClass("java/lang/IllegalStateException");
    if(type) { env->ThrowNew(type, code); env->DeleteLocalRef(type); }
}
void request_exit(JNIEnv *env, jclass) {
    if(const auto host = TVPGetEmbeddedSessionHost())
        host->request_exit(host->context, 0);
    else throw_java(env, "KRKR_OWNER_NOT_RUNNING");
}
void commit_text(JNIEnv *env, jclass, jstring value) {
    auto bytes = env->GetStringUTFChars(value, nullptr);
    if(!bytes) return; // JNI's pending allocation exception reaches Java.
    try {
        const std::string text(bytes);
        env->ReleaseStringUTFChars(value, bytes);
        bytes = nullptr;
        TVPMainScene::onTextInput(text);
    } catch(...) {
        if(bytes) env->ReleaseStringUTFChars(value, bytes);
        throw_java(env, "KRKR_TEXT_INPUT_FAILED");
    }
}
void register_methods(JNIEnv *env, const char *name, JNINativeMethod *methods, jint count) {
    // JniHelper uses the application classloader even on attached owner threads.
    cocos2d::JniMethodInfo lookup;
    if(!cocos2d::JniHelper::getStaticMethodInfo(lookup, name, methods[0].name, methods[0].signature))
        throw std::runtime_error("KRKR_JNI_CLASS_OR_SIGNATURE");
    const auto result = env->RegisterNatives(lookup.classID, methods, count);
    env->DeleteLocalRef(lookup.classID);
    check(env, "KRKR_JNI_REGISTER");
    if(result != JNI_OK) throw std::runtime_error("KRKR_JNI_REGISTER");
}
void owner_call(jmethodID method) {
    // Original JniHelper attaches native-created threads and registers its TLS
    // destructor to DetachCurrentThread. No JNIEnv crosses a thread boundary.
    auto env = cocos2d::JniHelper::getEnv();
    if(!env || !host_class || !method) throw std::runtime_error("KRKR_ANDROID_HOST_UNINITIALIZED");
    LocalFrame frame(env);
    env->CallStaticVoidMethod(host_class, method);
    check(env, "KRKR_JAVA_OWNER_CALLBACK");
}
}

extern "C" ShinkiEmuStatus shinki_emu_krkr2_android_initialize(void *vm, void *activity) noexcept {
    try {
        std::lock_guard<std::mutex> lock(initialize_mutex);
        if(!vm || !activity || (host_vm && host_vm != vm)) return ShinkiEmuINVALID_STATE;
        auto java_vm = static_cast<JavaVM *>(vm);
        JNIEnv *env = nullptr;
        if(java_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) return ShinkiEmuINVALID_STATE;
        LocalFrame frame(env);
        if(host_class) {
            // NativeActivity may be recreated in the same process. The old
            // owner has joined before Java releases its Activity references;
            // retain only process-lifetime VM/application-classloader state.
            auto method = env->GetStaticMethodID(host_class, "initialize", "(Landroid/app/Activity;)V");
            check(env, "KRKR_JAVA_HOST_INITIALIZER");
            if(!method) throw std::runtime_error("KRKR_JAVA_HOST_INITIALIZER");
            ready.store(false, std::memory_order_release);
            env->CallStaticVoidMethod(host_class, method, static_cast<jobject>(activity));
            check(env, "KRKR_JAVA_HOST_INITIALIZATION");
            ready.store(true, std::memory_order_release);
            return ShinkiEmuOK;
        }
        if(!host_vm) {
            host_vm = java_vm;
            cocos2d::JniHelper::setJavaVM(host_vm);
        }
        auto activity_class = env->GetObjectClass(static_cast<jobject>(activity));
        check(env, "KRKR_ACTIVITY_CLASS");
        if(!activity_class) throw std::runtime_error("KRKR_ACTIVITY_CLASS");
        auto get_context = env->GetMethodID(activity_class, "getApplicationContext", "()Landroid/content/Context;");
        check(env, "KRKR_APPLICATION_CONTEXT");
        if(!get_context) throw std::runtime_error("KRKR_APPLICATION_CONTEXT");
        auto context = env->CallObjectMethod(static_cast<jobject>(activity), get_context);
        check(env, "KRKR_APPLICATION_CONTEXT");
        // Process-lifetime context/classloader, not a leaked Activity global ref.
        if(!context || !cocos2d::JniHelper::setClassLoaderFrom(context)) throw std::runtime_error("KRKR_CLASSLOADER");
        env->DeleteLocalRef(context);
        env->DeleteLocalRef(activity_class);
        JNINativeMethod helper[] = {
            {const_cast<char *>("nativeSetContext"), const_cast<char *>("(Landroid/content/Context;Landroid/content/res/AssetManager;)V"), reinterpret_cast<void *>(Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetContext)},
            {const_cast<char *>("nativeSetAudioDeviceInfo"), const_cast<char *>("(ZII)V"), reinterpret_cast<void *>(Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetAudioDeviceInfo)},
            {const_cast<char *>("nativeSetEditTextDialogResult"), const_cast<char *>("([B)V"), reinterpret_cast<void *>(Java_org_cocos2dx_lib_Cocos2dxHelper_nativeSetEditTextDialogResult)},
        };
        JNINativeMethod bitmap[] = {{const_cast<char *>("nativeInitBitmapDC"), const_cast<char *>("(II[B)V"), reinterpret_cast<void *>(Java_org_cocos2dx_lib_Cocos2dxBitmap_nativeInitBitmapDC)}};
        JNINativeMethod sensor[] = {{const_cast<char *>("onSensorChanged"), const_cast<char *>("(FFFJ)V"), reinterpret_cast<void *>(Java_org_cocos2dx_lib_Cocos2dxAccelerometer_onSensorChanged)}};
        JNINativeMethod platform[] = {{const_cast<char *>("nativeCommitText"), const_cast<char *>("(Ljava/lang/String;)V"), reinterpret_cast<void *>(commit_text)}};
        JNINativeMethod host[] = {
            {const_cast<char *>("nativeRequestExit"), const_cast<char *>("()V"), reinterpret_cast<void *>(request_exit)},
        };
        register_methods(env, "org.cocos2dx.lib.Cocos2dxHelper", helper, 3);
        register_methods(env, "org.cocos2dx.lib.Cocos2dxBitmap", bitmap, 1);
        register_methods(env, "org.cocos2dx.lib.Cocos2dxAccelerometer", sensor, 1);
        register_methods(env, "org.tvp.krkr2core.KR2Activity", platform, 1);
        register_methods(env, "org.shinkirou.krk2.KrkrHost", host, 1);
        cocos2d::JniMethodInfo initialize;
        if(!cocos2d::JniHelper::getStaticMethodInfo(initialize, "org.shinkirou.krk2.KrkrHost", "initialize", "(Landroid/app/Activity;)V"))
            throw std::runtime_error("KRKR_JAVA_HOST_INITIALIZER");
        host_class = static_cast<jclass>(env->NewGlobalRef(initialize.classID));
        check(env, "KRKR_JAVA_HOST_CLASS");
        if(!host_class) throw std::runtime_error("KRKR_JAVA_HOST_CLASS");
        start_method = env->GetStaticMethodID(host_class, "ownerStarted", "()V");
        drain_method = env->GetStaticMethodID(host_class, "drain", "()V");
        stop_method = env->GetStaticMethodID(host_class, "ownerStopped", "()V");
        check(env, "KRKR_JAVA_OWNER_SIGNATURES");
        if(!start_method || !drain_method || !stop_method) throw std::runtime_error("KRKR_JAVA_OWNER_SIGNATURES");
        env->CallStaticVoidMethod(host_class, initialize.methodID, static_cast<jobject>(activity));
        env->DeleteLocalRef(initialize.classID);
        check(env, "KRKR_JAVA_HOST_INITIALIZATION");
        ready.store(true, std::memory_order_release);
        return ShinkiEmuOK;
    } catch(...) {
        return ShinkiEmuCORE_ERROR;
    }
}

namespace shinki::krkr::android {
bool initialized() noexcept { return ready.load(std::memory_order_acquire); }
void owner_started() { owner_call(start_method); }
void drain() { owner_call(drain_method); }
void owner_stopped() { owner_call(stop_method); }
}
