package dev.teamy.broker.pairing;

import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

/** Bounded binary frames for socket transport and private anonymous process pipes. */
public final class Frames {
    private Frames() {}
    public static byte[] read(InputStream input) throws Exception {
        DataInputStream stream = new DataInputStream(input);
        int size = stream.readInt();
        if (size < 1 || size > 16384) throw new Exception("pairing frame rejected");
        byte[] data = new byte[size];
        try { stream.readFully(data); return data; }
        catch (Exception ignored) { java.util.Arrays.fill(data, (byte)0); throw new Exception("pairing frame rejected"); }
    }
    public static void write(OutputStream output, byte[] data) throws Exception {
        if (data.length < 1 || data.length > 16384) throw new Exception("pairing frame rejected");
        DataOutputStream stream = new DataOutputStream(output);
        stream.writeInt(data.length); stream.write(data); stream.flush();
    }
}
