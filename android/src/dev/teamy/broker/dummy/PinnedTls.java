package dev.teamy.broker.dummy;

import java.net.Socket;
import java.security.MessageDigest;
import java.security.cert.CertificateException;
import java.security.cert.X509Certificate;
import java.util.Arrays;
import java.util.List;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSocket;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;

/** Session-only exact certificate pin. No trust-all manager, persistent store, or production mode. */
public final class PinnedTls {
    public static final String DESKTOP = PrivatePhoneConfig.DESKTOP;
    public static final int PORT = PrivatePhoneConfig.PORT;
    private PinnedTls() {}
    public static Socket socket(String pinText) throws Exception {
        final byte[] pin = Crypto.unhex(pinText);
        if (pin.length != 32) { Arrays.fill(pin, (byte)0); throw new CertificateException("invalid pin"); }
        SSLContext context = SSLContext.getInstance("TLS");
        context.init(null, new TrustManager[] {new X509TrustManager() {
            @Override public X509Certificate[] getAcceptedIssuers() { return new X509Certificate[0]; }
            @Override public void checkClientTrusted(X509Certificate[] chain, String type) throws CertificateException { throw new CertificateException("client role rejected"); }
            @Override public void checkServerTrusted(X509Certificate[] chain, String type) throws CertificateException {
                try {
                    if (chain == null || chain.length != 1) throw new CertificateException("single pinned certificate required");
                    chain[0].checkValidity();
                    List<String> uses = chain[0].getExtendedKeyUsage();
                    if (uses == null || !uses.contains("1.3.6.1.5.5.7.3.1")) throw new CertificateException("server use required");
                    boolean address = false;
                    if (chain[0].getSubjectAlternativeNames() != null) for (List<?> name : chain[0].getSubjectAlternativeNames()) {
                        if (Integer.valueOf(7).equals(name.get(0)) && DESKTOP.equals(name.get(1))) address = true;
                    }
                    if (!address) throw new CertificateException("desktop address rejected");
                    byte[] hash = MessageDigest.getInstance("SHA-256").digest(chain[0].getEncoded());
                    try { if (!MessageDigest.isEqual(pin, hash)) throw new CertificateException("pin rejected"); }
                    finally { Arrays.fill(hash, (byte)0); }
                } catch (CertificateException error) { throw error; }
                catch (Exception ignored) { throw new CertificateException("certificate rejected"); }
                finally { Arrays.fill(pin, (byte)0); }
            }
        }}, null);
        SSLSocket socket = (SSLSocket)context.getSocketFactory().createSocket();
        // API 26 has TLS 1.2; use only that established floor, with no older-protocol fallback.
        socket.setEnabledProtocols(new String[] {"TLSv1.2"});
        return socket;
    }
}
