package dev.teamy.broker.dummy;
import android.app.Activity;
import android.os.*;
import android.view.WindowManager;
import android.widget.*;
import android.content.Intent;
import java.util.concurrent.*;
/** Native request decisions/input and separately permissioned persistent relationship management. */
public final class BrokerActivity extends Activity {
    private final ExecutorService actor=Executors.newSingleThreadExecutor();
    private final Handler ui=new Handler(Looper.getMainLooper());
    private TextView comparison,status,request;
    private Button pair,replace,match,reject,start,approve,deny,cancel,send,autofill;
    private KeyboardViewport viewport;
    private EditText input;private String inputRequest;private LinearLayout approvalCard,settings;
    private volatile PairingClient pairing;
    private boolean foreground,busy;
    private int generation;
    private String selected;
    private ApprovalNotifications notifications;
    private String lastPairingTiming="Pairing timing: unavailable";
    private long creationCount;private volatile PairingDiagnostics.Stage pairingStage=PairingDiagnostics.Stage.NONE;
    private volatile PairingDiagnostics.Close lastPairingClose=PairingDiagnostics.Close.NONE;
    private void stage(PairingDiagnostics.Stage value){pairingStage=value;getSharedPreferences("safe_pairing_diagnostics",MODE_PRIVATE).edit().putBoolean("active",value!=PairingDiagnostics.Stage.NONE&&value!=PairingDiagnostics.Stage.CLOSED).apply();}
    private void closeReason(PairingDiagnostics.Close reason){lastPairingClose=reason;stage(PairingDiagnostics.Stage.CLOSED);getSharedPreferences("safe_pairing_diagnostics",MODE_PRIVATE).edit().putString("close",reason.name()).apply();}
    private void lifecycle(){android.util.Log.i("BrokerLifecycle",PairingDiagnostics.format(creationCount,pairingStage,lastPairingClose).replace('\n',' '));}

