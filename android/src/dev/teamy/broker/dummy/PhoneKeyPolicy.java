package dev.teamy.broker.dummy;
/** TLS signs pre-hashed inputs; only hardware EC keys authorizing both modes are enrolled. */
final class PhoneKeyPolicy {
    static final String LEGACY="teamy-no-vault-phone-v1",TLS="teamy-no-vault-phone-tls-v2";
    static String[] digests(){return new String[]{"NONE","SHA-256","SHA-384","SHA-512"};}
    static void require(boolean hardware,String[] authorized)throws Exception {
        boolean raw=false,sha256=false;for(String value:authorized){raw|="NONE".equals(value);sha256|="SHA-256".equals(value);}
        if(!hardware||!raw||!sha256)throw new Exception("TLS key policy rejected");
    }
    static void alias(String alias)throws Exception {if(!LEGACY.equals(alias)&&!TLS.equals(alias))throw new Exception("identity alias rejected");}
}