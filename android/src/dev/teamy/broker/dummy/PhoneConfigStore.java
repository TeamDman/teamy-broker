package dev.teamy.broker.dummy;

import android.content.ContentValues;
import android.content.Context;
import android.content.SharedPreferences;
import android.database.Cursor;
import android.database.sqlite.SQLiteDatabase;

/** Whole profiles are read from committed SQLite rows, without a staging cache. */
final class PhoneConfigStore implements DurablePhoneConfig.Store {
    private final Context context;

    private PhoneConfigStore(Context context) {
        this.context = context.getApplicationContext();
    }

    static synchronized PrivatePhoneConfig load(Context context) throws Exception {
        return new DurablePhoneConfig(new PhoneConfigStore(context)).load();
    }

    static synchronized void save(Context context, PrivatePhoneConfig profile) throws Exception {
        boolean paired = PhoneIdentity.storedIdentityPaired(context);
        new DurablePhoneConfig(new PhoneConfigStore(context)).save(profile, paired);
    }

    private SQLiteDatabase open() {
        SQLiteDatabase db = context.openOrCreateDatabase("phone-runtime-profile-v1.db", Context.MODE_PRIVATE, null);
        try {
            db.execSQL("PRAGMA synchronous=FULL");
            db.execSQL("CREATE TABLE IF NOT EXISTS profile (_id INTEGER PRIMARY KEY CHECK (_id=1), desktop TEXT NOT NULL, phone TEXT NOT NULL, desktop_node TEXT NOT NULL, phone_node TEXT NOT NULL)");
            return db;
        } catch (RuntimeException failure) {
            db.close();
            throw failure;
        }
    }

    @Override
    public PrivatePhoneConfig read() throws Exception {
        try (SQLiteDatabase db = open(); Cursor row = db.rawQuery("SELECT desktop,phone,desktop_node,phone_node FROM profile WHERE _id=1", null)) {
            if (row.moveToFirst()) {
                return new PrivatePhoneConfig(row.getString(0), row.getString(1), row.getString(2), row.getString(3));
            }
        }
        // Read-only migration retains preferences if the first database write fails.
        SharedPreferences legacy = context.getSharedPreferences("phone-runtime-profile-v1", Context.MODE_PRIVATE);
        java.util.Map<String, ?> values = legacy.getAll();
        if (values.isEmpty()) return null;
        return new PrivatePhoneConfig((String) values.get("desktop"), (String) values.get("phone"),
                (String) values.get("desktop_node"), (String) values.get("phone_node"));
    }

    @Override
    public void commit(PrivatePhoneConfig profile) {
        try (SQLiteDatabase db = open()) {
            db.beginTransaction();
            try {
                ContentValues values = new ContentValues();
                values.put("_id", 1);
                values.put("desktop", profile.desktop);
                values.put("phone", profile.phone);
                values.put("desktop_node", profile.desktopNode);
                values.put("phone_node", profile.phoneNode);
                if (db.insertWithOnConflict("profile", null, values, SQLiteDatabase.CONFLICT_REPLACE) == -1) {
                    throw new IllegalStateException("phone configuration publication failed");
                }
                db.setTransactionSuccessful();
            } finally {
                db.endTransaction();
            }
        }
    }
}
