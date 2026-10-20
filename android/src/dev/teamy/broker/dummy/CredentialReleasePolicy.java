package dev.teamy.broker.dummy;
/** Public debug signing is suitable only for synthetic test requests. */
public final class CredentialReleasePolicy {
    public static final String PUBLIC_DEBUG_SHA256="e96d983997281700b9d0ecc674ec11edec96d433c0fa9ddd5c216479f2b738c9";
    public static boolean allowed(boolean isolatedFixture,boolean debuggable,String packageName,String signerSha256){
        if(isolatedFixture)return true;
        return !debuggable&&"dev.teamy.broker.release".equals(packageName)&&signerSha256!=null&&signerSha256.matches("[0-9a-f]{64}")&&!PUBLIC_DEBUG_SHA256.equals(signerSha256);
    }
}
