package dev.teamy.broker.dummy;

import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.json.JSONObject;
import javax.net.ssl.SSLSocket;

/** Foreground device-loopback protocol; no emulator detection or remote-host option. */
public final class Protocol {
    private Protocol() {}
    public static JSONObject envelope(byte[] key, byte[] aad, byte[] plain) throws Exception {
        byte[] nonce = Crypto.random(12);
        byte[] ciphertext = Crypto.seal(key, nonce, aad, plain);
        try {
            return new JSONObject().put("nonce", Crypto.hex(nonce)).put("ciphertext", Crypto.hex(ciphertext));
        } finally {
            Arrays.fill(nonce, (byte)0);
            Arrays.fill(ciphertext, (byte)0);
        }
    }
    public static byte[] open(byte[] key, byte[] aad, JSONObject envelope) throws Exception {
        byte[] nonce = Crypto.unhex(envelope.getString("nonce"));
        byte[] ciphertext = Crypto.unhex(envelope.getString("ciphertext"));
        try { return Crypto.open(key, nonce, aad, ciphertext); }
        finally { Arrays.fill(nonce, (byte)0); Arrays.fill(ciphertext, (byte)0); }
    }
    public static JSONObject exchange(Socket socket, boolean tls, String operation, JSONObject envelope) throws Exception {
        // Desktop reachability requires a separately authorized test tunnel; none is configured here.
        java.util.concurrent.ScheduledExecutorService timer = java.util.concurrent.Executors.newSingleThreadScheduledExecutor();
        java.util.concurrent.ScheduledFuture<?> watchdog = timer.schedule(() -> { try { socket.close(); } catch (Exception ignored) {} }, 8, java.util.concurrent.TimeUnit.SECONDS);
        try {
        if (tls) PrivatePhoneConfig.requireConfigured();
        socket.connect(new InetSocketAddress(tls ? PinnedTls.DESKTOP : "127.0.0.1", tls ? PinnedTls.PORT : 43187), 3000);
        socket.setSoTimeout(5000);
        if (tls) ((SSLSocket)socket).startHandshake();
        byte[] packet = new JSONObject().put("operation", operation).put("envelope", envelope).toString().getBytes(StandardCharsets.UTF_8);
        byte[] result = null;
        try {
            if (packet.length > 16384) throw new Exception("oversized packet");
            DataOutputStream output = new DataOutputStream(socket.getOutputStream());
            output.writeInt(packet.length); output.write(packet); output.flush();
            DataInputStream input = new DataInputStream(socket.getInputStream());
            int length = input.readInt();
            if (length < 0 || length > 16384) throw new Exception("oversized packet");
            result = new byte[length]; input.readFully(result);
            return new JSONObject(new String(result, StandardCharsets.UTF_8));
        } finally {
            Arrays.fill(packet, (byte)0);
            if (result != null) Arrays.fill(result, (byte)0);
        }
        } finally { watchdog.cancel(false); timer.shutdownNow(); }
    }
}
