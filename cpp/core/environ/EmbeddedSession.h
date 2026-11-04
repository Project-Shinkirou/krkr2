#pragma once
#include <atomic>
#include <string>

class tTJSNI_BaseWindow;
class iWindowLayer;
class iTVPTexture2D;

// All callbacks run on the original VM/window owner. These are internal core
// boundaries, not the public Family ABI; no window handle leaves the adapter.
struct TVPEmbeddedSessionHost {
    void *context;
    const char *renderer;
    void (*frame_changed)(void *, tTJSNI_BaseWindow *);
    void (*frame_ready)(void *, tTJSNI_BaseWindow *, iTVPTexture2D *, unsigned int, unsigned int);
    void (*request_exit)(void *, int);
    bool (*set_fullscreen)(void *, iWindowLayer *, bool);
    void (*set_caption)(void *, iWindowLayer *, const std::string &);
};
inline std::atomic<const TVPEmbeddedSessionHost *> TVPEmbeddedSessionHostSlot{nullptr};
inline const TVPEmbeddedSessionHost *TVPGetEmbeddedSessionHost() noexcept {
    return TVPEmbeddedSessionHostSlot.load(std::memory_order_acquire);
}
inline bool TVPInstallEmbeddedSessionHost(const TVPEmbeddedSessionHost *host) noexcept {
    if(!host || !host->context || !host->renderer || !host->frame_changed || !host->frame_ready || !host->request_exit || !host->set_fullscreen || !host->set_caption)
        return false;
    const TVPEmbeddedSessionHost *expected = nullptr;
    return TVPEmbeddedSessionHostSlot.compare_exchange_strong(
        expected, host, std::memory_order_release, std::memory_order_relaxed);
}
inline bool TVPRemoveEmbeddedSessionHost(const TVPEmbeddedSessionHost *host) noexcept {
    auto expected = host;
    return TVPEmbeddedSessionHostSlot.compare_exchange_strong(
        expected, nullptr, std::memory_order_acq_rel, std::memory_order_acquire);
}
