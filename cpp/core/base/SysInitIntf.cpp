//---------------------------------------------------------------------------
/*
        TVP2 ( T Visual Presenter 2 )  A script authoring tool
        Copyright (C) 2000 W.Dee <dee@kikyou.info> and contributors

        See details of license at "license.txt"
*/
//---------------------------------------------------------------------------
// System Initialization and Uninitialization
//---------------------------------------------------------------------------
#include "tjsCommHead.h"

#include <vector>
#include <algorithm>
#include <functional>

#include "tjsUtils.h"
#include "SysInitIntf.h"
#include "ScriptMgnIntf.h"
#include "tvpgl.h"
#include <exception>
#include "ncbind.hpp"
#include "tjsExecutionControl.h"
#include "ThreadImpl.h"

//---------------------------------------------------------------------------
// global data
//---------------------------------------------------------------------------
ttstr TVPProjectDir; // project directory (in unified storage name)
ttstr TVPDataPath; // data directory (in unified storage name)
//---------------------------------------------------------------------------

extern void TVPGL_C_Init();

//---------------------------------------------------------------------------
// TVPSystemInit : Entire System Initialization
//---------------------------------------------------------------------------
void TVPSystemInit() {
#if CC_TARGET_PLATFORM != CC_PLATFORM_WIN32
#ifndef CC_TARGET_OS_IPHONE
    if(!TVPProtectInit())
        return;
#endif
// #else
#ifdef USING_PROTECT
    while(!TVPProtectInit()) {
        TVPUpdateLicense();
    }
#endif
#endif

    TVPBeforeSystemInit();

    TVPInitScriptEngine();

    TVPInitTVPGL();
    //	TVPGL_C_Init();

    TVPAfterSystemInit();
}
//---------------------------------------------------------------------------

//---------------------------------------------------------------------------
// TVPSystemUninit : System shutdown, cleanup, etc...
//---------------------------------------------------------------------------
static void TVPCauseAtExit(tjs_int first, tjs_int last);

bool TVPSystemUninitCalled = false;

void TVPSystemUninit() {
    if(TVPSystemUninitCalled)
        return;
    TVPSystemUninitCalled = true;

    std::exception_ptr failure;
    const auto cleanup = [&](auto call) {
        try { call(); } catch(...) { if(!failure) failure = std::current_exception(); }
    };
    cleanup([] { TVPBeforeSystemUninit(); });
    // Stop producers and release queued script closures before destroying the VM.
    cleanup([] { TVPCauseAtExit(0, TVP_ATEXIT_PRI_RELEASE); });
    cleanup([] { ncbAutoRegister::EndSession(); });
    cleanup([] { TVPThrowWorkerFailure(); });
    cleanup([] { TVPUninitScriptEngine(); });
    cleanup([] { TVPUninitTVPGL(); });
    cleanup([] { TVPAfterSystemUninit(); });
    cleanup([] { TVPCauseAtExit(TVP_ATEXIT_PRI_RELEASE, 0x7fffffff); });
    if(failure) std::rethrow_exception(failure);
}
//---------------------------------------------------------------------------

//---------------------------------------------------------------------------
// TVPAddAtExitHandler related
//---------------------------------------------------------------------------
struct tTVPAtExitInfo {
    tTVPAtExitInfo(tjs_int pri, void (*handler)()) {
        Priority = pri, Handler = handler;
    }

    tjs_int Priority;

    void (*Handler)();

    bool operator<(const tTVPAtExitInfo &r) const {
        return this->Priority < r.Priority;
    }

    bool operator>(const tTVPAtExitInfo &r) const {
        return this->Priority > r.Priority;
    }

    bool operator==(const tTVPAtExitInfo &r) const {
        return this->Priority == r.Priority;
    }
};

static std::vector<tTVPAtExitInfo> *TVPAtExitInfos = nullptr;
void TVPBeginSystemSession() {
    // Called before any VM/configuration initialization, after the previous
    // session has joined its producers and released its application.
    TVPSystemUninitCalled = false;
}

//---------------------------------------------------------------------------
void TVPAddAtExitHandler(tjs_int pri, void (*handler)()) {
    if(!TVPAtExitInfos)
        TVPAtExitInfos = new std::vector<tTVPAtExitInfo>();
    TVPAtExitInfos->emplace_back(pri, handler);
}

//---------------------------------------------------------------------------
static void TVPCauseAtExit(tjs_int first, tjs_int last) {
    if(!TVPAtExitInfos) return;
    // Registrars are static objects. Keep the registry across sessions; deleting
    // it loses every callback permanently and leaves a dangling global pointer.
    std::stable_sort(TVPAtExitInfos->begin(), TVPAtExitInfos->end());
    std::exception_ptr failure;
    for(const auto &entry : *TVPAtExitInfos) {
        if(entry.Priority < first || entry.Priority >= last) continue;
        try { entry.Handler(); } catch(...) {
            if(!failure) failure = std::current_exception();
        }
    }
    if(failure) std::rethrow_exception(failure);
}
//---------------------------------------------------------------------------
