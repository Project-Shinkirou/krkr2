#pragma once
#include <atomic>
#include <string>
#include <vector>

// Opt-in embedding only. The host table stays alive until every native caller
// has completed; install before startup and remove after joining native workers.
struct TVPEmbeddedModalHost {
    void *context;
    int (*request)(void *, const std::string &caption,
                   const std::string &message,
                   const std::vector<std::string> &buttons,
                   std::string *input);
};

inline std::atomic<const TVPEmbeddedModalHost *> TVPEmbeddedModalHostSlot{nullptr};

inline bool TVPInstallEmbeddedModalHost(const TVPEmbeddedModalHost *host) noexcept {
    if(!host || !host->request)
        return false;
    const TVPEmbeddedModalHost *expected = nullptr;
    return TVPEmbeddedModalHostSlot.compare_exchange_strong(
        expected, host, std::memory_order_release, std::memory_order_relaxed);
}

inline bool TVPRemoveEmbeddedModalHost(const TVPEmbeddedModalHost *host) noexcept {
    auto expected = host;
    return TVPEmbeddedModalHostSlot.compare_exchange_strong(
        expected, nullptr, std::memory_order_acq_rel, std::memory_order_acquire);
}

inline const TVPEmbeddedModalHost *TVPGetEmbeddedModalHost() noexcept {
    return TVPEmbeddedModalHostSlot.load(std::memory_order_acquire);
}
