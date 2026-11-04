package org.tvp.krkr2core;

import android.app.Activity;
import android.app.ActivityManager;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Debug;
import android.provider.Settings;
import android.view.ViewGroup;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Locale;
import org.shinkirou.krk2.KrkrHost;

/** Original platform service name, backed by the existing Manager Activity.
 * This is a ContextWrapper, not an Activity or an application entry point. */
public final class KR2Activity extends ContextWrapper {
    private static KR2Activity instance;
    private static final ActivityManager.MemoryInfo memory = new ActivityManager.MemoryInfo();
    private static final Debug.MemoryInfo process = new Debug.MemoryInfo();
    private static EditText editor;
    private KR2Activity(Context context) { super(context); }
    public static void initialize(Activity host) { instance = new KR2Activity(host.getApplicationContext()); }
    public static KR2Activity GetInstance() {
        if (instance == null) throw new IllegalStateException("KRKR_PLATFORM_UNINITIALIZED");
        return instance;
    }
    public static void updateMemoryInfo() {
        ((ActivityManager)GetInstance().getSystemService(Context.ACTIVITY_SERVICE)).getMemoryInfo(memory);
        Debug.getMemoryInfo(process);
    }
    public static long getAvailMemory() { return memory.availMem; }
    public static long getUsedMemory() { return process.getTotalPss(); }
    public static String GetVersion() {
        try { return GetInstance().getPackageManager().getPackageInfo(GetInstance().getPackageName(), 0).versionName; }
        catch (android.content.pm.PackageManager.NameNotFoundException error) { throw new IllegalStateException(error); }
    }
    public static String getAndroidDeviceId() {
        return Settings.Secure.getString(GetInstance().getContentResolver(), Settings.Secure.ANDROID_ID);
    }
    public static String getLocaleName() {
        Locale locale = Locale.getDefault();
        return locale.getLanguage() + (locale.getCountry().isEmpty() ? "" : "_" + locale.getCountry().toLowerCase(Locale.ROOT));
    }
    public String[] getStoragePath() {
        ArrayList<String> paths = new ArrayList<>();
        paths.add(getFilesDir().getAbsolutePath());
        for (File directory : getExternalFilesDirs(null)) if (directory != null) paths.add(directory.getAbsolutePath());
        return paths.toArray(new String[0]);
    }
    public static boolean isWritableNormalOrSaf(String path) { return new File(path).canWrite(); }
    public static boolean CreateFolders(String path) { File directory = new File(path); return directory.isDirectory() || directory.mkdirs(); }
    public static boolean WriteFile(String path, byte[] bytes) {
        try (FileOutputStream stream = new FileOutputStream(path)) { stream.write(bytes); return true; }
        catch (IOException error) { return false; }
    }
    public static boolean DeleteFile(String path) { return new File(path).delete(); }
    public static boolean RenameFile(String source, String destination) { return new File(source).renameTo(new File(destination)); }

    private static native void nativeCommitText(String text);
    public static void showTextInput(int x, int y, int width, int height) {
        Activity host = KrkrHost.activity();
        host.runOnUiThread(() -> {
            if (editor == null) {
                editor = new EditText(host);
                editor.setSingleLine(true);
                editor.setOnEditorActionListener((view, action, event) -> {
                    final String text = view.getText().toString();
                    KrkrHost.queue(() -> nativeCommitText(text));
                    hideTextInput();
                    return true;
                });
                host.addContentView(editor, new FrameLayout.LayoutParams(Math.max(width, 1), Math.max(height, 1)));
            }
            editor.setX(x); editor.setY(y); editor.setVisibility(android.view.View.VISIBLE); editor.requestFocus();
            ((InputMethodManager)host.getSystemService(Context.INPUT_METHOD_SERVICE)).showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
        });
    }
    public static void hideTextInput() {
        Activity host = KrkrHost.activity();
        host.runOnUiThread(() -> {
            if (editor != null) {
                ((InputMethodManager)host.getSystemService(Context.INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(editor.getWindowToken(), 0);
                editor.setVisibility(android.view.View.GONE);
            }
        });
    }
    public static void release() {
        if (editor != null) {
            ViewGroup parent = (ViewGroup)editor.getParent();
            if (parent != null) parent.removeView(editor);
            editor = null;
        }
        instance = null;
    }
}
