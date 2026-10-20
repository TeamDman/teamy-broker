package dev.teamy.broker.dummy;
import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.os.*;
import java.util.concurrent.*;
import org.json.JSONObject;
/** User-started service; every shared UI/notification mutation belongs to one generation. */
public final class PhoneService extends Service {
    static final class State {
        final PhoneService service;final String status;final Pending pending;
        State(PhoneService service,String status,Pending pending){this.service=service;this.status=status;this.pending=pending;}
    }
    private static final ServiceGeneration<State> STATE=new ServiceGeneration<>(new State(null,"Stopped",null));
    static State snapshot(){return STATE.snapshot();}
    private long generation;
    private final SocketOwner sockets=new SocketOwner();
    private volatile PhoneConnection connection;
    private volatile boolean stopped;
    private boolean actorStarted;
    private PairingForeground.Lease pairingOwner;
    private final Handler pairingUi=new Handler(Looper.getMainLooper());
    private final Runnable pairingWatch=new Runnable(){public void run(){
        if(pairingOwner==null)return;
        NotificationManager manager=getSystemService(NotificationManager.class);
        android.app.NotificationChannel channel=manager.getNotificationChannel("approval-connection");
        if(stopped||!pairingOwner.client.live(false)||!manager.areNotificationsEnabled()||channel==null||channel.getImportance()==NotificationManager.IMPORTANCE_NONE){requestStop();return;}
        pairingUi.postDelayed(this,250);
    }};
    private ExecutorService actor;
    private ApprovalNotifications notifications;
    private final ArrayBlockingQueue<Decision> decisions=new ArrayBlockingQueue<>(1);
    static final class Pending implements SupplyRequest {
        final String id;final long expires,deadline;String app,reference,purpose,destination,account;boolean supply,credential,syntheticFixture;String providerState="idle";final java.util.concurrent.atomic.AtomicBoolean submitted=new java.util.concurrent.atomic.AtomicBoolean();
        Pending(String id,long expires,long remaining){this.id=id;this.expires=expires;deadline=SystemClock.elapsedRealtime()+remaining;}
        public String id(){return id;}public boolean supplyAllowed(){return supply;}public boolean fixture(){return syntheticFixture;}public boolean submitted(){return submitted.get();}public void markSubmitted(){submitted.set(true);}
        public boolean live(){return System.currentTimeMillis()<expires&&SystemClock.elapsedRealtime()<deadline;}
    }
    private static final class Decision {final String id,value;final byte[] secret;Decision(String i,String v){this(i,v,null);}Decision(String i,String v,byte[] s){id=i;value=v;secret=s;}void clear(){if(secret!=null)java.util.Arrays.fill(secret,(byte)0);}}
    static boolean submit(String id,String decision){
        State state=STATE.snapshot();PhoneService service=state.service;
        if(service==null||!(decision.equals("approve")||decision.equals("deny")||decision.equals("cancel")))return false;
        final boolean[] queued={false};
        STATE.update(service.generation,current->{
            Pending request=current.pending;
            if(service.stopped||service.connection==null||request==null||!request.live()||!request.id.equals(id))return null;
            if(request.submitted.get()||("approve".equals(decision)&&request.credential&&!request.providerState.equals("idle")))return null;queued[0]=service.decisions.offer(new Decision(id,decision));if(queued[0]&&(!"approve".equals(decision)||!request.credential))request.submitted.set(true);return current;
        },ignored->{});
        return queued[0];
    }
    static boolean supply(String id,byte[] bytes){
        State state=STATE.snapshot();PhoneService service=state.service;
        if(service==null){if(bytes!=null)java.util.Arrays.fill(bytes,(byte)0);return false;}
        return SupplyDispatcher.enqueue(STATE,service.generation,id,bytes,new SupplyDispatcher.Adapter<State>(){
            public boolean ready(State current){return current.service==service&&!service.stopped&&service.connection!=null;}
            public SupplyRequest pending(State current){return current.pending;}
            public boolean offer(State current,String requestId,byte[] value){return service.decisions.offer(new Decision(requestId,null,value));}
        });
    }
    private void clearDecisions(){Decision d;while((d=decisions.poll())!=null)d.clear();}
    @Override public IBinder onBind(Intent intent){return null;}
    @Override public void onCreate(){
        super.onCreate();notifications=new ApprovalNotifications(this);actor=Executors.newSingleThreadExecutor();
        generation=STATE.begin(new State(this,"Stopped",null),notifications::clear);
    }
    private void status(String value){STATE.update(generation,current->stopped?null:new State(this,value,current.pending),ignored->{});}
    private void requestStop(){STATE.update(generation,current->{stopSelf();return current;},ignored->{});}
    private void clearPending(){STATE.update(generation,current->new State(this,current.status,null),ignored->notifications.clear());}
    @Override public int onStartCommand(Intent intent,int flags,int startId){
        if(intent==null||"stop".equals(intent.getAction())){requestStop();return START_NOT_STICKY;}
        if("pairing".equals(intent.getAction())){startPairing(intent);return START_NOT_STICKY;}
        if(pairingOwner!=null)return START_NOT_STICKY;
        if(!"start".equals(intent.getAction())||!PhoneIdentity.paired(this)||!getSystemService(NotificationManager.class).areNotificationsEnabled()){status("Enable notifications and pair first");requestStop();return START_NOT_STICKY;}
        if(actorStarted||stopped||!STATE.current(generation))return START_NOT_STICKY;
        try{
            foreground(false);
            if(stopped||!STATE.current(generation))return START_NOT_STICKY;
            actorStarted=true;actor.execute(this::run);
        }catch(Exception ignored){status("Foreground service unavailable; open app and check permissions");requestStop();}
        return START_NOT_STICKY;
    }
    private void foreground(boolean pairing)throws Exception {
            String channel="approval-connection";NotificationManager manager=getSystemService(NotificationManager.class);
            manager.createNotificationChannel(new NotificationChannel(channel,"Approval connection",NotificationManager.IMPORTANCE_LOW));
            if(manager.getNotificationChannel(channel).getImportance()==NotificationManager.IMPORTANCE_NONE)throw new Exception("connection channel blocked");
            PendingIntent open=PendingIntent.getActivity(this,3,new Intent(this,BrokerActivity.class),PendingIntent.FLAG_IMMUTABLE|PendingIntent.FLAG_UPDATE_CURRENT);
            PendingIntent stop=PendingIntent.getService(this,4,new Intent(this,PhoneService.class).setAction("stop"),PendingIntent.FLAG_IMMUTABLE|PendingIntent.FLAG_UPDATE_CURRENT);
            Notification notification=new Notification.Builder(this,channel).setSmallIcon(android.R.drawable.ic_lock_lock).setContentTitle(pairing?"Teamy pairing connection":"Teamy approval connection").setContentText(pairing?"Temporary comparison · stops on cancel or expiry":"Tailscale · exact requests require permission").setContentIntent(open).setOngoing(true).setVisibility(Notification.VISIBILITY_SECRET).addAction(new Notification.Action.Builder(null,"Stop",stop).build()).build();
            STATE.update(generation,current->{
                if(stopped)return null;
                startForeground(2,notification,ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
                return new State(this,pairing?"Pairing connection":"Connecting",null);
            },ignored->{});
    }
    private void startPairing(Intent intent){
        if(actorStarted||pairingOwner!=null||stopped||!STATE.current(generation))return;
        PairingForeground.Lease lease=PairingForeground.claim(intent.getStringExtra("lease"));
        if(lease==null){requestStop();return;}
        pairingOwner=lease;
        try{
            if(!getSystemService(NotificationManager.class).areNotificationsEnabled())throw new Exception("notifications unavailable");
            foreground(true);
            if(!PairingForeground.started(lease,this)){requestStop();return;}
            pairingUi.post(pairingWatch);
        }catch(Exception ignored){PairingForeground.destroyed(lease);pairingOwner=null;requestStop();}
    }
    private static String metadata(JSONObject request,String field)throws Exception{String value=request.getString(field);if(value.length()==0||value.length()>1024)throw new Exception("metadata rejected");for(int i=0;i<value.length();i++)if(Character.isISOControl(value.charAt(i)))throw new Exception("metadata rejected");return value;}
    private void publish(JSONObject reply)throws Exception {
        final Pending value;
        if(reply.isNull("pending")){value=null;}
        else{
            JSONObject record=reply.getJSONObject("pending");String id=record.getString("request_id");java.util.UUID.fromString(id);
            long remaining=record.getLong("expires_unix_ms")-System.currentTimeMillis();
            boolean credential=record.getString("kind").equals("credential-use-v1");
            if(remaining<=0||remaining>60000||(!credential&&(!record.getString("kind").equals("no-vault-preview-v1")||record.length()!=3))||(credential&&record.length()!=4))throw new Exception("approval metadata rejected");
            value=new Pending(id,record.getLong("expires_unix_ms"),remaining);value.credential=credential;
            if(credential){JSONObject context=record.getJSONObject("credential"),request=context.getJSONObject("request");value.syntheticFixture=context.optBoolean("synthetic_fixture",false);value.supply=context.getBoolean("supply");value.providerState=context.getString("provider_state");if(!(value.providerState.equals("idle")||value.providerState.equals("waiting")||value.providerState.equals("failed")))throw new Exception("provider state rejected");value.app=metadata(request,"requesting_app");value.reference=metadata(request,"secret_ref");value.purpose=metadata(request,"purpose");value.destination=metadata(request,"destination");value.account=metadata(request,"account");if(!value.reference.startsWith("op://"))throw new Exception("reference rejected");if(!CredentialReleaseIdentity.allowed(this,context.optBoolean("synthetic_fixture",false)))throw new Exception("A verified release app is required for this action");}
        }
        final boolean[] invalid={false};
        STATE.update(generation,current->{
            if(stopped)return null;
            Pending prior=current.pending;
            if(prior!=null&&value!=null&&prior.id.equals(value.id)){
                if(prior.expires!=value.expires||prior.credential!=value.credential||prior.supply!=value.supply||prior.syntheticFixture!=value.syntheticFixture||!java.util.Objects.equals(prior.app,value.app)||!java.util.Objects.equals(prior.reference,value.reference)||!java.util.Objects.equals(prior.purpose,value.purpose)||!java.util.Objects.equals(prior.destination,value.destination)||!java.util.Objects.equals(prior.account,value.account))invalid[0]=true;
                prior.providerState=value.providerState;
                if(reply.has("accepted")&&!reply.isNull("accepted")&&!reply.optBoolean("accepted")){prior.submitted.set(false);}
                return null; // Existing deadline never renews on repeated polling.
            }
            return new State(this,current.status,value);
        },current->{if(current.pending==null)notifications.clear();else notifications.pending(current.pending.id,current.pending.deadline-SystemClock.elapsedRealtime());});
        if(invalid[0])throw new Exception("preview changed");
    }
    private void run(){
        int retry=2;
        while(!stopped&&STATE.current(generation)){
            PhoneConnection owned=null;
            try{
                owned=new PhoneConnection(this,sockets);connection=owned;
                if(stopped||!STATE.current(generation)){owned.close();break;}
                status("Connected · paired desktop");publish(owned.hello);retry=2;
                while(!stopped&&STATE.current(generation)){
                    NotificationManager manager=getSystemService(NotificationManager.class);
                    if(!manager.areNotificationsEnabled()||manager.getNotificationChannel("approval-connection").getImportance()==NotificationManager.IMPORTANCE_NONE){status("Notifications disabled; connection stopped");requestStop();return;}
                    Decision decision=decisions.poll(PhoneTiming.POLL_MS,TimeUnit.MILLISECONDS);State state=STATE.snapshot();
                    if(state.service==this&&state.pending!=null&&!state.pending.live())clearPending();
                    try{publish(owned.exchange(decision==null?null:decision.id,decision==null?null:decision.value,decision==null?null:decision.secret));}finally{if(decision!=null)decision.clear();}
                }
            }catch(Exception error){if(!stopped)status("Disconnected · retrying over Tailscale"+(error instanceof ConnectionFailure?" · "+((ConnectionFailure)error).phase.name()+"/"+((ConnectionFailure)error).category.name():" · UNAVAILABLE"));}
            finally{
                if(owned!=null)owned.close();if(connection==owned)connection=null;
                clearDecisions();clearPending(); // Generation gate ignores late old-actor cleanup.
            }
            if(!stopped&&STATE.current(generation))try{Thread.sleep(retry*1000L);retry=Math.min(30,retry*2);}catch(InterruptedException ignored){break;}
        }
    }
    @Override public void onDestroy(){
        stopped=true;pairingUi.removeCallbacks(pairingWatch);PairingForeground.Lease pairing=pairingOwner;pairingOwner=null;PairingForeground.destroyed(pairing);sockets.stop();PhoneConnection owned=connection;connection=null;if(owned!=null)owned.close();
        clearDecisions();if(actor!=null)actor.shutdownNow();
        STATE.end(generation,new State(null,"Stopped",null),()->{notifications.clear();stopForeground(STOP_FOREGROUND_REMOVE);});
        super.onDestroy();
    }
}