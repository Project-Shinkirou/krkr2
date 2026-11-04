# ShinkiEMU KrKr2 embedding

Upstream: https://github.com/2468785842/krkr2

Base: dca72645 (the complete upstream commit is recorded in `shinki/Cargo.toml`). Upstream engine, VM, renderer, mixer, plugins and game preferences remain in this repository. This is an independent `krkr2` core, not a forwarding adapter for `kirikiroid2-web`.

The C ABI v3 adapter lives in `shinki/`. Its owner-thread, frame-lease, cancellation, modal-service and physical-input implementation is adapted from Project-Shinkirou/kirikiroid2-web at 74cf8356 (`shinki-apple-ios63`), under that adapter's MIT OR Apache-2.0 license. The engine embedding seams were ported to KrKr2's modular archive, thread and window layout; KrKr2 does not use the other fork's SDL runtime. Original upstream notices remain unchanged.

- `shinki_emu_query` is the dynamic C entrypoint; static Manager builds expose only the unique `shinki_emu_krkr2_query` entrypoint through Rust `query`.
- Probe recognizes loose `startup.tjs` (9400) and `data.xp3` (9300). These scores are below the existing kirikiroid2-web core's 9500, preserving its default selection when both are installed.
- Each session boots the original TJS VM on a native owner thread. Android uses an EGL pbuffer; iOS uses an EAGL framebuffer without a second application window. Game textures are published through bounded immutable CPU leases.
- Pause/resume, physical keys, text, pointer, wheel and window state are delivered to the original engine. Host modal requests are cancellable; close cancels the VM and joins native producers before releasing the renderer and audio device. The original OpenAL/Oboe mixing implementation remains in use.
- VM caches, at-exit callbacks and native resources are reset between sessions. Native exceptions stay behind the C ABI boundary; a failed teardown prevents unsafe reuse.
- Static linking resolves CMake's transitive archive closure and prefixes its defined symbols. Android Java/JNI packages use independent `org.kr2rc2dx`, `org.shinkirou.krk2` and `org.tvp.krkr2core` namespaces, allowing both KiriKiri implementations in one Manager process.
- The Cocos overlay gains explicit iOS selection, an iOS PVR header fix and JNI local-reference ownership fixes. Its macOS audio compatibility patch no longer renames the real iOS AVAudioSession class.
- The pinned AsmJit overlay includes Apple's instruction-cache declarations on iOS as well as macOS. It preserves `sys_icache_invalidate` rather than omitting cache maintenance.

The original per-game preferences and save paths remain upstream-owned. The temporary writable resource directory belongs only to Cocos runtime resources and is removed after session shutdown; no adapter-managed save root is introduced.

Build with the target's prepared vcpkg dependencies using the ShinkiEMU `native-vcpkg`/`bison` recipes. The full source tree, this notice, original LICENSE and dependency copyright files accompany redistributions. Fork source: https://github.com/Project-Shinkirou/krkr2/tree/shinki .

## AsmJit iOS header backport

- Port: `asmjit`, upstream vcpkg version-date `2025-10-13`; this repository's overlay adds port-version `1`. Its source stays pinned to `9eb6edbf711ceb25346ee40bae68b40a4505cdf5` and the upstream SHA512 in `vcpkg/ports/asmjit/portfile.cmake`.
- Patch: `vcpkg/ports/asmjit/apple-icache-header.patch`. In [upstream `src/asmjit/core/virtmem.cpp`, lines 61–69](https://github.com/asmjit/asmjit/blob/9eb6edbf711ceb25346ee40bae68b40a4505cdf5/src/asmjit/core/virtmem.cpp#L61-L69), move the `libkern/OSCacheControl.h` include out of `#if TARGET_OS_OSX` into its enclosing `#if defined(__APPLE__)`. The macOS-only `sys/utsname.h` include stays under `TARGET_OS_OSX`.
- Reason: the existing Apple branch in `flushInstructionCache` calls `sys_icache_invalidate` on iOS as well as macOS ([lines 1104–1110](https://github.com/asmjit/asmjit/blob/9eb6edbf711ceb25346ee40bae68b40a4505cdf5/src/asmjit/core/virtmem.cpp#L1104-L1110)), but the original guard hides its declaration from the iOS SDK. The actual arm64-ios build failed at line 1109 with “use of undeclared identifier `sys_icache_invalidate`”. The call is unchanged; no cache flush is disabled.
- Upstream already contains this correction in AsmJit **1.23**, revision [`4f85506f12ee21f2b5f819f06b5d77aaefcbc9b2`](https://github.com/asmjit/asmjit/commit/4f85506f12ee21f2b5f819f06b5d77aaefcbc9b2): see [`asmjit/core/virt_mem.cpp`, lines 65–74](https://github.com/asmjit/asmjit/blob/4f85506f12ee21f2b5f819f06b5d77aaefcbc9b2/asmjit/core/virt_mem.cpp#L65-L74). No duplicate upstream PR is needed. Retain this backport until the project's pinned vcpkg port upgrades to a source revision containing that correction.

## FFmpeg iOS configure target

The `ffmpeg@3.3.9` overlay adds port-version `2`. Its iOS branch now passes `--target-os=darwin`, matching FFmpeg's Apple platform name. Previously it enabled cross compilation but omitted that required configure option, causing the arm64-ios configure step to fail with `Must specify target arch and OS when cross-compiling`. `build.sh.in` already passes `--arch=${BUILD_ARCH}` and retains that architecture selection, SDK/compiler flags and all configure checks. FFmpeg is still built and linked; no codec, probe or validation is replaced or skipped.

The embedded CMake entrypoint preserves the explicitly selected vcpkg triplet instead of invoking the standalone Android script that forcibly resets it. Android chainloads the NDK toolchain from `ANDROID_NDK_HOME`; iOS passes its system name, device/simulator SDK, architecture and deployment target explicitly. Cross builds cannot silently select the host platform.

Android's native dependency manifest now explicitly includes Oboe, which the original wave renderer already requires. The pinned vcpkg Oboe recipe installs its headers and library but no CMake package config, so the sound module requires the real header/library directly and retains its `log`/`OpenSLES` platform links. No audio implementation is bypassed.

The `oboe@1.10.0#1` overlay backports vcpkg's `fix-ndk-r30.patch` for [google/oboe issue 2406](https://github.com/google/oboe/issues/2406). Android NDK 30 supplies the playback-parameter declarations and makes `AAudio_DeviceType` an enum; the old Oboe guard redeclared those types and its integer-alias assertion rejected that valid enum. The patch uses the SDK declarations on NDK 30, checks the type's 32-bit size, and retains every enumerator-value assertion. It neither suppresses compiler errors nor replaces the real audio backend.
