#pragma once
#include "NativeOwner.h"
#include "NativeInput.h"
#include "environ/EmbeddedSession.h"
#include <memory>
#include <string>
#include <thread>

class TVPAppDelegate;
namespace shinki::krkr {
class HostModal;
class NativeSession final {
    const std::thread::id thread_ = std::this_thread::get_id();
    ShinkiKrkrCallbacks callbacks_;
    TVPEmbeddedSessionHost host_;
    NativeInput input_;
    std::unique_ptr<HostModal> modal_;
    std::unique_ptr<TVPAppDelegate> app_;
    iTVPTexture2D *texture_ = nullptr;
    tTJSNI_BaseWindow *window_ = nullptr;
    unsigned int logical_width_ = 0, logical_height_ = 0;
    bool installed_ = false, launched_ = false;
    bool dirty_ = true, changed_ = false, closed_ = false;
#if defined(__ANDROID__) && defined(SHINKI_EMU_STATIC_CORE)
    bool android_owner_ = false;
#endif
    std::atomic<bool> exited_{false};
    int exit_code_ = 0;
    static void changed(void *, tTJSNI_BaseWindow *);
    static void ready(void *, tTJSNI_BaseWindow *, iTVPTexture2D *, unsigned int, unsigned int);
    static void exit(void *, int);
    static bool fullscreen(void *, iWindowLayer *, bool);
    static void caption(void *, iWindowLayer *, const std::string &);
    void tick(float);
public:
    void check_thread() const;
    explicit NativeSession(const ShinkiKrkrBootOptions &);
    ~NativeSession();
    NativeSession(const NativeSession &) = delete;
    NativeSession &operator=(const NativeSession &) = delete;
    void boot(const ShinkiKrkrBootOptions &);
    std::uint64_t frame_interval_ns() const;
    std::uint64_t modal_wait_ns() const;
    ShinkiKrkrFrameInfo frame_info() const;
    void copy_frame(const ShinkiKrkrFrameInfo &, std::uint8_t *, std::uint64_t);
    ShinkiKrkrAdvanceResult advance(std::uint64_t, const ShinkiKrkrEvent *, std::uint32_t);
    void shutdown();
};
}
