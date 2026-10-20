package dev.teamy.broker.dummy;
/** Fixed phase/category only; exception text, causes and protocol material never escape. */
final class ConnectionFailure extends Exception {
    enum Phase { PIN_LOAD,KEYSTORE,PHONE_KEY,TLS_CONTEXT,TLS_SOCKET,TCP_CONNECT,TLS_HANDSHAKE,HELLO_READ,HELLO_VALIDATION,EXCHANGE,EXCHANGE_WRITE,EXCHANGE_READ,REPLY_VALIDATION }
    enum Category { UNAVAILABLE,TIMEOUT,CERTIFICATE,KEY_OPERATION,TLS_PROTOCOL,TRANSPORT }
    final Phase phase;final Category category;
    ConnectionFailure(Phase phase){this(phase,Category.UNAVAILABLE);}
    ConnectionFailure(Phase phase,Category category){super("approval connection phase failed");this.phase=phase;this.category=category;}
    interface Operation<T>{T run()throws Exception;}
    static <T>T at(Phase phase,Operation<T> operation)throws ConnectionFailure {try{return operation.run();}catch(Exception error){throw new ConnectionFailure(phase,classify(error));}}
    static Category classify(Throwable error){
        Category result=Category.UNAVAILABLE;
        for(int depth=0;error!=null&&depth<8;depth++,error=error.getCause()){
            if(error instanceof java.security.cert.CertificateException)return Category.CERTIFICATE;
            if(error instanceof java.security.InvalidKeyException||error instanceof java.security.SignatureException)return Category.KEY_OPERATION;
            if(error instanceof java.net.SocketTimeoutException)return Category.TIMEOUT;
            if(error instanceof javax.net.ssl.SSLException)result=Category.TLS_PROTOCOL;
            else if(result==Category.UNAVAILABLE&&error instanceof java.io.IOException)result=Category.TRANSPORT;
        }
        return result;
    }
}