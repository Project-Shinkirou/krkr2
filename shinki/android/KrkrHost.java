package org.shinkirou.krk2;

import android.app.Activity;
import android.app.AlertDialog;
import java.lang.ref.WeakReference;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.kr2rc2dx.lib.Cocos2dxHelper;

/** Services for the native core; the Manager remains the only entry Activity. */
public final class KrkrHost implements Cocos2dxHelper.Cocos2dxHelperListener {
    private static WeakReference<Activity> activity = new WeakReference<>(null);
    private static final ConcurrentLinkedQueue<Runnable> pending = new ConcurrentLinkedQueue<>();
    private static volatile boolean initialized;
    private static volatile boolean ownerActive;
    private static volatile boolean resumed;

    private static native void nativeRequestExit();
    public static void requestExit() {
        if (!queue(KrkrHost::nativeRequestExit)) throw new RejectedExecutionException("KRKR_OWNER_NOT_RUNNING");
    }

    // Called by the registered native initializer, before any query registration.
    public static void initialize(Activity host) {
        // Cocos service and IME setup belongs to the Activity UI thread.
        CompletableFuture<Void> ready = new CompletableFuture<>();
        host.runOnUiThread(() -> {
            try {
                initializeServices(host);
                ready.complete(null);
            } catch (RuntimeException | Error failure) {
                ready.completeExceptionally(failure);
            }
        });
        try { ready.join(); }
        catch (CompletionException failure) {
            if (failure.getCause() instanceof Error) throw (Error)failure.getCause();
            throw (RuntimeException)failure.getCause();
        }
    }

    private static synchronized void initializeServices(Activity host) {
        if (initialized) throw new IllegalStateException("KRKR_HOST_ALREADY_INITIALIZED");
        activity = new WeakReference<>(host);
        try {
            Cocos2dxHelper.init(host);
            org.tvp.krkr2core.KR2Activity.initialize(host);
            initialized = true;
            if (resumed) Cocos2dxHelper.onResume();
        } catch (RuntimeException | Error failure) {
            try { Cocos2dxHelper.releaseHost(); }
            catch (RuntimeException cleanup) { failure.addSuppressed(cleanup); }
            activity.clear();
            throw failure;
        }
    }

    public static Activity activity() {
        Activity host = activity.get();
        if (host == null || host.isDestroyed()) throw new IllegalStateException("KRKR_ACTIVITY_UNAVAILABLE");
        return host;
    }

    public static synchronized boolean queue(Runnable runnable) {
        if (!ownerActive) return false; // An event for a closed VM has no consumer.
        pending.add(runnable);
        return true;
    }

    public static synchronized void ownerStarted() { ownerActive = true; }
    public static void drain() {
        for (Runnable next; (next = pending.poll()) != null;) next.run();
    }
    public static synchronized void ownerStopped() {
        ownerActive = false;
        Cocos2dxHelper.resetOwner();
        pending.clear();
    }

    public static synchronized void onResume() {
        resumed = true;
        if (initialized) {
            Cocos2dxHelper.onResume();
            Cocos2dxHelper.onEnterForeground();
        }
    }
    public static synchronized void onPause() {
        resumed = false;
        if (initialized) {
            Cocos2dxHelper.onPause();
            Cocos2dxHelper.onEnterBackground();
        }
    }

    // MainActivity calls this after NativeActivity.onDestroy has joined native
    // execution; there can no longer be an owner using the Java services.
    public static synchronized void destroy(Activity host) {
        if (activity.get() != host) return;
        if (ownerActive) throw new IllegalStateException("KRKR_OWNER_SURVIVED_ACTIVITY");
        if (initialized) {
            org.tvp.krkr2core.KR2Activity.release();
            Cocos2dxHelper.releaseHost();
            initialized = false;
        }
        pending.clear();
        activity.clear();
    }

    @Override public void runOnGLThread(Runnable runnable) {
        if (!queue(runnable)) throw new RejectedExecutionException("KRKR_OWNER_NOT_RUNNING");
    }
    @Override public void showDialog(String title, String message) {
        Activity host = activity();
        host.runOnUiThread(() -> new AlertDialog.Builder(host).setTitle(title)
            .setMessage(message).setPositiveButton(android.R.string.ok, null).show());
    }
}
