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

The original per-game preferences and save paths remain upstream-owned. The temporary writable resource directory belongs only to Cocos runtime resources and is removed after session shutdown; no adapter-managed save root is introduced.

Build with the target's prepared vcpkg dependencies using the ShinkiEMU `native-vcpkg`/`bison` recipes. The full source tree, this notice, original LICENSE and dependency copyright files accompany redistributions. Fork source: https://github.com/Project-Shinkirou/krkr2/tree/shinki .
