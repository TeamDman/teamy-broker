package dev.teamy.broker.dummy;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import javax.net.ssl.SSLSocket;
/** Isolated JVM smoke test of the exact Android TLS helper, never a device-validation claim. */
public final class TlsTest {
    public static void main(String[] args) throws Exception {
        boolean rejected = args.length > 2;
        try (Socket socket = PinnedTls.socket(args[1])) {
            socket.connect(new InetSocketAddress("127.0.0.1", Integer.parseInt(args[0])), 3000);
            socket.setSoTimeout(3000); ((SSLSocket)socket).startHandshake();
            if (rejected) throw new AssertionError("Wrong pin accepted");
            byte[] value = DummyPolicy.VALUE.getBytes(StandardCharsets.UTF_8);
            DataOutputStream out = new DataOutputStream(socket.getOutputStream());
            out.writeInt(value.length); out.write(value); out.flush();
            DataInputStream in = new DataInputStream(socket.getInputStream());
            int size = in.readInt(); if (size != value.length) throw new AssertionError("Invalid frame");
            byte[] result = new byte[size]; in.readFully(result);
            if (!Arrays.equals(value,result)) throw new AssertionError("Invalid dummy response");
            Arrays.fill(value,(byte)0); Arrays.fill(result,(byte)0);
        } catch (javax.net.ssl.SSLHandshakeException expected) { if (!rejected) throw expected; }
        System.out.println("Isolated TLS pin test passed");
    }
}
