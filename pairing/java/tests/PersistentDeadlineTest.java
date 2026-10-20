import dev.teamy.broker.pairing.VerifiedPairing;
import java.util.concurrent.atomic.AtomicLong;
import org.junit.Test;
import static org.junit.Assert.*;
public final class PersistentDeadlineTest {
    @Test public void manualComparisonSurvivesNinetySecondsButExpiresAtFiveMinutes()throws Exception {
        AtomicLong clock=new AtomicLong();byte[] binding={1};
        try(VerifiedPairing phone=VerifiedPairing.persistentPreview(VerifiedPairing.Role.PHONE,binding,clock::get,300000,300000);
            VerifiedPairing desktop=VerifiedPairing.persistentPreview(VerifiedPairing.Role.DESKTOP,binding,clock::get,300000,300000)){
            desktop.acceptHandshake(phone.nextHandshake());phone.acceptHandshake(desktop.nextHandshake());desktop.acceptHandshake(phone.nextHandshake());
            clock.set(120000);assertEquals(phone.authenticationCode(),desktop.authenticationCode());
            byte[] a=phone.confirmLocal(),b=desktop.confirmLocal();phone.acceptConfirmation(b);desktop.acceptConfirmation(a);
            clock.set(299999);assertArrayEquals(new byte[]{1},desktop.open(phone.seal(new byte[]{1})));
            clock.set(300000);try{phone.seal(new byte[]{1});fail("deadline must reject");}catch(Exception expected){}
        }
    }
    @Test public void legacyAndOversizedPersistentDurationsReject()throws Exception {
        try{new VerifiedPairing(VerifiedPairing.Role.PHONE,new byte[]{1},()->0,300000,300000);fail("legacy must reject");}catch(Exception expected){}
        try{VerifiedPairing.persistentPreview(VerifiedPairing.Role.PHONE,new byte[]{1},()->0,300001,300001);fail("oversized must reject");}catch(Exception expected){}
    }
}