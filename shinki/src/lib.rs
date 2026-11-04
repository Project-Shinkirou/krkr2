//! The original KrKr2 VM, Cocos renderer and audio scheduler live in the
//! linked native module. Static Android builds isolate its private symbol closure.
use shinki_emu_abi::{CoreApi, Query, Status};

unsafe extern "C" {
    #[link_name = "shinki_emu_krkr2_query"]
    pub fn query(version: u32, output: *mut *const CoreApi) -> Status;
}

#[cfg(feature = "static")]
unsafe extern "C" {
    fn shinki_emu_krkr2_set_resource_root(path: *const u8, length: usize) -> Status;
    fn shinki_emu_krkr2_android_initialize(vm: *mut std::ffi::c_void, activity: *mut std::ffi::c_void) -> Status;
}

/// Configure the APK resources extracted into the Manager's app-private tree.
#[cfg(feature = "static")]
pub fn set_resource_root(path: &str) -> Status {
    unsafe { shinki_emu_krkr2_set_resource_root(path.as_ptr(), path.len()) }
}

/// Initialize the private Cocos/SDL Java consumers before query registration.
///
/// # Safety
/// `vm` must remain valid for the process lifetime. `activity` is a live local
/// AndroidApp reference for this call only; native code does not retain it.
/// The calling thread must be attached to `vm`. The host forwards lifecycle
/// events and releases Java helper Activity references after native teardown.
#[cfg(all(feature = "static", target_os = "android"))]
pub unsafe fn initialize_android(vm: *mut std::ffi::c_void, activity: *mut std::ffi::c_void) -> Status {
    unsafe { shinki_emu_krkr2_android_initialize(vm, activity) }
}

pub const QUERY: Query = query;

/// The production session lifecycle (`native/NativeLifecycle.h`) driven
/// through test-only C entry points. No VM, GL context or session is started.
#[cfg(test)]
mod lifecycle {
    use shinki_emu_abi::Status;
    use std::ffi::c_void;

    #[link(name = "shinki_emu_krkr2_lifecycle_test", kind = "static")]
    unsafe extern "C" {
        fn shinki_krkr2_text_test_validate(text: *const u8, bytes: usize) -> u32;
        fn shinki_krkr2_lifecycle_test_new() -> *mut c_void;
        fn shinki_krkr2_lifecycle_test_free(lifecycle: *mut c_void);
        fn shinki_krkr2_lifecycle_test_phase(lifecycle: *const c_void) -> u32;
        fn shinki_krkr2_lifecycle_test_pause(lifecycle: *mut c_void, paused: u32);
        fn shinki_krkr2_lifecycle_test_waiting(lifecycle: *mut c_void, blocked: u32);
        fn shinki_krkr2_lifecycle_test_observe(
            lifecycle: *mut c_void,
            state: u32,
            exit_code: i32,
        ) -> u32;
        fn shinki_krkr2_lifecycle_test_end(lifecycle: *mut c_void, status: Status);
        fn shinki_krkr2_lifecycle_test_close_status(failure: Status) -> Status;
        fn shinki_krkr2_lifecycle_test_frame(thrown: u32, exit_requested: u32) -> u32;
    }

    /// `NativeSession::advance` result states.
    const ADVANCE_RUNNING: u32 = 0;
    const ADVANCE_EXITED: u32 = 2;
    /// Exceptions a native frame can end with.
    const NO_THROW: u32 = 0;
    const EABORT: u32 = 1;
    const VM_SILENT: u32 = 2;
    const OTHER: u32 = 3;

    struct Lifecycle(*mut c_void);
    impl Lifecycle {
        fn new() -> Self {
            let raw = unsafe { shinki_krkr2_lifecycle_test_new() };
            assert!(!raw.is_null());
            Self(raw)
        }
        fn phase(&self) -> u32 {
            unsafe { shinki_krkr2_lifecycle_test_phase(self.0) }
        }
        fn pause(&self, paused: bool) {
            unsafe { shinki_krkr2_lifecycle_test_pause(self.0, paused.into()) }
        }
        fn waiting(&self, blocked: bool) {
            unsafe { shinki_krkr2_lifecycle_test_waiting(self.0, blocked.into()) }
        }
        fn observe(&self, state: u32, exit_code: i32) -> bool {
            unsafe { shinki_krkr2_lifecycle_test_observe(self.0, state, exit_code) != 0 }
        }
        fn end(&self, status: Status) {
            unsafe { shinki_krkr2_lifecycle_test_end(self.0, status) }
        }
    }
    impl Drop for Lifecycle {
        fn drop(&mut self) {
            unsafe { shinki_krkr2_lifecycle_test_free(self.0) }
        }
    }

