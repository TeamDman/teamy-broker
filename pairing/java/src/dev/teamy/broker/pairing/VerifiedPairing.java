package dev.teamy.broker.pairing;

import com.google.security.cryptauth.lib.securegcm.D2DConnectionContext;
import com.google.security.cryptauth.lib.securegcm.Ukey2Handshake;
import java.security.MessageDigest;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.function.LongSupplier;

/** Session-only application gate around the unchanged official Google UKEY2 engine. */
public final class VerifiedPairing implements AutoCloseable {
    public enum Role { PHONE, DESKTOP }
    private final Role role;
    private final LongSupplier clock;
    private final long pairingUntil, sessionUntil;
    private final byte[] binding;
    private Ukey2Handshake handshake;
    private D2DConnectionContext context;
    private String code;
    private boolean local, remote, closed;
    private static final byte[] CONFIRM = "teamy-broker/dummy/ukey2/confirm/v1\0".getBytes(StandardCharsets.UTF_8);
    private static final byte[] DATA = "teamy-broker/dummy/ukey2/data/v1\0".getBytes(StandardCharsets.UTF_8);

    public VerifiedPairing(Role role, byte[] binding) throws Exception {
        this(role, binding, () -> System.nanoTime() / 1000000L, 90000, 180000);
    }
    /** Clock injection is only for deterministic expiry tests; no clock persistence. */
    public VerifiedPairing(Role role, byte[] binding, LongSupplier clock, long pairingMs, long sessionMs) throws Exception {
        this(role,binding,clock,pairingMs,sessionMs,false);
    }
    /** Persistent preview scope is separately validated by both callers before this factory. */
    public static VerifiedPairing persistentPreview(Role role,byte[] binding,LongSupplier clock,long pairingMs,long sessionMs)throws Exception {
        return new VerifiedPairing(role,binding,clock,pairingMs,sessionMs,true);
    }
    private VerifiedPairing(Role role,byte[] binding,LongSupplier clock,long pairingMs,long sessionMs,boolean persistent)throws Exception {
        if (binding.length == 0 || binding.length > 2048 || pairingMs <= 0 || pairingMs > (persistent?300000:90000)
            || sessionMs < pairingMs || sessionMs > (persistent?300000:180000)) throw new Exception("pairing policy rejected");
        this.role = role; this.binding = binding.clone(); this.clock = clock;
        long now = clock.getAsLong(); pairingUntil = Math.addExact(now, pairingMs); sessionUntil = Math.addExact(now, sessionMs);
        handshake = role == Role.PHONE
            ? Ukey2Handshake.forInitiator(Ukey2Handshake.HandshakeCipher.P256_SHA512)
            : Ukey2Handshake.forResponder(Ukey2Handshake.HandshakeCipher.P256_SHA512);
    }
    private void live() throws Exception {
        if (closed || clock.getAsLong() >= sessionUntil || ((!local || !remote) && clock.getAsLong() >= pairingUntil)) {
            close(); throw new Exception("pairing unavailable");
        }
    }
    public byte[] nextHandshake() throws Exception {
        live();
        // Upstream retains returned rawMessage1/rawMessage2 for later SAS/key derivation.
        // Caller owns only this clone and may clear it immediately after transport.
        try { return handshake.getNextHandshakeMessage().clone(); }
        catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
    }
    public void acceptHandshake(byte[] message) throws Exception {
        live(); bounded(message);
        // Upstream retains the input transcript. Never lend it the transport array that callers wipe.
        // The retained clone belongs to the engine and must survive through toConnectionContext().
        try { handshake.parseHandshakeMessage(message.clone()); }
        catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
    }
    /** Eight upstream authentication bytes = 64 bits, rendered verbatim as four hex groups. */
    public String authenticationCode() throws Exception {
        live();
        if (code != null) return code;
        byte[] auth = null;
        try {
            auth = handshake.getVerificationString(8);
            StringBuilder value = new StringBuilder(19);
            char[] hex = "0123456789ABCDEF".toCharArray();
            for (int i = 0; i < auth.length; i++) {
                if (i > 0 && i % 2 == 0) value.append(' ');
                value.append(hex[(auth[i] & 255) >>> 4]).append(hex[auth[i] & 15]);
            }
            code = value.toString(); return code;
        } catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
        finally { if (auth != null) Arrays.fill(auth, (byte)0); }
    }
    /** Must only be invoked by the local explicit Match action after actual comparison. */
    public byte[] confirmLocal() throws Exception {
        live();
        if (code == null || local || remote) throw new Exception("pairing state rejected");
        try {
            handshake.verifyHandshake(); context = handshake.toConnectionContext(); handshake = null;
            local = true;
            byte[] plain = confirmation(role);
            try { return context.encodeMessageToPeer(plain); } finally { Arrays.fill(plain, (byte)0); }
        } catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
    }
    /** Verifies remote Match, opposite role and exact canonical binding under the same derived keys. */
    public void acceptConfirmation(byte[] message) throws Exception {
        live(); bounded(message);
        if (!local || remote || context == null) { close(); throw new Exception("pairing state rejected"); }
        byte[] plain = null, expected = confirmation(role == Role.PHONE ? Role.DESKTOP : Role.PHONE);
        try {
            plain = context.decodeMessageFromPeer(message);
            if (!MessageDigest.isEqual(plain, expected)) throw new Exception("pairing context rejected");
            remote = true;
        } catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
        finally { if (plain != null) Arrays.fill(plain, (byte)0); Arrays.fill(expected, (byte)0); }
    }
    private byte[] confirmation(Role sender) {
        byte[] plain = new byte[CONFIRM.length + 1 + binding.length];
        System.arraycopy(CONFIRM, 0, plain, 0, CONFIRM.length);
        plain[CONFIRM.length] = (byte)sender.ordinal();
        System.arraycopy(binding, 0, plain, CONFIRM.length + 1, binding.length);
        return plain;
    }
    private void ready() throws Exception { live(); if (!local || !remote || context == null) throw new Exception("both confirmations required"); }
    private static void bounded(byte[] bytes) throws Exception { if (bytes.length == 0 || bytes.length > 16384) throw new Exception("pairing frame rejected"); }
    public byte[] seal(byte[] data) throws Exception {
        ready(); if (data.length > 8192) throw new Exception("pairing frame rejected");
        byte[] plain = new byte[DATA.length + data.length];
        System.arraycopy(DATA, 0, plain, 0, DATA.length); System.arraycopy(data, 0, plain, DATA.length, data.length);
        try { return context.encodeMessageToPeer(plain); }
        catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
        finally { Arrays.fill(plain, (byte)0); }
    }
    public byte[] open(byte[] message) throws Exception {
        ready(); bounded(message); byte[] plain = null;
        try {
            plain = context.decodeMessageFromPeer(message);
            if (plain.length < DATA.length || plain.length > DATA.length + 8192) throw new Exception("pairing frame rejected");
            for (int i = 0; i < DATA.length; i++) if (plain[i] != DATA[i]) throw new Exception("pairing frame rejected");
            return Arrays.copyOfRange(plain, DATA.length, plain.length);
        } catch (Exception ignored) { close(); throw new Exception("pairing rejected"); }
        finally { if (plain != null) Arrays.fill(plain, (byte)0); }
    }
    @Override public void close() {
        closed = true; local = remote = false; code = null; context = null; handshake = null;
        Arrays.fill(binding, (byte)0); // JCE/private-key objects have no portable destruction API; drop references best effort.
    }
    @Override public String toString() { return "VerifiedPairing([redacted])"; }
}
