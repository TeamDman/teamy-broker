package dev.teamy.broker.pairing;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Windows desktop adapter. stdout is exclusively a private anonymous binary pipe, never a log. */
public final class PipeWorker {
    public static void main(String[] ignoredArgs) {
        boolean phone=false,persistent=false;
        if(ignoredArgs.length>2)return;
        for(String arg:ignoredArgs){if("--test-phone".equals(arg)&&!phone)phone=true;else if("--persistent-preview".equals(arg)&&!persistent)persistent=true;else return;}
        VerifiedPairing.Role role = phone ? VerifiedPairing.Role.PHONE : VerifiedPairing.Role.DESKTOP;
        VerifiedPairing pairing = null;
        try {
            while (true) {
                byte[] command = Frames.read(System.in), result = null, payload = Arrays.copyOfRange(command, 1, command.length);
                try {
                    int op = command[0] & 255;
                    if (op == 0) {
                        if (pairing != null) throw new Exception("pairing state rejected");
                        pairing = persistent ? VerifiedPairing.persistentPreview(role,payload,()->System.nanoTime()/1000000L,300000,300000) : new VerifiedPairing(role, payload); result = new byte[0];
                    } else {
                        if (pairing == null) throw new Exception("pairing state rejected");
                        switch (op) {
                            case 1: pairing.acceptHandshake(payload); result = new byte[0]; break;
                            case 2: result = pairing.nextHandshake(); break;
                            case 3: result = pairing.authenticationCode().getBytes(StandardCharsets.US_ASCII); break;
                            case 4: result = pairing.confirmLocal(); break;
                            case 5: pairing.acceptConfirmation(payload); result = new byte[0]; break;
                            case 6: result = pairing.seal(payload); break;
                            case 7: result = pairing.open(payload); break;
                            default: throw new Exception("pairing state rejected");
                        }
                    }
                    byte[] response = new byte[1 + result.length];
                    System.arraycopy(result, 0, response, 1, result.length);
                    try { Frames.write(System.out, response); } finally { Arrays.fill(response, (byte)0); }
                } finally {
                    Arrays.fill(command, (byte)0); Arrays.fill(payload, (byte)0); if (result != null) Arrays.fill(result, (byte)0);
                }
            }
        } catch (Exception ignored) {
            try { Frames.write(System.out, new byte[]{1}); } catch (Exception ignoredWrite) {}
        } finally { if (pairing != null) pairing.close(); }
    }
}
