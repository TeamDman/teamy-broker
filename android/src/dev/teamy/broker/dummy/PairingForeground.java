package dev.teamy.broker.dummy;
import android.app.Activity;
import android.app.NotificationManager;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.*;
/** Main-thread handoff to the existing non-sticky connected-device service. No secret in an Intent. */
final class PairingForeground {
 static final class Lease {
  final String token=java.util.UUID.randomUUID().toString();final PairingClient client;final Runnable ready,failed;PhoneService service;
  Lease(PairingClient client,Runnable ready,Runnable failed){this.client=client;this.ready=ready;this.failed=failed;}
 }
 private static final Handler MAIN=new Handler(Looper.getMainLooper());
 private static Lease current;
 static void start(Activity activity,PairingClient client,Runnable ready,Runnable failed){
  if(Looper.myLooper()!=Looper.getMainLooper())throw new IllegalStateException("main thread required");
  NotificationManager manager=activity.getSystemService(NotificationManager.class);
  if(current!=null||PhoneService.snapshot().service!=null||!manager.areNotificationsEnabled()||(Build.VERSION.SDK_INT>=33&&activity.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)){client.close();failed.run();return;}
  Lease lease=new Lease(client,ready,failed);current=lease;
  try{activity.startForegroundService(new Intent(activity,PhoneService.class).setAction("pairing").putExtra("lease",lease.token));}
  catch(RuntimeException ignored){current=null;client.close();failed.run();return;}
  MAIN.postDelayed(()->{if(current==lease&&lease.service==null){finish(client);failed.run();}},3000);
 }
 static Lease claim(String token){return current!=null&&current.service==null&&current.token.equals(token)&&current.client.live(false)?current:null;}
 static boolean started(Lease lease,PhoneService service){
  if(current!=lease||!lease.client.live(false))return false;
  lease.service=service;lease.ready.run();return current==lease&&lease.client.live(false);
 }
 static void finish(PairingClient client){
  if(Looper.myLooper()!=Looper.getMainLooper()){MAIN.post(()->finish(client));return;}
  Lease lease=current;if(lease==null||lease.client!=client)return;
  current=null;client.close();if(lease.service!=null)lease.service.stopSelf();
 }
 static void destroyed(Lease lease){
  if(lease==null)return;lease.client.close();if(current==lease){current=null;lease.failed.run();}
 }
 static boolean active(){return current!=null&&current.service!=null&&current.client.live(false);}
}
