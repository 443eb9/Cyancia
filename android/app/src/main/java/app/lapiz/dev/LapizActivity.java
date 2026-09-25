package app.lapiz.dev;

import android.app.NativeActivity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.webkit.MimeTypeMap;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

public class LapizActivity extends NativeActivity {

    private static final int DOCUMENT_REQUEST = 9274;
    private volatile long documentRequest;

    private static native void onDocumentPickedNative(
        long request,
        int status,
        String uri,
        String name
    );

    public synchronized boolean requestDocument(
        long request,
        boolean save,
        String name
    ) {
        if (documentRequest != 0 || isFinishing() || isDestroyed()) {
            return false;
        }
        documentRequest = request;
        runOnUiThread(() -> {
            if (documentRequest != request) {
                return;
            }
            Intent intent = new Intent(
                save
                    ? Intent.ACTION_CREATE_DOCUMENT
                    : Intent.ACTION_OPEN_DOCUMENT
            );
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION |
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION |
                    Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
            );
            String extension = MimeTypeMap.getFileExtensionFromUrl(name);
            String mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(
                extension
            );
            intent.setType(mime != null ? mime : "*/*");
            if (save) {
                intent.putExtra(Intent.EXTRA_TITLE, name);
            }
            try {
                startActivityForResult(intent, DOCUMENT_REQUEST);
            } catch (Exception error) {
                completeDocument(-1, null, null);
            }
        });
        return true;
    }

    private synchronized void completeDocument(
        int status,
        String uri,
        String name
    ) {
        long request = documentRequest;
        documentRequest = 0;
        if (request != 0) {
            onDocumentPickedNative(request, status, uri, name);
        }
    }

    @Override
    protected void onActivityResult(
        int requestCode,
        int resultCode,
        Intent data
    ) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != DOCUMENT_REQUEST) {
            return;
        }
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            completeDocument(0, null, null);
            return;
        }
        Uri uri = data.getData();
        int flags =
            data.getFlags() &
            (Intent.FLAG_GRANT_READ_URI_PERMISSION |
                Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        if (flags != 0) {
            try {
                getContentResolver().takePersistableUriPermission(uri, flags);
            } catch (SecurityException ignored) {}
        }
        String name = null;
        try (
            Cursor cursor = getContentResolver().query(
                uri,
                new String[] { OpenableColumns.DISPLAY_NAME },
                null,
                null,
                null
            )
        ) {
            if (cursor != null && cursor.moveToFirst()) {
                int index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (index >= 0) {
                    name = cursor.getString(index);
                }
            }
        } catch (Exception ignored) {}
        if (name == null || name.isEmpty()) {
            name = uri.getLastPathSegment();
        }
        completeDocument(name == null ? -1 : 1, uri.toString(), name);
    }

    public boolean copyDocument(String uri, String path) {
        try (
            InputStream input = getContentResolver().openInputStream(
                Uri.parse(uri)
            );
            OutputStream output = new FileOutputStream(path)
        ) {
            if (input == null) {
                return false;
            }
            transfer(input, output);
            return true;
        } catch (Exception error) {
            return false;
        }
    }

    public boolean writeDocument(String uri, String path) {
        try (
            InputStream input = new FileInputStream(path);
            OutputStream output = getContentResolver().openOutputStream(
                Uri.parse(uri),
                "wt"
            )
        ) {
            if (output == null) {
                return false;
            }
            transfer(input, output);
            return true;
        } catch (Exception error) {
            return false;
        }
    }

    private static void transfer(InputStream input, OutputStream output)
        throws java.io.IOException {
        byte[] buffer = new byte[64 * 1024];
        int count;
        while ((count = input.read(buffer)) != -1) {
            output.write(buffer, 0, count);
        }
    }

    @Override
    protected void onDestroy() {
        completeDocument(0, null, null);
        super.onDestroy();
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        hideSystemBars();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
    }

    private void hideSystemBars() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController controller =
                getWindow().getInsetsController();
            controller.setSystemBarsBehavior(
                WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            );
            controller.hide(WindowInsets.Type.systemBars());
        } else {
            getWindow()
                .getDecorView()
                .setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_FULLSCREEN |
                        View.SYSTEM_UI_FLAG_HIDE_NAVIGATION |
                        View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY |
                        View.SYSTEM_UI_FLAG_LAYOUT_STABLE |
                        View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN |
                        View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                );
        }
    }
}
