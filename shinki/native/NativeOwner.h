#pragma once
#include "HostModalAbi.h"
#include <cstdint>
#include <exception>
#include <string>
struct NativeCancelled : std::exception {
    const char *what() const noexcept override { return "KRKR_CANCELLED"; }
};

// Private C++ session seam. Public callers use shinki_emu.h exclusively.
struct ShinkiKrkrWindowState { std::uint32_t width, height, focused, visible; };
struct ShinkiKrkrFrameInfo { std::uint32_t width, height, logical_width, logical_height, stride; };
struct ShinkiKrkrCallbacks {
    void *context;
    std::uint32_t (*is_cancelled)(void *);
    ShinkiKrkrModalCallback modal;
    bool (*fullscreen)(void *, bool);
    void (*caption)(void *, const std::string &);
};
struct ShinkiKrkrBootOptions {
    const std::uint8_t *game_path_utf8;
    std::uint64_t game_path_bytes;
    const std::uint8_t *resource_root_utf8;
    std::uint64_t resource_root_bytes;
    const std::uint8_t *writable_root_utf8;
    std::uint64_t writable_root_bytes;
    ShinkiKrkrWindowState window;
    ShinkiKrkrCallbacks callbacks;
};
struct ShinkiKrkrEvent {
    std::uint32_t kind, key_code, key_state, modifiers, pointer_button, value;
    float x, y;
    std::uint32_t width, height;
    const std::uint8_t *text_utf8;
    std::uint64_t text_bytes;
};
struct ShinkiKrkrAdvanceResult {
    std::uint32_t state;
    std::int32_t exit_code;
    std::uint32_t reset_clock;
    std::uint32_t frame_changed;
};
