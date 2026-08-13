package app.span.android;

import android.content.Intent;
import android.service.quicksettings.TileService;

public final class SpanTileService extends TileService {
    @Override public void onClick() {
        super.onClick();
        Intent intent = new Intent(this, SendClipboardActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        startActivityAndCollapse(intent);
    }
}