    fn close_status(failure: Status) -> Status {
        unsafe { shinki_krkr2_lifecycle_test_close_status(failure) }
    }
    fn frame_escapes(thrown: u32, exit_requested: bool) -> bool {
        unsafe { shinki_krkr2_lifecycle_test_frame(thrown, exit_requested.into()) != 0 }
    }
    #[test]
    fn native_utf8_accepts_scalar_values_and_rejects_invalid_encodings() {
        for text in ["", "native/path", "日本語", "\u{1f680}", "\u{10ffff}"] {
            assert_ne!(unsafe { shinki_krkr2_text_test_validate(text.as_ptr(), text.len()) }, 0);
        }
        for bytes in [
            &[0xc0, 0xaf][..],
            &[0xed, 0xa0, 0x80],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xe3, 0x81],
            &[0x80],
            &[0xe3, 0x28, 0x82],
        ] {
            assert_eq!(unsafe { shinki_krkr2_text_test_validate(bytes.as_ptr(), bytes.len()) }, 0);
        }
    }


    #[test]
    fn game_exit_finishes_session_and_close_still_succeeds() {
        let lifecycle = Lifecycle::new();
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_RUNNING);
        // System.exit / System.terminate / accepted main-window close all end
        // in TVPExitApplication, which advance reports as state 2.
        assert!(lifecycle.observe(ADVANCE_EXITED, 0));
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FINISHED);
        // Teardown after the exit is clean; the session stays FINISHED and
        // close() reports OK.
        lifecycle.end(shinki_emu_abi::OK);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FINISHED);
        assert_eq!(close_status(shinki_emu_abi::OK), shinki_emu_abi::OK);
        // Host controls arriving after the exit do not hide it.
        lifecycle.pause(true);
        lifecycle.pause(false);
        lifecycle.waiting(true);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FINISHED);
    }

    #[test]
    fn exit_code_does_not_turn_game_exit_into_failure() {
        let lifecycle = Lifecycle::new();
        assert!(lifecycle.observe(ADVANCE_EXITED, 3));
        lifecycle.end(shinki_emu_abi::OK);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FINISHED);
    }

    #[test]
    fn ordinary_frames_and_host_controls_keep_session_alive() {
        let lifecycle = Lifecycle::new();
        for _ in 0..3 {
            assert!(!lifecycle.observe(ADVANCE_RUNNING, 0));
        }
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_RUNNING);
        lifecycle.pause(true);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_PAUSED);
        lifecycle.pause(false);
        lifecycle.waiting(true);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_WAITING);
        lifecycle.waiting(false);
        assert!(!lifecycle.observe(ADVANCE_RUNNING, 0));
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_RUNNING);
    }

    #[test]
    fn native_failure_stays_failed_and_close_reports_it() {
        let lifecycle = Lifecycle::new();
        lifecycle.end(shinki_emu_abi::CORE_ERROR);
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FAILED);
        assert!(lifecycle.observe(ADVANCE_EXITED, 0));
        assert_eq!(lifecycle.phase(), shinki_emu_abi::SESSION_FAILED);
        assert_eq!(
            close_status(shinki_emu_abi::CORE_ERROR),
            shinki_emu_abi::CORE_ERROR
        );
        // Host cancellation is not reported again by close().
        assert_eq!(close_status(shinki_emu_abi::CANCELLED), shinki_emu_abi::OK);
    }

    #[test]
    fn exit_unwind_ends_the_frame_only_after_an_exit_request() {
        // TVPExitApplication unwinds with EAbort, then the VM stops silently.
        assert!(!frame_escapes(EABORT, true));
        assert!(!frame_escapes(VM_SILENT, true));
        assert!(!frame_escapes(NO_THROW, false));
        // Without an exit request the same exceptions are real failures.
        assert!(frame_escapes(EABORT, false));
        assert!(frame_escapes(VM_SILENT, false));
        assert!(frame_escapes(OTHER, true));
        assert!(frame_escapes(OTHER, false));
    }
}
