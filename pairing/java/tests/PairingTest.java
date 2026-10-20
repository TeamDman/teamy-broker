import dev.teamy.broker.pairing.VerifiedPairing;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.junit.Test;
import static org.junit.Assert.*;

public final class PairingTest {
    private static final byte[] BINDING = "dummy:v2:desktop:phone:ephemeral-context".getBytes(StandardCharsets.UTF_8);
    private interface Throwing { void run() throws Exception; }
    private static void rejected(Throwing operation) throws Exception {
        try { operation.run(); fail("operation must reject"); } catch (Exception expected) { /* Redacted. */ }
    }
    private static void handshake(VerifiedPairing phone, VerifiedPairing desktop) throws Exception {
        desktop.acceptHandshake(phone.nextHandshake());
        phone.acceptHandshake(desktop.nextHandshake());
        desktop.acceptHandshake(phone.nextHandshake());
    }
    private static void matched(VerifiedPairing phone, VerifiedPairing desktop) throws Exception {
        assertEquals(phone.authenticationCode(), desktop.authenticationCode());
        byte[] a = phone.confirmLocal(), b = desktop.confirmLocal();
        try { phone.acceptConfirmation(b); desktop.acceptConfirmation(a); }
        finally { Arrays.fill(a, (byte)0); Arrays.fill(b, (byte)0); }
    }
    @Test public void bothConfirmationsRequiredThenDummyTraffic() throws Exception {
        try (VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE, BINDING);
             VerifiedPairing b = new VerifiedPairing(VerifiedPairing.Role.DESKTOP, BINDING)) {
            handshake(a,b); String code = a.authenticationCode();
            assertTrue(code.matches("[0-9A-F]{4}( [0-9A-F]{4}){3}")); assertEquals(code,b.authenticationCode());
            rejected(() -> a.seal(new byte[]{1})); rejected(() -> b.seal(new byte[]{1}));
            byte[] first = a.confirmLocal(); rejected(() -> a.seal(new byte[]{1}));
            byte[] second = b.confirmLocal();
            b.acceptConfirmation(first); rejected(() -> a.seal(new byte[]{1})); a.acceptConfirmation(second);
            byte[] dummy = "TEAMY-DUMMY-ONLY".getBytes(StandardCharsets.UTF_8);
            byte[] encrypted = a.seal(dummy); assertArrayEquals(dummy,b.open(encrypted));
            Arrays.fill(dummy,(byte)0); Arrays.fill(encrypted,(byte)0); Arrays.fill(first,(byte)0); Arrays.fill(second,(byte)0);
        }
    }
    @Test public void bindingMismatchFailsWithoutApplicationTraffic() throws Exception {
        try (VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE, BINDING);
             VerifiedPairing b = new VerifiedPairing(VerifiedPairing.Role.DESKTOP, "other-dummy-context".getBytes(StandardCharsets.UTF_8))) {
            handshake(a,b); assertEquals(a.authenticationCode(), b.authenticationCode());
            byte[] ca = a.confirmLocal(), cb = b.confirmLocal();
            rejected(() -> a.acceptConfirmation(cb)); rejected(() -> b.acceptConfirmation(ca));
            rejected(() -> a.seal(new byte[]{1})); rejected(() -> b.seal(new byte[]{1}));
            Arrays.fill(ca,(byte)0); Arrays.fill(cb,(byte)0);
        }
    }
    @Test public void crossSessionReplayAndCorruptionFailClosed() throws Exception {
        try (VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING);
             VerifiedPairing b = new VerifiedPairing(VerifiedPairing.Role.DESKTOP,BINDING);
             VerifiedPairing x = new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING);
             VerifiedPairing y = new VerifiedPairing(VerifiedPairing.Role.DESKTOP,BINDING)) {
            handshake(a,b); handshake(x,y); String ca=a.authenticationCode(), cb=b.authenticationCode();
            assertEquals(ca,cb); assertNotEquals(ca,x.authenticationCode()); y.authenticationCode();
            byte[] am=a.confirmLocal(), bm=b.confirmLocal(), xm=x.confirmLocal(); y.confirmLocal();
            rejected(() -> y.acceptConfirmation(am)); // Different transcript, identical binding.
            a.acceptConfirmation(bm); b.acceptConfirmation(am);
            byte[] data=a.seal(new byte[]{1}); assertArrayEquals(new byte[]{1},b.open(data));
            rejected(() -> b.open(data)); rejected(() -> b.open(data));
            Arrays.fill(data,(byte)0); Arrays.fill(am,(byte)0); Arrays.fill(bm,(byte)0); Arrays.fill(xm,(byte)0);
        }
    }
    @Test public void pairingExpiryAndSessionExpiryAreHardAndDistinct() throws Exception {
        final long[] ticks={0};
        try (VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING,()->ticks[0],10,20);
             VerifiedPairing b = new VerifiedPairing(VerifiedPairing.Role.DESKTOP,BINDING,()->ticks[0],10,20)) {
            handshake(a,b); matched(a,b); ticks[0]=10; assertArrayEquals(new byte[]{1},b.open(a.seal(new byte[]{1})));
            ticks[0]=20; rejected(() -> a.seal(new byte[]{1})); rejected(() -> b.seal(new byte[]{1}));
        }
        ticks[0]=0;
        try (VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING,()->ticks[0],10,20);
             VerifiedPairing b = new VerifiedPairing(VerifiedPairing.Role.DESKTOP,BINDING,()->ticks[0],10,20)) {
            handshake(a,b); a.authenticationCode(); b.authenticationCode(); ticks[0]=10;
            rejected(() -> a.confirmLocal()); rejected(() -> b.confirmLocal());
        }
    }
    @Test public void cancellationAndUnexpectedStateNeverAuthorize() throws Exception {
        VerifiedPairing a = new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING);
        rejected(a::confirmLocal); rejected(() -> a.acceptConfirmation(new byte[]{1}));
        a.close(); rejected(a::nextHandshake); rejected(a::authenticationCode); rejected(() -> a.seal(new byte[]{1}));
        assertEquals("VerifiedPairing([redacted])", a.toString());
    }
    @Test public void peerConfirmationCannotBeReused() throws Exception {
        try (VerifiedPairing a=new VerifiedPairing(VerifiedPairing.Role.PHONE,BINDING);
             VerifiedPairing b=new VerifiedPairing(VerifiedPairing.Role.DESKTOP,BINDING)) {
            handshake(a,b); a.authenticationCode(); b.authenticationCode();
            byte[] ca=a.confirmLocal(), cb=b.confirmLocal(); a.acceptConfirmation(cb); b.acceptConfirmation(ca);
            rejected(() -> a.acceptConfirmation(cb)); rejected(() -> b.acceptConfirmation(ca));
            Arrays.fill(ca,(byte)0); Arrays.fill(cb,(byte)0);
        }
    }
}
