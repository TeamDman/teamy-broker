import dev.teamy.broker.pairing.Frames;
import dev.teamy.broker.pairing.VerifiedPairing;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.util.Arrays;
import org.junit.Test;
import static org.junit.Assert.*;

public final class PairingNegativeTest {
    private interface Action{void run()throws Exception;}
    private void denied(Action action)throws Exception{try{action.run();fail("must reject");}catch(Exception expected){}}
    private void handshake(VerifiedPairing a,VerifiedPairing b)throws Exception{b.acceptHandshake(a.nextHandshake());a.acceptHandshake(b.nextHandshake());b.acceptHandshake(a.nextHandshake());a.authenticationCode();b.authenticationCode();}
    @Test public void wrongDirectionConfirmationClosesSession()throws Exception{
        try(VerifiedPairing a=new VerifiedPairing(VerifiedPairing.Role.PHONE,new byte[]{1});VerifiedPairing b=new VerifiedPairing(VerifiedPairing.Role.DESKTOP,new byte[]{1})){
            handshake(a,b);byte[] own=a.confirmLocal();b.confirmLocal();denied(()->a.acceptConfirmation(own));denied(()->a.seal(new byte[]{1}));Arrays.fill(own,(byte)0);
        }
    }
    @Test public void tamperedCiphertextClosesChannel()throws Exception{
        try(VerifiedPairing a=new VerifiedPairing(VerifiedPairing.Role.PHONE,new byte[]{1});VerifiedPairing b=new VerifiedPairing(VerifiedPairing.Role.DESKTOP,new byte[]{1})){
            handshake(a,b);byte[] am=a.confirmLocal(),bm=b.confirmLocal();a.acceptConfirmation(bm);b.acceptConfirmation(am);
            byte[] ciphertext=a.seal(new byte[]{1});ciphertext[ciphertext.length-1]^=1;denied(()->b.open(ciphertext));denied(()->b.seal(new byte[]{1}));Arrays.fill(am,(byte)0);Arrays.fill(bm,(byte)0);Arrays.fill(ciphertext,(byte)0);
        }
    }
    @Test public void malformedFramesRejectBeforeAllocation()throws Exception{
        for(byte[] frame:new byte[][]{{0,0,0,0},{0,0,64,1},{127,-1,-1,-1},{0,0,0,2,1}})denied(()->Frames.read(new ByteArrayInputStream(frame)));
        ByteArrayOutputStream out=new ByteArrayOutputStream();Frames.write(out,new byte[]{1,2});assertArrayEquals(new byte[]{1,2},Frames.read(new ByteArrayInputStream(out.toByteArray())));
    }
    @Test public void invalidHandshakeCannotProduceCode()throws Exception{
        try(VerifiedPairing a=new VerifiedPairing(VerifiedPairing.Role.PHONE,new byte[]{1})){denied(()->a.acceptHandshake(new byte[]{1,2,3}));denied(a::authenticationCode);denied(a::confirmLocal);}
    }
}
