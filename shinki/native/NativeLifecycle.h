#pragma once
#include "shinki_emu.h"
#include "NativeOwner.h"
#include <atomic>
#include <cstdint>
#include <utility>

namespace shinki::krkr {
// Host-visible session phase. The original exit paths (System.exit,
// System.terminate, the main-window close accepted by onCloseQuery, and the
// exit after an error dialog) all end in TVPExitApplication, which the embedded
// build reports through NativeSession::advance as state 2 instead of ending
// the process. FINISHED and FAILED are final: a later pause, resume or modal
// wait cannot replace them, so the Host always observes how the game ended.
class Lifecycle {
    std::atomic<std::uint32_t> phase_{ShinkiEmuSESSION_RUNNING};
    static constexpr bool is_final(std::uint32_t phase) noexcept {
        return phase == ShinkiEmuSESSION_FINISHED || phase == ShinkiEmuSESSION_FAILED;
    }
    bool move(std::uint32_t next) noexcept {
        auto current = phase_.load(std::memory_order_acquire);
        do {
            if(is_final(current)) return false;
        } while(!phase_.compare_exchange_weak(current, next, std::memory_order_acq_rel, std::memory_order_acquire));
        return true;
    }

public:
    std::uint32_t phase() const noexcept { return phase_.load(std::memory_order_acquire); }
    void pause(bool paused) noexcept { move(paused ? ShinkiEmuSESSION_PAUSED : ShinkiEmuSESSION_RUNNING); }
    // A Host modal blocks the owner thread; it never ends the session.
    void waiting(bool blocked) noexcept { move(blocked ? ShinkiEmuSESSION_WAITING : ShinkiEmuSESSION_RUNNING); }
    // Owner loop: true once the game has ended itself; the loop then stops and
    // tears the native session down normally.
    bool observe(const ShinkiKrkrAdvanceResult &result) noexcept {
        if(result.state != 2) return false;
        move(ShinkiEmuSESSION_FINISHED);
        return true;
    }
    // Worker result after native teardown. Startup, frame or cleanup failure
    // (and Host cancellation) report FAILED; a clean end keeps FINISHED.
    void end(ShinkiEmuStatus status) noexcept {
        if(status != ShinkiEmuOK) phase_.store(ShinkiEmuSESSION_FAILED, std::memory_order_release);
    }
    // close() succeeds after a game exit and after Host cancellation; only a
    // real native failure is returned again.
    static ShinkiEmuStatus close_status(ShinkiEmuStatus failure) noexcept {
        return failure == ShinkiEmuCANCELLED ? ShinkiEmuOK : failure;
    }
};

// TVPExitApplication requests the exit and then unwinds the current frame with
// EAbort; once the exit is recorded the VM also stops with its silent
// exception. Outside tTVPApplication::Run (an exit after an error dialog, or a
// close handler) that unwinding reaches the owner. After an exit request it is
// the expected end of the frame, not a native failure. Any other exception,
// including Host cancellation, still propagates.
template<class Abort, class Silent, class Exited, class Frame>
void absorb_exit_unwind(Exited &&exit_requested, Frame &&frame) {
    try {
        std::forward<Frame>(frame)();
    } catch(const Abort &) {
        if(!exit_requested()) throw;
    } catch(const Silent &) {
        if(!exit_requested()) throw;
    }
}
}
