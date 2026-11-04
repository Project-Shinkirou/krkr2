// Test-only C entry points over the production session lifecycle unit. This
// static library is linked only by the Rust crate's unit tests; the shipped
// core DLL does not contain or export it, and no VM or GL session is opened.
#include "NativeLifecycle.h"
#include "NativeText.h"
#include <cstdint>
#include <new>
#include <stdexcept>

namespace {
struct Abort {};
struct Silent {};
}

extern "C" {
std::uint32_t shinki_krkr2_text_test_validate(const char *text, std::size_t bytes) noexcept {
    return shinki::krkr::valid_utf8(text, bytes);
}
void *shinki_krkr2_lifecycle_test_new() noexcept { return new(std::nothrow) shinki::krkr::Lifecycle; }
void shinki_krkr2_lifecycle_test_free(void *p) noexcept { delete static_cast<shinki::krkr::Lifecycle *>(p); }
std::uint32_t shinki_krkr2_lifecycle_test_phase(const void *p) noexcept {
    return static_cast<const shinki::krkr::Lifecycle *>(p)->phase();
}
void shinki_krkr2_lifecycle_test_pause(void *p, std::uint32_t paused) noexcept {
    static_cast<shinki::krkr::Lifecycle *>(p)->pause(paused != 0);
}
void shinki_krkr2_lifecycle_test_waiting(void *p, std::uint32_t blocked) noexcept {
    static_cast<shinki::krkr::Lifecycle *>(p)->waiting(blocked != 0);
}
// Owner loop step for one NativeSession::advance result; 1 stops the loop.
std::uint32_t shinki_krkr2_lifecycle_test_observe(void *p, std::uint32_t state, std::int32_t exit_code) noexcept {
    ShinkiKrkrAdvanceResult result{};
    result.state = state;
    result.exit_code = exit_code;
    return static_cast<shinki::krkr::Lifecycle *>(p)->observe(result);
}
void shinki_krkr2_lifecycle_test_end(void *p, ShinkiEmuStatus status) noexcept {
    static_cast<shinki::krkr::Lifecycle *>(p)->end(status);
}
ShinkiEmuStatus shinki_krkr2_lifecycle_test_close_status(ShinkiEmuStatus failure) noexcept {
    return shinki::krkr::Lifecycle::close_status(failure);
}
// One native frame that throws `thrown` (0 none, 1 EAbort, 2 silent VM stop,
// 3 any other exception). Returns 0 when the frame ends normally, 1 when the
// exception reaches the owner.
std::uint32_t shinki_krkr2_lifecycle_test_frame(std::uint32_t thrown, std::uint32_t exit_requested) noexcept {
    try {
        shinki::krkr::absorb_exit_unwind<Abort, Silent>([&] { return exit_requested != 0; }, [&] {
            if(thrown == 1) throw Abort{};
            if(thrown == 2) throw Silent{};
            if(thrown == 3) throw std::runtime_error("frame");
        });
        return 0;
    } catch(...) {
        return 1;
    }
}
}
