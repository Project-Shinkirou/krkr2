#pragma once
namespace shinki::krkr::android {
bool initialized() noexcept;
void owner_started();
void drain();
void owner_stopped();
}
