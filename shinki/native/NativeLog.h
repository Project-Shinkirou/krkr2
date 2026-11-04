#pragma once
#include "shinki_emu.h"
#include <spdlog/sinks/base_sink.h>
#include <mutex>

namespace shinki::krkr {
class NativeLog final : public spdlog::sinks::base_sink<std::mutex> {
    ShinkiEmuDiagnostics diagnostics_;
    void sink_it_(const spdlog::details::log_msg &message) override {
        if(!diagnostics_.emit) return;
        ShinkiEmuLogRecord record{};
        switch(message.level) {
        case spdlog::level::critical: case spdlog::level::err: record.level = ShinkiEmuLOG_ERROR; break;
        case spdlog::level::warn: record.level = ShinkiEmuLOG_WARN; break;
        case spdlog::level::info: record.level = ShinkiEmuLOG_INFO; break;
        case spdlog::level::debug: record.level = ShinkiEmuLOG_DEBUG; break;
        default: record.level = ShinkiEmuLOG_TRACE; break;
        }
        record.target = {reinterpret_cast<const uint8_t *>(message.logger_name.data()), message.logger_name.size()};
        record.message = {reinterpret_cast<const uint8_t *>(message.payload.data()), message.payload.size()};
        if(record.target.len > 256) { record.target = {}; ++record.dropped_fields; }
        if(record.message.len > 65536) { record.message = {}; ++record.dropped_fields; }
        diagnostics_.emit(diagnostics_.context, &record);
    }
    void flush_() override { if(diagnostics_.flush) diagnostics_.flush(diagnostics_.context); }
public:
    explicit NativeLog(ShinkiEmuDiagnostics diagnostics) : diagnostics_(diagnostics) {}
};
}
