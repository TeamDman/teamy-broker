package dev.teamy.broker.dummy;
import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
/** Checks this installed APK; TLS identity remains separately user paired. */
final class CredentialReleaseIdentity {
    static boolean allowed(Context context,boolean isolatedFixture)throws Exception {
        PackageInfo info=context.getPackageManager().getPackageInfo(context.getPackageName(),PackageManager.GET_SIGNING_CERTIFICATES);
        String signer=null;
        if(info.signingInfo!=null&&!info.signingInfo.hasMultipleSigners()){
            android.content.pm.Signature[] signatures=info.signingInfo.getApkContentsSigners();
            if(signatures.length==1){byte[] digest=java.security.MessageDigest.getInstance("SHA-256").digest(signatures[0].toByteArray());StringBuilder hex=new StringBuilder();for(byte value:digest)hex.append(String.format(java.util.Locale.ROOT,"%02x",value&255));signer=hex.toString();}
        }
        return CredentialReleasePolicy.allowed(isolatedFixture,(context.getApplicationInfo().flags&ApplicationInfo.FLAG_DEBUGGABLE)!=0,context.getPackageName(),signer);
    }
}
