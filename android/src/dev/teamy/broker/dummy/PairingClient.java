package dev.teamy.broker.dummy;

import dev.teamy.broker.pairing.Frames;
import dev.teamy.broker.pairing.VerifiedPairing;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;

/** One foreground socket: official UKEY2, both Match actions, then bound dummy packets. */
final class PairingClient implements AutoCloseable {
    private final boolean persistent;
    private final PairingTiming timing=new PairingTiming();
    void foreground(boolean active){timing.foreground=active?PairingTiming.Foreground.FOREGROUND:PairingTiming.Foreground.BACKGROUND;}
    String safeTiming(){return timing.format(now(),reason);}
    PairingClient(){this(false);}
    PairingClient(boolean persistent){this(persistent,PhoneIdentity.TLS_ALIAS);}
    PairingClient(boolean persistent,String alias){this.persistent=persistent;this.enrollmentAlias=alias;}
    private final String enrollmentAlias;
    private final EnrollmentPublication publication=new EnrollmentPublication();
    private final Socket socket = new Socket();
    private final ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor();
    private volatile VerifiedPairing pairing;
    private volatile byte[] key;
    private volatile boolean closed;
    private volatile PairingDiagnostics.Close reason=PairingDiagnostics.Close.NONE;
    PairingDiagnostics.Close closeReason(){return reason;}
    private volatile long pairUntil = now()+90000, sessionUntil = now()+180000;
    private ScheduledFuture<?> expiry;
    String comparison;
    String desktop;
    private static final byte[] READY = "teamy-broker/dummy/ready/v2".getBytes(StandardCharsets.UTF_8);
    private static long now() { return System.nanoTime() / 1000000L; }
    private ScheduledFuture<?> limit(long until) throws Exception {
        if (closed || until <= now()) throw new Exception("session unavailable");
        return timer.schedule(()->close(PairingDiagnostics.Close.DEADLINE), until-now(), TimeUnit.MILLISECONDS);
    }
    void connect() throws Exception {
        long initial = now(); timing.start=initial;pairUntil = initial + (persistent?300000:90000); sessionUntil = initial + (persistent?300000:180000);
        expiry = limit(pairUntil);
        ScheduledFuture<?> operation = limit(initial + 10000);
        byte[] binding = null, frame = null;
        try {
            PrivatePhoneConfig.requireConfigured();
            socket.connect(new InetSocketAddress(PrivatePhoneConfig.DESKTOP, PrivatePhoneConfig.PORT), 3000);
            socket.setSoTimeout(10000);
            binding = Frames.read(socket.getInputStream());
            JSONObject record = new JSONObject(new String(binding, StandardCharsets.UTF_8));
            long wall = System.currentTimeMillis();
            PairingBudget budget=PairingBudget.map(persistent,wall,now(),pairUntil,sessionUntil,record.getLong("pairing_expires_unix_ms"),record.getLong("session_expires_unix_ms"));
            timing.bindingBudget=budget.bindingPairMs;
            if (record.getInt("version") != 2 || !record.getString("desktop").equals(DummyPolicy.DESKTOP)
                || !record.getString("phone").equals(DummyPolicy.PHONE)
                || !record.getString("desktop_node").equals(PrivatePhoneConfig.DESKTOP_NODE)
                || !record.getString("phone_node").equals(PrivatePhoneConfig.PHONE_NODE)
                || !record.getString("endpoint").equals(PrivatePhoneConfig.DESKTOP+":"+PrivatePhoneConfig.PORT)
                || !record.getString("scope").equals(persistent?PhoneIdentity.SCOPE:"dummy-only/session-only/ukey2-p256")
                || !record.getString("session_id").matches("[0-9a-f]{32}")
                || !record.getString("nonce").matches("[0-9a-f]{64}"))
                throw new Exception("pairing context rejected");
            pairUntil=budget.pairUntil;sessionUntil=budget.sessionUntil;
            expiry.cancel(false); expiry = limit(pairUntil);
            long sample=now();
            pairing = persistent ? VerifiedPairing.persistentPreview(VerifiedPairing.Role.PHONE,binding,PairingClient::now,pairUntil-sample,sessionUntil-sample)
                : new VerifiedPairing(VerifiedPairing.Role.PHONE,binding,PairingClient::now,pairUntil-sample,sessionUntil-sample);
            frame = pairing.nextHandshake(); Frames.write(socket.getOutputStream(), frame); Arrays.fill(frame,(byte)0);
            frame = Frames.read(socket.getInputStream()); pairing.acceptHandshake(frame); Arrays.fill(frame,(byte)0);
            frame = pairing.nextHandshake(); Frames.write(socket.getOutputStream(), frame);
            comparison = pairing.authenticationCode(); desktop = record.getString("desktop");timing.ready=now();timing.budgetAtReady=Math.max(0,pairUntil-timing.ready);
        } catch (Exception ignored) { close(PairingDiagnostics.Close.TRANSPORT); throw new Exception("pairing unavailable"); }
        finally { timing.setupTimerCancelled=operation.cancel(false); if (binding != null) Arrays.fill(binding,(byte)0); if (frame != null) Arrays.fill(frame,(byte)0); }
    }
    /** Called only by the visible native Match button after comparing both screens. */
    void match() throws Exception {
        ScheduledFuture<?> operation = limit(pairUntil);
        byte[] frame = null, bootstrap = null;
        try {
            socket.setSoTimeout((int)Math.max(1,pairUntil-now()));
            frame = pairing.confirmLocal(); Frames.write(socket.getOutputStream(), frame); Arrays.fill(frame,(byte)0);
            frame = Frames.read(socket.getInputStream()); pairing.acceptConfirmation(frame); Arrays.fill(frame,(byte)0);
            frame = Frames.read(socket.getInputStream()); bootstrap = pairing.open(frame);
            if (bootstrap.length != 32) throw new Exception("session rejected");
            key = bootstrap.clone();
            if (key.length != 32) throw new Exception("session rejected");
            Arrays.fill(frame,(byte)0); frame = pairing.seal(READY); Frames.write(socket.getOutputStream(),frame);
            expiry.cancel(false); expiry = limit(sessionUntil); comparison = null;
        } catch (Exception ignored) { close(PairingDiagnostics.Close.TRANSPORT); throw new Exception("pairing unavailable"); }
        finally { operation.cancel(false); if(frame!=null)Arrays.fill(frame,(byte)0); if(bootstrap!=null)Arrays.fill(bootstrap,(byte)0); }
    }
    void enroll(android.content.Context context)throws Exception {
        if(!persistent)throw new Exception("persistent scope required");
        ScheduledFuture<?> operation=limit(pairUntil);byte[] frame=null,certificate=null;
        try{
            certificate=PhoneIdentity.certificate(enrollmentAlias);frame=pairing.seal(certificate);Frames.write(socket.getOutputStream(),frame);Arrays.fill(frame,(byte)0);
            frame=Frames.read(socket.getInputStream());certificate=pairing.open(frame);Arrays.fill(frame,(byte)0);
            frame=pairing.seal("persistent-enrolled-v1".getBytes(StandardCharsets.UTF_8));Frames.write(socket.getOutputStream(),frame);Arrays.fill(frame,(byte)0);
            frame=Frames.read(socket.getInputStream());byte[] ack=pairing.open(frame);try{if(!Arrays.equals(ack,"persistent-enrolled-v1".getBytes(StandardCharsets.UTF_8)))throw new Exception("enrollment rejected");}finally{Arrays.fill(ack,(byte)0);}
            final byte[] desktopCertificate=certificate;
            if(!publication.publish(!closed&&now()<pairUntil,()->PhoneIdentity.remember(context,desktopCertificate,enrollmentAlias)))throw new Exception("enrollment cancelled");
        }catch(Exception ignored){close(PairingDiagnostics.Close.TRANSPORT);throw new Exception("enrollment unavailable; inspect desktop phone-status, revoke and re-pair if incomplete");}
        finally{operation.cancel(false);if(frame!=null)Arrays.fill(frame,(byte)0);if(certificate!=null)Arrays.fill(certificate,(byte)0);}
    }
    JSONObject exchange(String operationName, JSONObject envelope) throws Exception {
        // Whole-exchange timer, including slow bytes. Inactivity timeout alone is insufficient.
        ScheduledFuture<?> operation = limit(Math.min(sessionUntil,now()+5000));
        byte[] plain = null, frame = null, response = null;
        try {
            socket.setSoTimeout(5000);
            plain = new JSONObject().put("operation",operationName).put("envelope",envelope).toString().getBytes(StandardCharsets.UTF_8);
            frame = pairing.seal(plain); Frames.write(socket.getOutputStream(),frame); Arrays.fill(frame,(byte)0);
            frame = Frames.read(socket.getInputStream()); response = pairing.open(frame);
            return new JSONObject(new String(response,StandardCharsets.UTF_8));
        } catch(Exception ignored) { close(PairingDiagnostics.Close.TRANSPORT); throw new Exception("session unavailable"); }
        finally { operation.cancel(false); if(plain!=null)Arrays.fill(plain,(byte)0); if(frame!=null)Arrays.fill(frame,(byte)0); if(response!=null)Arrays.fill(response,(byte)0); }
    }
    byte[] keyCopy() throws Exception { byte[] current=key; if(closed || current==null)throw new Exception("session unavailable"); return current.clone(); }
    boolean live(boolean ready) { return !closed && now() < (ready ? sessionUntil : pairUntil); }
    @Override public void close(){close(PairingDiagnostics.Close.EXPLICIT_REJECT);}
    private synchronized void close(PairingDiagnostics.Close cause) {
        if(reason==PairingDiagnostics.Close.NONE){reason=cause;timing.closed(now());}
        publication.cancel();closed=true;
        try {socket.close();}catch(Exception ignored){}
        // Crypto methods run on the sole actor. Closing the socket unblocks it; actor clears JCE references.
        byte[] current=key; key=null; if(current!=null)Arrays.fill(current,(byte)0);
        comparison=null; desktop=null; timer.shutdownNow();
    }
    void destroyEngine() { close(); VerifiedPairing current=pairing; pairing=null; if(current!=null)current.close(); }
    @Override public String toString() {return "PairingClient([redacted])";}
}
