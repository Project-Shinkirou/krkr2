#pragma once
#include "tjsError.h"
#include <atomic>

namespace TJS {
// The owner installs one cancellation source for the VM and its worker VMs, then
// removes it after all workers join. Silent interruption bypasses script catch.
using TJSExecutionCancellation = bool (*)(void *);
inline std::atomic<TJSExecutionCancellation> TJSExecutionCancelled{nullptr};
inline std::atomic<void *> TJSExecutionCancellationContext{nullptr};
inline void TJSCheckExecutionCancellation() {
    const auto callback = TJSExecutionCancelled.load(std::memory_order_acquire);
    if(callback && callback(TJSExecutionCancellationContext.load(std::memory_order_acquire)))
        throw eTJSSilent();
}
}
