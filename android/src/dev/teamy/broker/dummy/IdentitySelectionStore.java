package dev.teamy.broker.dummy;
import android.content.Context;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.sqlite.SQLiteDatabase;
/** SQLite FULL transactions publish the whole public alias/pin record durably. */
final class IdentitySelectionStore implements DurableIdentitySelection.Store {
    private final Context context;
    IdentitySelectionStore(Context context){this.context=context.getApplicationContext();}
    private SQLiteDatabase open(){
        SQLiteDatabase db=context.openOrCreateDatabase("phone-identity-v2.db",Context.MODE_PRIVATE,null);
        try{db.execSQL("PRAGMA synchronous=FULL");db.execSQL("CREATE TABLE IF NOT EXISTS identity (_id INTEGER PRIMARY KEY CHECK (_id=1), alias TEXT, pin TEXT, previous_alias TEXT, previous_pin TEXT)");return db;}
        catch(RuntimeException failure){db.close();throw failure;}
    }
    public DurableIdentitySelection.Selection read(){
        try(SQLiteDatabase db=open();Cursor row=db.rawQuery("SELECT alias,pin,previous_alias,previous_pin FROM identity WHERE _id=1",null)){
            if(row.moveToFirst())return new DurableIdentitySelection.Selection(row.getString(0),row.getString(1),row.getString(2),row.getString(3));
        }
        // Read-only migration: a failed first transaction cannot mutate the old preferences.
        android.content.SharedPreferences old=context.getSharedPreferences("phone-identity-v1",Context.MODE_PRIVATE);
        String pin=old.getString("desktop_certificate",null);
        return new DurableIdentitySelection.Selection(pin==null?null:old.getString("active_alias",PhoneKeyPolicy.LEGACY),pin,old.getString("previous_alias",null),old.getString("previous_desktop_certificate",null));
    }
    public void commit(DurableIdentitySelection.Selection next){
        try(SQLiteDatabase db=open()){
            db.beginTransaction();
            try{ContentValues values=new ContentValues();values.put("_id",1);values.put("alias",next.alias);values.put("pin",next.pin);values.put("previous_alias",next.previousAlias);values.put("previous_pin",next.previousPin);
                if(db.insertWithOnConflict("identity",null,values,SQLiteDatabase.CONFLICT_REPLACE)==-1)throw new IllegalStateException("identity publication failed");
                db.setTransactionSuccessful();
            }finally{db.endTransaction();}
        }
    }
}