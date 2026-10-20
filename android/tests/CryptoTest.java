import dev.teamy.broker.dummy.Crypto;
import dev.teamy.broker.dummy.BootstrapPolicy;
import dev.teamy.broker.dummy.DummyPolicy;
import java.util.Arrays;
public final class CryptoTest {
    public static void main(String[] args) throws Exception {
        byte[] key = new byte[32], nonce = new byte[12], plain = new byte[16];
        byte[] sealed = Crypto.seal(key, nonce, new byte[0], plain);
        String vector = "cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919";
        require(Crypto.hex(sealed).equals(vector));
        require(Arrays.equals(Crypto.open(key, nonce, new byte[0], sealed), plain));
        sealed[0] ^= 1;
        boolean rejected = false;
        try { Crypto.open(key, nonce, new byte[0], sealed); } catch (Exception expected) { rejected = true; }
        require(rejected);
        require(DummyPolicy.allowedValue("TEAMY-DUMMY-ONLY"));
        require(DummyPolicy.allowedPrefix("TEAMY-"));
        require(!DummyPolicy.allowedValue("NOT-A-CREDENTIAL"));
        require(!DummyPolicy.allowedPrefix("arbitrary paste"));
        String valid = new String(new char[64]).replace('\0', 'a');
        require(BootstrapPolicy.error("", "", true).contains("session key"));
        require(BootstrapPolicy.error(valid.substring(1), valid, true).contains("Session key"));
        require(BootstrapPolicy.error(valid.replace('a', 'z'), valid, true).contains("Session key"));
        require(BootstrapPolicy.error(valid, "", true).contains("certificate pin"));
        require(BootstrapPolicy.error(valid, valid.substring(1), true).contains("TLS certificate pin"));
        require(BootstrapPolicy.error(valid, valid, true) == null);
        require(BootstrapPolicy.error(valid.toUpperCase(), valid, true) == null);
        require(BootstrapPolicy.error(valid, "", false) == null);
        Arrays.fill(key, (byte)0); Arrays.fill(plain, (byte)0); Arrays.fill(sealed, (byte)0);
        System.out.println("Java AES-GCM vector, tamper rejection and dummy input/bootstrap policy passed.");
    }
    private static void require(boolean condition) { if (!condition) throw new AssertionError("contract failed"); }
}
