package dev.teamy.broker.dummy;
import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.math.BigInteger;
import java.security.*;
import java.security.cert.*;
import java.security.spec.ECGenParameterSpec;
import java.util.Date;
import javax.net.ssl.*;
import javax.security.auth.x500.X500Principal;
/** Nonexportable phone private key; only public certificates cross authenticated pairing. */
final class PhoneIdentity {
    static final String SCOPE="persistent-no-vault-preview-v1", ALIAS=PhoneKeyPolicy.LEGACY, TLS_ALIAS=PhoneKeyPolicy.TLS;
    private static KeyStore store() throws Exception {KeyStore store=KeyStore.getInstance("AndroidKeyStore");store.load(null);return store;}
    private static DurableIdentitySelection selection(Context context){return new DurableIdentitySelection(new IdentitySelectionStore(context));}
    static byte[] certificate(String alias) throws Exception {
        PhoneKeyPolicy.alias(alias);
        KeyStore store=store();
        if(!store.containsAlias(alias)) {
            if(!TLS_ALIAS.equals(alias))throw new Exception("legacy identity unavailable");
            KeyPairGenerator generator=KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC,"AndroidKeyStore");
            generator.initialize(new KeyGenParameterSpec.Builder(alias,KeyProperties.PURPOSE_SIGN|KeyProperties.PURPOSE_VERIFY)
                .setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1")).setDigests(PhoneKeyPolicy.digests())
                .setCertificateSubject(new X500Principal("CN=Phone no-vault approval identity"))
                .setCertificateSerialNumber(new BigInteger(128,new SecureRandom()).add(BigInteger.ONE))
                .setCertificateNotBefore(new Date(System.currentTimeMillis()-86400000L))
                .setCertificateNotAfter(new Date(System.currentTimeMillis()+315360000000L)).build());
            generator.generateKeyPair();store=store();
        }
        requireTlsKey(store,alias);
        return store.getCertificate(alias).getEncoded();
    }
    static void requireTlsKey(KeyStore store,String alias)throws Exception {
        PrivateKey key=(PrivateKey)store.getKey(alias,null);
        android.security.keystore.KeyInfo info=KeyFactory.getInstance("EC","AndroidKeyStore").getKeySpec(key,android.security.keystore.KeyInfo.class);
        PhoneKeyPolicy.require(info.isInsideSecureHardware(),info.getDigests());
        // Fixed nonsecret self-test exercises Conscrypt's exact raw-signing operation before enrollment.
        byte[] digest=MessageDigest.getInstance("SHA-256").digest("teamy-broker/no-vault/tls-key-self-test/v1".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        byte[] signed=null;
        try{Signature sign=Signature.getInstance("NONEwithECDSA");sign.initSign(key);sign.update(digest);signed=sign.sign();
            Signature verify=Signature.getInstance("NONEwithECDSA");verify.initVerify(store.getCertificate(alias).getPublicKey());verify.update(digest);if(!verify.verify(signed))throw new Exception("TLS key self-test rejected");
        }finally{java.util.Arrays.fill(digest,(byte)0);if(signed!=null)java.util.Arrays.fill(signed,(byte)0);}
    }
    static boolean paired(Context context){try{return selection(context).active().paired();}catch(RuntimeException failure){return false;}}
    static void remember(Context context,byte[] desktop,String alias) throws Exception {
        PhoneKeyPolicy.alias(alias);requireTlsKey(store(),alias);
        if(desktop.length==0||desktop.length>4096)throw new Exception("identity rejected");
        X509Certificate cert=(X509Certificate)CertificateFactory.getInstance("X.509").generateCertificate(new java.io.ByteArrayInputStream(desktop));
        cert.checkValidity();cert.verify(cert.getPublicKey());
        selection(context).replace(alias,android.util.Base64.encodeToString(desktop,android.util.Base64.NO_WRAP));
    }
    static void forget(Context context) throws Exception {
        // A durable tombstone prevents legacy preferences from resurrecting trust.
        selection(context).forget();
        store().deleteEntry(ALIAS);store().deleteEntry(TLS_ALIAS);
    }
    static SSLSocket createSocket(Context context) throws Exception {
        final DurableIdentitySelection.Selection selected=ConnectionFailure.at(ConnectionFailure.Phase.PIN_LOAD,()->selection(context).active()); final String alias=selected.alias;PhoneKeyPolicy.alias(alias);
        String encoded=selected.pin;
        if(encoded==null)throw new Exception("phone not paired");
        byte[] pin=ConnectionFailure.at(ConnectionFailure.Phase.PIN_LOAD,()->android.util.Base64.decode(encoded,android.util.Base64.NO_WRAP));
        KeyStore store=ConnectionFailure.at(ConnectionFailure.Phase.KEYSTORE,PhoneIdentity::store);PrivateKey key=ConnectionFailure.at(ConnectionFailure.Phase.PHONE_KEY,()->(PrivateKey)store.getKey(alias,null));
        X509Certificate phone=(X509Certificate)store.getCertificate(alias);
        if(key==null||phone==null)throw new ConnectionFailure(ConnectionFailure.Phase.PHONE_KEY);
        X509KeyManager manager=new X509KeyManager(){
            public String[] getClientAliases(String t,java.security.Principal[] i){return new String[]{alias};}
            public String chooseClientAlias(String[] t,java.security.Principal[] i,java.net.Socket s){return alias;}
            public String[] getServerAliases(String t,java.security.Principal[] i){return null;}
            public String chooseServerAlias(String t,java.security.Principal[] i,java.net.Socket s){return null;}
            public X509Certificate[] getCertificateChain(String a){return alias.equals(a)?new X509Certificate[]{phone}:null;}
            public PrivateKey getPrivateKey(String a){return alias.equals(a)?key:null;}
        };
        X509TrustManager trust=new X509TrustManager(){
            public X509Certificate[] getAcceptedIssuers(){return new X509Certificate[0];}
            public void checkClientTrusted(X509Certificate[] c,String a)throws CertificateException{throw new CertificateException("client role rejected");}
            public void checkServerTrusted(X509Certificate[] c,String a)throws CertificateException{
                try{if(c.length!=1||!MessageDigest.isEqual(c[0].getEncoded(),pin))throw new CertificateException("desktop identity changed");c[0].checkValidity();c[0].verify(c[0].getPublicKey());}
                catch(Exception ignored){throw new CertificateException("desktop identity rejected");}
            }
        };
        SSLContext tls=ConnectionFailure.at(ConnectionFailure.Phase.TLS_CONTEXT,()->{SSLContext value=SSLContext.getInstance("TLSv1.3");value.init(new KeyManager[]{manager},new TrustManager[]{trust},new SecureRandom());return value;});
        SSLSocket socket=ConnectionFailure.at(ConnectionFailure.Phase.TLS_SOCKET,()->(SSLSocket)tls.getSocketFactory().createSocket());
        try{socket.setEnabledProtocols(new String[]{"TLSv1.3"});socket.setSoTimeout(PhoneTiming.CONNECT_MS);return socket;}
        catch(Exception ignored){try{socket.close();}catch(Exception ignoredClose){}throw new ConnectionFailure(ConnectionFailure.Phase.TLS_SOCKET);}
    }
}