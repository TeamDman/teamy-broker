package dev.teamy.broker.dummy;
/** Stop can close a raw socket before TCP connect/TLS handshake/hello completes. */
final class SocketOwner {
    private AutoCloseable current;
    private boolean stopped;
    boolean install(AutoCloseable candidate){
        synchronized(this){if(!stopped&&current==null){current=candidate;return true;}}
        close(candidate);return false;
    }
    synchronized void release(AutoCloseable expected){if(current==expected)current=null;}
    void stop(){AutoCloseable owned;synchronized(this){stopped=true;owned=current;current=null;}close(owned);}
    private static void close(AutoCloseable value){if(value!=null)try{value.close();}catch(Exception ignored){}}
}