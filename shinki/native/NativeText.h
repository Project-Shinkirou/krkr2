#pragma once
#include <cstddef>
#include <cstdint>

namespace shinki::krkr {
inline bool valid_utf8(const char *text, std::size_t bytes) noexcept {
    const auto *p = reinterpret_cast<const unsigned char *>(text);
    while(bytes) {
        const auto lead = *p++;
        --bytes;
        if(lead < 0x80) continue;
        unsigned count;
        std::uint32_t value;
        std::uint32_t minimum;
        if(lead >= 0xc2 && lead <= 0xdf) { count = 1; value = lead & 0x1f; minimum = 0x80; }
        else if(lead >= 0xe0 && lead <= 0xef) { count = 2; value = lead & 0x0f; minimum = 0x800; }
        else if(lead >= 0xf0 && lead <= 0xf4) { count = 3; value = lead & 7; minimum = 0x10000; }
        else return false;
        if(bytes < count) return false;
        bytes -= count;
        while(count--) {
            if((*p & 0xc0) != 0x80) return false;
            value = (value << 6) | (*p++ & 0x3f);
        }
        if(value < minimum || value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) return false;
    }
    return true;
}
}
