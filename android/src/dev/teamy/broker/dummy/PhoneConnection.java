package dev.teamy.broker.dummy;
import android.content.Context;
import dev.teamy.broker.pairing.Frames;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import javax.net.ssl.SSLSocket;
import org.json.JSONObject;
/** Standard TLS 1.3 mTLS; fresh session/strict sequence and a preview-only operation allowlist. */
final class PhoneConnection implements AutoCloseable {
    private final java.util.concurrent.ScheduledExecutorService timer;
    private final SSLSocket socket;
    private OperationDeadline deadline(long milliseconds){return new OperationDeadline((action,delay)->{java.util.concurrent.ScheduledFuture<?> ticket=timer.schedule(action,delay,java.util.concurrent.TimeUnit.MILLISECONDS);return ()->ticket.cancel(false);},milliseconds,this::close);}
    private final SocketOwner owner;
    private final String session;
    private long sequence;
    final JSONObject hello;
    PhoneConnection(Context context,SocketOwner owner)throws Exception {
        this.owner=owner;socket=PhoneIdentity.createSocket(context);
        if(!owner.install(socket))throw new Exception("approval connection cancelled");
        timer=java.util.concurrent.Executors.newSingleThreadScheduledExecutor();
        OperationDeadline limit=deadline(PhoneTiming.CONNECT_MS);
        ConnectionFailure.Phase phase=ConnectionFailure.Phase.TCP_CONNECT;
        try{PrivatePhoneConfig.requireConfigured();socket.connect(new java.net.InetSocketAddress(PrivatePhoneConfig.DESKTOP,PrivatePhoneConfig.PORT),PhoneTiming.TCP_MS);phase=ConnectionFailure.Phase.TLS_HANDSHAKE;socket.startHandshake();phase=ConnectionFailure.Phase.HELLO_READ;hello=read();phase=ConnectionFailure.Phase.HELLO_VALIDATION;session=hello.getString("session_id");java.util.UUID.fromString(session);validate(hello,0);}
        catch(Exception error){close();throw new ConnectionFailure(phase,limit.expired()?ConnectionFailure.Category.TIMEOUT:ConnectionFailure.classify(error));}finally{limit.close();}
    }
    private JSONObject read()throws Exception {byte[] bytes=Frames.read(socket.getInputStream());try{if(bytes.length>16384)throw new Exception("oversized metadata");return new JSONObject(new String(bytes,StandardCharsets.UTF_8));}finally{Arrays.fill(bytes,(byte)0);}}
    private void validate(JSONObject reply,long expected)throws Exception {
        if(reply.length()!=6||reply.getInt("version")!=1||!reply.getString("scope").equals(PhoneIdentity.SCOPE)||!reply.getString("session_id").equals(session)||reply.getLong("sequence")!=expected)throw new Exception("approval protocol rejected");
    }
    JSONObject exchange(String id,String decision)throws Exception {return exchange(id,decision,null);}
    JSONObject exchange(String id,String decision,byte[] value)throws Exception {
        long next=Math.addExact(sequence,1);String operation=value!=null?"supply":id==null?"poll":"decide";
        JSONObject request=new JSONObject().put("version",1).put("scope",PhoneIdentity.SCOPE).put("session_id",session).put("sequence",next).put("operation",operation).put("request_id",id==null?JSONObject.NULL:id).put("decision",decision==null?JSONObject.NULL:decision);
        byte[] bytes=null;OperationDeadline limit=null,outbound=null;
        ConnectionFailure.Phase phase=ConnectionFailure.Phase.EXCHANGE_WRITE;
        try{
            limit=deadline(PhoneTiming.EXCHANGE_MS);socket.setSoTimeout(PhoneTiming.EXCHANGE_MS);
            bytes=request.toString().getBytes(StandardCharsets.UTF_8);
            outbound=deadline(PhoneTiming.OUTBOUND_MS);
            try{Frames.write(socket.getOutputStream(),bytes);if(value!=null)Frames.write(socket.getOutputStream(),value);}finally{outbound.close();}
            phase=ConnectionFailure.Phase.EXCHANGE_READ;JSONObject reply=read();
            phase=ConnectionFailure.Phase.REPLY_VALIDATION;validate(reply,next);sequence=next;return reply;
        }catch(Exception error){boolean expired=(limit!=null&&limit.expired())||(outbound!=null&&outbound.expired());close();throw new ConnectionFailure(phase,expired?ConnectionFailure.Category.TIMEOUT:ConnectionFailure.classify(error));}
        finally{if(limit!=null)limit.close();if(outbound!=null)outbound.close();if(bytes!=null)Arrays.fill(bytes,(byte)0);if(value!=null)Arrays.fill(value,(byte)0);}

    }
    public void close(){try{socket.close();}catch(Exception ignored){}timer.shutdownNow();owner.release(socket);}
    public String toString(){return "PhoneConnection([no-vault])";}
}