    private final Runnable refresh=new Runnable(){public void run(){render();if(foreground)ui.postDelayed(this,500);}};
    @Override public void onCreate(Bundle ignored){super.onCreate(null);getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);notifications=new ApprovalNotifications(this);
        android.content.SharedPreferences diagnosticPrefs=getSharedPreferences("safe_pairing_diagnostics",MODE_PRIVATE);
        long prior=diagnosticPrefs.getLong("creations",0);creationCount=prior==Long.MAX_VALUE?prior:Math.max(0,prior)+1;
        lastPairingClose=diagnosticPrefs.getBoolean("active",false)?PairingDiagnostics.Close.PROCESS_RECREATED:PairingDiagnostics.parseClose(diagnosticPrefs.getString("close","NONE"));
        diagnosticPrefs.edit().putLong("creations",creationCount).putBoolean("active",false).putString("close",lastPairingClose.name()).apply();lifecycle();
        ScrollView scroll=new ScrollView(this);LinearLayout layout=new LinearLayout(this);layout.setOrientation(LinearLayout.VERTICAL);int pad=(int)(24*getResources().getDisplayMetrics().density);layout.setPadding(pad,pad,pad,pad);scroll.addView(layout);viewport=KeyboardViewport.install(this,scroll);
        layout.setBackgroundColor(android.graphics.Color.rgb(242,245,250));
        text(layout,"TEAMY",14).setTextColor(android.graphics.Color.rgb(42,91,175));
        text(layout,"Desktop approvals",30).setTypeface(null,android.graphics.Typeface.BOLD);
        android.content.pm.PackageInfo installed;
        try{installed=getPackageManager().getPackageInfo(getPackageName(),0);}catch(android.content.pm.PackageManager.NameNotFoundException impossible){throw new IllegalStateException(impossible);}
        final String appVersion=installed.versionName;final long appCode=installed.getLongVersionCode();
        final boolean privateRelease="dev.teamy.broker.release".equals(getPackageName())&&(installed.applicationInfo.flags&android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE)==0;
        text(layout,"Version "+appVersion+" · build "+appCode+(privateRelease?" · PRIVATE RELEASE":" · NO VAULT"),15);
        button(layout,"Copy debug info",()->{
            String report=SafeDebugInfo.format(appVersion,appCode,PhoneService.snapshot().status,privateRelease)+"\n"+PairingDiagnostics.format(creationCount,pairingStage,lastPairingClose)+"\n"+(pairing==null?lastPairingTiming:pairing.safeTiming());
            android.content.ClipboardManager clipboard=getSystemService(android.content.ClipboardManager.class);
            android.content.ClipData clip=android.content.ClipData.newPlainText("Teamy Broker safe debug info",report);
            android.os.PersistableBundle extras=new android.os.PersistableBundle();extras.putBoolean("android.content.extra.IS_SENSITIVE",true);clip.getDescription().setExtras(extras);
            clipboard.setPrimaryClip(clip);android.widget.Toast.makeText(this,"Safe debug info copied",android.widget.Toast.LENGTH_SHORT).show();
        });
        status=text(layout,"Checking paired desktop…",16);
        approvalCard=card(layout);text(approvalCard,"Current request",20).setTypeface(null,android.graphics.Typeface.BOLD);
        request=text(approvalCard,"No pending request.",17);
        input=new EditText(this);input.setHint("Credential value · stays on this request");input.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);input.setAutofillHints(android.view.View.AUTOFILL_HINT_PASSWORD);input.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_YES);input.setSaveEnabled(false);input.setSingleLine(true);input.setTransformationMethod(android.text.method.PasswordTransformationMethod.getInstance());input.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(4096)});approvalCard.addView(input);viewport.watch(input);
        autofill=button(approvalCard,"Choose with password manager",()->getSystemService(android.view.autofill.AutofillManager.class).requestAutofill(input));
        send=button(approvalCard,"Send to paired desktop",()->send());
        approve=button(approvalCard,"Approve desktop read",()->submit("approve"));deny=button(approvalCard,"Deny",()->submit("deny"));cancel=button(approvalCard,"Cancel request",()->submit("cancel"));
        text(approvalCard,"Phone supply is your explicit action. Autofill may support login fields; search your item manually if unavailable. No automatic op:// lookup. A 15-minute broker lease does not expire the underlying token.",13);
        Button settingsToggle=button(layout,"Connection & identity settings",()->settings.setVisibility(settings.getVisibility()==android.view.View.VISIBLE?android.view.View.GONE:android.view.View.VISIBLE));
        settings=card(layout);settings.setVisibility(android.view.View.GONE);layout=settings;
        text(layout,"Pair once with this desktop over Tailscale. Compare all four groups and Match on BOTH screens. Saved pairing defaults to harmless previews. Credential actions require one exact, separately enabled request permission. Background connection uses an ongoing service notification. Android sleep/OEM restrictions may delay delivery.",16);
        text(layout,PrivatePhoneConfig.CONFIGURED ? "Desktop "+PrivatePhoneConfig.DESKTOP+":"+PrivatePhoneConfig.PORT+" · "+PrivatePhoneConfig.DESKTOP_NODE+"\nPhone "+PrivatePhoneConfig.PHONE+" · "+PrivatePhoneConfig.PHONE_NODE : "Private phone configuration is missing from this build.",14);
        pair=button(layout,"Pair phone · run desktop phone-pair first",()->pair(false));
        replace=button(layout,"Replace phone identity · desktop replacement window",()->pair(true));
        text(layout,"Replacement creates a TLS-compatible hardware key after your explicit tap. Compare and Match again. Current key and pairing remain saved until authenticated enrollment completes; the previous identity is retained for coordinated recovery.",14);
        comparison=text(layout,"No comparison active.",22);
        comparison.setOnClickListener(view->{
            PairingClient channel=pairing;
            String code=channel==null?null:PairingContinuation.copyValue(channel.comparison,foreground,channel.live(false),busy);
            if(code==null)return;
            android.content.ClipData clip=android.content.ClipData.newPlainText("Teamy Broker pairing comparison",code);
            android.os.PersistableBundle extras=new android.os.PersistableBundle();extras.putBoolean("android.content.extra.IS_SENSITIVE",true);clip.getDescription().setExtras(extras);
            getSystemService(android.content.ClipboardManager.class).setPrimaryClip(clip);
            android.widget.Toast.makeText(this,"Copied",android.widget.Toast.LENGTH_SHORT).show();
        });

        match=button(layout,"Match · all four groups agree",()->match());reject=button(layout,"Reject pairing",()->{closeReason(PairingDiagnostics.Close.EXPLICIT_REJECT);endPair();});
        button(layout,"Enable notifications",()->notifications.enable());
        start=button(layout,"Start approval connection",()->{
            android.app.NotificationManager manager=getSystemService(android.app.NotificationManager.class);
            if(!manager.areNotificationsEnabled()||(Build.VERSION.SDK_INT>=33&&checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)!=android.content.pm.PackageManager.PERMISSION_GRANTED)){notifications.enable();status.setText("Enable notifications, then tap Start again.");return;}
            android.app.NotificationChannel channel=manager.getNotificationChannel("approval-connection");
            if(channel!=null&&channel.getImportance()==android.app.NotificationManager.IMPORTANCE_NONE){status.setText("Approval connection channel blocked; enable it in Android notification settings.");return;}
            try{startForegroundService(new Intent(this,PhoneService.class).setAction("start"));}catch(Exception e){status.setText("Open this screen and enable notifications before starting.");}});
        button(layout,"Stop approval connection",()->stopService(new Intent(this,PhoneService.class)));
        button(layout,"Forget phone identity",()->{endPair();stopService(new Intent(this,PhoneService.class));actor.execute(()->{try{PhoneIdentity.forget(this);runOnUiThread(()->status.setText("Phone identity forgotten. Also run desktop phone-revoke."));}catch(Exception e){runOnUiThread(()->status.setText("Identity deletion failed."));}});});
        render();
    }
    private LinearLayout card(LinearLayout parent){LinearLayout view=new LinearLayout(this);view.setOrientation(LinearLayout.VERTICAL);int p=(int)(18*getResources().getDisplayMetrics().density);view.setPadding(p,p,p,p);android.graphics.drawable.GradientDrawable background=new android.graphics.drawable.GradientDrawable();background.setColor(android.graphics.Color.WHITE);background.setCornerRadius(p);view.setBackground(background);LinearLayout.LayoutParams params=new LinearLayout.LayoutParams(-1,-2);params.setMargins(0,p,0,p);parent.addView(view,params);return view;}
    private void clearInput(){if(input!=null)input.getText().clear();}
    private void send(){String id=selected;if(id==null)return;try(ApprovalInput value=new ApprovalInput(input.getText())){if(!CredentialInputHandoff.send(value,this::clearInput,()->getSystemService(android.view.autofill.AutofillManager.class).cancel(),bytes->PhoneService.supply(id,bytes)))status.setText("Request unavailable; value cleared.");}catch(Exception ignored){clearInput();status.setText("Unable to send. Value cleared; re-enter only while this request is live.");}}
    private TextView text(LinearLayout layout,String value,int size){TextView view=new TextView(this);view.setText(value);view.setTextSize(size);view.setTextColor(android.graphics.Color.rgb(24,36,55));view.setPadding(0,12,0,12);view.setSaveEnabled(false);layout.addView(view);return view;}
    private Button button(LinearLayout layout,String value,Runnable action){Button button=new Button(this);button.setText(value);button.setMinHeight((int)(48*getResources().getDisplayMetrics().density));button.setAllCaps(false);button.setOnClickListener(v->action.run());button.setSaveEnabled(false);layout.addView(button);return button;}
    private void render(){
        if(pairing!=null&&!pairing.live(false)){closeReason(pairing.closeReason()==PairingDiagnostics.Close.NONE?PairingDiagnostics.Close.DEADLINE:pairing.closeReason());endPair();return;}
        PhoneService.State serviceState=PhoneService.snapshot();PhoneService.Pending pending=serviceState.pending;boolean available=pending!=null&&pending.live()&&PhoneService.snapshot().service!=null;
        selected=available?pending.id:null;if(!java.util.Objects.equals(inputRequest,selected)){clearInput();inputRequest=selected;}
        boolean credential=available&&pending.credential,supply=credential&&pending.supply,waiting=available&&pending.submitted.get();
        long seconds=available?Math.max(0,(pending.deadline-SystemClock.elapsedRealtime()+999)/1000):0;
        request.setText(!available?"You're up to date. New desktop requests appear here.":(waiting?"Sending decision…\n":seconds+" seconds remaining\n")+(credential?"App (claimed): "+pending.app+"\nReference: "+pending.reference+"\nPurpose: "+pending.purpose+"\nDestination: "+pending.destination+"\nAccount: "+pending.account+"\n"+(pending.syntheticFixture?"ISOLATED TEST: no vault read. Enter only teamy-fixture-input-v1 or choose Use fixture provider. Provider: "+pending.providerState:supply?"Supply this exact value from your phone, or use desktop 1Password. Provider: "+pending.providerState:"Use desktop 1Password. This does not unlock mobile 1Password. Provider: "+pending.providerState):"Harmless preview · no vault or lease"));
        input.setImportantForAutofill(credential&&pending.syntheticFixture?android.view.View.IMPORTANT_FOR_AUTOFILL_NO:android.view.View.IMPORTANT_FOR_AUTOFILL_YES);
        input.setVisibility(supply&&!waiting?android.view.View.VISIBLE:android.view.View.GONE);autofill.setVisibility(supply&&!waiting&&!pending.syntheticFixture?android.view.View.VISIBLE:android.view.View.GONE);send.setVisibility(supply?android.view.View.VISIBLE:android.view.View.GONE);send.setEnabled(supply&&!waiting);
        approve.setVisibility(android.view.View.VISIBLE);approve.setText(credential?(pending.syntheticFixture?"Use fixture provider":"Use desktop 1Password"):"Approve harmless preview");approve.setEnabled(available&&!waiting&&(!credential||pending.providerState.equals("idle")));deny.setEnabled(available&&!waiting);cancel.setEnabled(available&&!waiting);
        if(waiting||!available)clearInput();
        pair.setEnabled(!busy&&pairing==null&&!PhoneIdentity.paired(this)&&PhoneService.snapshot().service==null);replace.setEnabled(!busy&&pairing==null&&PhoneIdentity.paired(this)&&PhoneService.snapshot().service==null);match.setEnabled(!busy&&pairing!=null);reject.setEnabled(pairing!=null);start.setEnabled(!busy&&pairing==null&&PhoneIdentity.paired(this)&&PhoneService.snapshot().service==null);
        if(!busy&&pairing==null)status.setText((PhoneIdentity.paired(this)?"Paired · ":"Unpaired · ")+PhoneService.snapshot().status);
    }
    private void submit(String decision){String id=selected;if(id==null||!PhoneService.submit(id,decision))status.setText("Request unavailable; wait for authoritative state.");}
    private void pair(boolean replacement){if(busy||pairing!=null||PhoneService.snapshot().service!=null)return;final String alias=PhoneIdentity.TLS_ALIAS;PairingClient channel=new PairingClient(true,alias);channel.foreground(foreground);pairing=channel;busy=true;stage(PairingDiagnostics.Stage.CONNECTING);int epoch=generation;comparison.setText("Connecting for persistent no-vault enrollment.");render();
        PairingForeground.start(this,channel,()->actor.execute(()->{try{ConnectionFailure.at(ConnectionFailure.Phase.PHONE_KEY,()->PhoneIdentity.certificate(alias));channel.connect();runOnUiThread(()->{if(foreground&&epoch==generation){busy=false;stage(PairingDiagnostics.Stage.COMPARISON);comparison.setText("PERSISTENT NO-VAULT PAIRING\n"+channel.comparison+"\nCompare both screens before Match.");render();}});}catch(Exception e){final String failure=e instanceof ConnectionFailure?"Hardware TLS key self-test unavailable · "+((ConnectionFailure)e).phase.name()+"/"+((ConnectionFailure)e).category.name()+". Saved identity retained.":"Pairing unavailable. Check desktop phone-status and arm a fresh window.";runOnUiThread(()->{if(epoch==generation){closeReason(channel.closeReason());endPair();comparison.setText(failure);}});}}),()->{if(pairing==channel&&epoch==generation){closeReason(channel.closeReason());endPair();comparison.setText("Enable notifications, then pair again. Temporary pairing uses the approval connection notification.");}});
    }
    private void match(){PairingClient channel=pairing;if(channel==null||busy)return;busy=true;stage(PairingDiagnostics.Stage.CONFIRMING);int epoch=generation;render();actor.execute(()->{try{channel.match();stage(PairingDiagnostics.Stage.ENROLLING);channel.enroll(this);runOnUiThread(()->{if(foreground&&epoch==generation){closeReason(PairingDiagnostics.Close.ENROLLED);endPair();comparison.setText("Persistent no-vault relationship saved. Enable notifications, then Start approval connection.");}});}catch(Exception e){runOnUiThread(()->{if(epoch==generation){closeReason(channel.closeReason());endPair();comparison.setText("Enrollment incomplete. Inspect desktop status; revoke and re-pair if needed.");}});}finally{channel.destroyEngine();}});}
    private void endPair(){generation++;PairingClient channel=pairing;pairing=null;if(channel!=null){PairingForeground.finish(channel);channel.close();lastPairingTiming=channel.safeTiming();actor.execute(channel::destroyEngine);}busy=false;comparison.setText("No comparison active.");render();}
    @Override protected void onResume(){super.onResume();foreground=true;if(pairing!=null)pairing.foreground(true);viewport.resume();lifecycle();ui.post(refresh);}
    @Override protected void onPause(){foreground=false;if(pairing!=null)pairing.foreground(false);viewport.pause();clearInput();if(input!=null)try{getSystemService(android.view.autofill.AutofillManager.class).cancel();}catch(RuntimeException ignored){/* Input is already cleared; finish lifecycle cleanup. */}ui.removeCallbacks(refresh);if(pairing!=null&&!PairingContinuation.retainComparison(pairing.live(false),pairing.comparison!=null,busy)){closeReason(pairingStage==PairingDiagnostics.Stage.CONFIRMING||pairingStage==PairingDiagnostics.Stage.ENROLLING?PairingDiagnostics.Close.PAUSE_CONFIRMING:PairingDiagnostics.Close.PAUSE_UNREADY);endPair();}lifecycle();super.onPause();}
    @Override protected void onDestroy(){ui.removeCallbacks(refresh);if(pairing!=null){closeReason(PairingDiagnostics.Close.ACTIVITY_DESTROYED);endPair();}lifecycle();actor.shutdown();super.onDestroy();}
    @Override public void onSaveInstanceState(Bundle out){/* No comparison/request/decision restoration. */}
}
