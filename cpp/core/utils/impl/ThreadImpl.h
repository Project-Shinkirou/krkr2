//---------------------------------------------------------------------------
/*
        TVP2 ( T Visual Presenter 2 )  A script authoring tool
        Copyright (C) 2000 W.Dee <dee@kikyou.info> and contributors

        See details of license at "license.txt"
*/
//---------------------------------------------------------------------------
// Thread base class
//---------------------------------------------------------------------------
#ifndef ThreadImplH
#define ThreadImplH

#include "tjsNative.h"
#include "ThreadIntf.h"
#include <condition_variable>
#include <thread>
#include <atomic>
#include <exception>
void TVPRecordWorkerFailure(std::exception_ptr failure);
void TVPThrowWorkerFailure();

//---------------------------------------------------------------------------
// tTVPThread
//---------------------------------------------------------------------------
class tTVPThread {
    std::atomic<bool> Terminated{false};
    std::thread Handle;
    // DWORD ThreadId;
    std::mutex _mutex; // for suspend
    std::condition_variable _cond;
    bool Suspended;

    static void * /*__stdcall*/ StartProc(void *arg);

public:
    tTVPThread(bool suspended);

    virtual ~tTVPThread();

    [[nodiscard]] bool GetTerminated() const { return Terminated; }

    void SetTerminated(bool s) { Terminated = s; }

    void Terminate() { Terminated = true; Resume(); }

protected:
    virtual void Execute() = 0;

public:
    void WaitFor();

    tTVPThreadPriority GetPriority();

    void SetPriority(tTVPThreadPriority pri);

    // void Suspend();
    void Resume();

    // 	HANDLE GetHandle() const { return Handle; } 	/* win32 specific
    // */ 	DWORD GetThreadId() const { return ThreadId; }  /* win32
    // specific */
};
//---------------------------------------------------------------------------

//---------------------------------------------------------------------------
// tTVPThreadEvent
//---------------------------------------------------------------------------
class tTVPThreadEvent {
    std::condition_variable Handle;
    std::mutex Mutex;

public:
    void Set();

    void WaitFor(tjs_uint timeout);
};
//---------------------------------------------------------------------------

void TVPAddOnThreadExitEvent(const std::function<void()> &ev);

void TVPOnThreadExited();
//---------------------------------------------------------------------------
#endif
