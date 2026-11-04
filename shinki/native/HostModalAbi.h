#pragma once
#include <cstdint>

struct ShinkiKrkrModalText {
    const std::uint8_t *data;
    std::uint64_t bytes;
};
struct ShinkiKrkrModalRequest {
    ShinkiKrkrModalText caption;
    ShinkiKrkrModalText message;
    const ShinkiKrkrModalText *buttons;
    std::uint32_t button_count;
    std::uint32_t has_input;
    ShinkiKrkrModalText initial_text;
};
struct ShinkiKrkrModalResponse {
    std::uint32_t kind; // 0: cancelled, 1: submitted
    std::uint32_t button_index;
    std::uint8_t *text;
    std::uint64_t text_capacity;
    std::uint64_t text_bytes;
};
using ShinkiKrkrModalCallback = std::int32_t (*)(
    void *, const ShinkiKrkrModalRequest *, ShinkiKrkrModalResponse *);

static_assert(sizeof(void *) == 8, "The native adapter requires x64");
static_assert(sizeof(ShinkiKrkrModalText) == 16);
static_assert(sizeof(ShinkiKrkrModalRequest) == 64);
static_assert(sizeof(ShinkiKrkrModalResponse) == 32);
