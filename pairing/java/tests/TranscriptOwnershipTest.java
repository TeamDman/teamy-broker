import com.google.security.cryptauth.lib.securegcm.D2DConnectionContext;
import com.google.security.cryptauth.lib.securegcm.Ukey2Handshake;
import dev.teamy.broker.pairing.VerifiedPairing;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import org.junit.Test;
import static org.junit.Assert.*;

/** Independent official peer preserves its transcript while every caller-owned transport is wiped. */
public final class TranscriptOwnershipTest {
    private static final byte[] BINDING="dummy-transcript-ownership:exact-context:v2".getBytes(StandardCharsets.UTF_8);
    private static final byte[] CONFIRM="teamy-broker/dummy/ukey2/confirm/v1\0".getBytes(StandardCharsets.UTF_8);
    private static final byte[] DATA="teamy-broker/dummy/ukey2/data/v1\0".getBytes(StandardCharsets.UTF_8);
    private void fromWrapped(VerifiedPairing wrapped,Ukey2Handshake official)throws Exception{
        byte[] transport=wrapped.nextHandshake();
        try{official.parseHandshakeMessage(transport.clone());}finally{Arrays.fill(transport,(byte)0);}
    }
    private void fromOfficial(Ukey2Handshake official,VerifiedPairing wrapped)throws Exception{
        byte[] transport=official.getNextHandshakeMessage().clone();
        try{wrapped.acceptHandshake(transport);}finally{Arrays.fill(transport,(byte)0);}
    }
    private String render(byte[] auth){
        StringBuilder out=new StringBuilder(19);char[] digits="0123456789ABCDEF".toCharArray();
        for(int i=0;i<auth.length;i++){if(i>0&&i%2==0)out.append(' ');out.append(digits[(auth[i]&255)>>>4]).append(digits[auth[i]&15]);}
        return out.toString();
    }
    private byte[] confirmation(int role){
        byte[] plain=new byte[CONFIRM.length+1+BINDING.length];System.arraycopy(CONFIRM,0,plain,0,CONFIRM.length);plain[CONFIRM.length]=(byte)role;System.arraycopy(BINDING,0,plain,CONFIRM.length+1,BINDING.length);return plain;
    }
    private void interoperate(boolean wrappedPhone)throws Exception{
        VerifiedPairing.Role role=wrappedPhone?VerifiedPairing.Role.PHONE:VerifiedPairing.Role.DESKTOP;
        Ukey2Handshake official=wrappedPhone?Ukey2Handshake.forResponder(Ukey2Handshake.HandshakeCipher.P256_SHA512):Ukey2Handshake.forInitiator(Ukey2Handshake.HandshakeCipher.P256_SHA512);
        try(VerifiedPairing wrapped=new VerifiedPairing(role,BINDING)){
            if(wrappedPhone){fromWrapped(wrapped,official);fromOfficial(official,wrapped);fromWrapped(wrapped,official);}
            else{fromOfficial(official,wrapped);fromWrapped(wrapped,official);fromOfficial(official,wrapped);}
            byte[] auth=official.getVerificationString(8);
            try{assertTrue("independent official transcript authentication mismatch",wrapped.authenticationCode().equals(render(auth)));}finally{Arrays.fill(auth,(byte)0);}
            official.verifyHandshake();D2DConnectionContext context=official.toConnectionContext();
            byte[] encrypted=wrapped.confirmLocal(),decoded=context.decodeMessageFromPeer(encrypted),expected=confirmation(wrappedPhone?0:1);
            assertTrue("wrapped confirmation not bound to official channel",MessageDigest.isEqual(expected,decoded));Arrays.fill(encrypted,(byte)0);Arrays.fill(decoded,(byte)0);Arrays.fill(expected,(byte)0);
            byte[] peer=confirmation(wrappedPhone?1:0);encrypted=context.encodeMessageToPeer(peer);Arrays.fill(peer,(byte)0);wrapped.acceptConfirmation(encrypted);Arrays.fill(encrypted,(byte)0);
            byte[] dummy="TEAMY-DUMMY-ONLY".getBytes(StandardCharsets.UTF_8),payload=new byte[DATA.length+dummy.length];System.arraycopy(DATA,0,payload,0,DATA.length);System.arraycopy(dummy,0,payload,DATA.length,dummy.length);
            encrypted=wrapped.seal(dummy);decoded=context.decodeMessageFromPeer(encrypted);assertArrayEquals(payload,decoded);Arrays.fill(encrypted,(byte)0);Arrays.fill(decoded,(byte)0);
            encrypted=context.encodeMessageToPeer(payload);decoded=wrapped.open(encrypted);assertArrayEquals(dummy,decoded);Arrays.fill(encrypted,(byte)0);Arrays.fill(decoded,(byte)0);Arrays.fill(payload,(byte)0);Arrays.fill(dummy,(byte)0);
        }
    }
    @Test public void wrappedInitiatorPreservesTranscriptWhileCallerWipesBuffers()throws Exception{interoperate(true);}
    @Test public void wrappedResponderPreservesTranscriptWhileCallerWipesBuffers()throws Exception{interoperate(false);}
}
