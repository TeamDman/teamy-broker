package dev.teamy.broker.dummy;
import android.Manifest;
import android.app.Activity;
import android.content.Context;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;
/** Generic attention only. Every decision still goes through the authenticated live actor. */
final class ApprovalNotifications {
    private static final String CHANNEL="shared-approval", TAG="temporary-approval";
    private static final int ID=1;
    private final Context activity;
    private final NotificationManager manager;
    private String shown;
    ApprovalNotifications(Context activity){
        this.activity=activity;manager=activity.getSystemService(NotificationManager.class);
        NotificationChannel channel=new NotificationChannel(CHANNEL,"Approval requests",NotificationManager.IMPORTANCE_HIGH);
        channel.setLockscreenVisibility(Notification.VISIBILITY_SECRET);manager.createNotificationChannel(channel);
        // The service owns reconciliation; Activity lifecycle must not dismiss its request.
    }
    void enable(){if(Build.VERSION.SDK_INT>=33&&activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)((Activity)activity).requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},41);}
    void pending(String id,long remaining){
        if(id.equals(shown))return;
        clear();
        if(remaining<=0||!manager.areNotificationsEnabled()||(Build.VERSION.SDK_INT>=33&&activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED))return;
        Intent intent=new Intent(activity,BrokerActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent open=PendingIntent.getActivity(activity,0,intent,PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        Notification notification=new Notification.Builder(activity,CHANNEL).setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle("Teamy Broker approval").setContentText("Open the app to review a desktop request.")
            .setVisibility(Notification.VISIBILITY_SECRET).setContentIntent(open).setAutoCancel(true).setOnlyAlertOnce(true)
            .setTimeoutAfter(Math.min(remaining,60000)).build();
        try{manager.notify(TAG,ID,notification);shown=id;}catch(SecurityException ignored){clear();}
    }
    void clear(){manager.cancel(TAG,ID);shown=null;}
}