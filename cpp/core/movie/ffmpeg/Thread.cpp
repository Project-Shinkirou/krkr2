#include "Thread.h"
#include <thread>
#include <stdexcept>
#include "MsgIntf.h"
#include "ThreadImpl.h"
#include "tjsError.h"

NS_KRMOVIE_BEGIN

CThread::CThread() : m_bStop(false), m_bRunning(false) {}

CThread::~CThread() {
    if(m_bRunning) {
        StopThread();
    }
    if(m_ThreadId) {
        m_ThreadId->join();
        delete m_ThreadId;
    }
}

void CThread::Create() {
    if(m_bRunning.exchange(true)) {
        TVPThrowExceptionMessage(TJS_W("thread already in running"));
    }
    m_bStop = false;
    if(m_ThreadId) {
        m_ThreadId->join();
        delete m_ThreadId;
    }
    m_ThreadId = new std::thread(&CThread::entry, this);
}

void CThread::StopThread(bool bWait /*= true*/) {
    m_bStop = true;
    m_StopEvent.notify_all();
    if(m_ThreadId && bWait) {
        m_ThreadId->join();
        delete m_ThreadId;
        m_ThreadId = nullptr;
    }
}

void CThread::Sleep(unsigned int milliseconds) {
    if(IsCurrentThread()) {
        std::unique_lock<std::mutex> lock(m_mtxStopEvent);
        m_StopEvent.wait_for(lock, std::chrono::milliseconds(milliseconds));
    } else {
        std::this_thread::sleep_for(std::chrono::milliseconds(milliseconds));
    }
}

bool CThread::IsCurrentThread() {
    if(!m_ThreadId)
        return false;
    return m_ThreadId->get_id() == std::this_thread::get_id();
}

int CThread::entry() {
    // Preserve the worker failure for owner teardown, while always running OnExit
    // and joining. A cancelled worker VM must not terminate the embedding process.
    try {
        OnStartup();
        Process();
    } catch(const TJS::eTJSSilent &) {
        m_bStop = true;
    } catch(...) {
        m_bStop = true;
        TVPRecordWorkerFailure(std::current_exception());
    }
    try { OnExit(); }
    catch(...) { TVPRecordWorkerFailure(std::current_exception()); }
    m_bRunning = false;
    try { TVPOnThreadExited(); }
    catch(...) { TVPRecordWorkerFailure(std::current_exception()); }
    return 0;
}

NS_KRMOVIE_END
