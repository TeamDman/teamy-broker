package dev.teamy.broker.dummy;
public final class ConnectionFailureTest {
    public static void main(String[]args)throws Exception {
        String marker="PRIVATE-SENTINEL-NEVER-EXPOSE";
        try{ConnectionFailure.at(ConnectionFailure.Phase.TLS_CONTEXT,()->{throw new Exception(marker);});throw new AssertionError("must fail");}
        catch(ConnectionFailure error){if(error.phase!=ConnectionFailure.Phase.TLS_CONTEXT||error.getCause()!=null||error.toString().contains(marker))throw new AssertionError("redaction failed");}
        System.out.println("Fixed connection-phase redaction passed");
    }
}