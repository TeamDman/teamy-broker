package dev.teamy.broker.dummy;
import java.util.Arrays;
/** Owns one bounded UTF-8 value; never saved or printed. */
final class ApprovalInput implements AutoCloseable {
    private byte[] bytes;
    ApprovalInput(CharSequence text)throws Exception {
        if(text.length()==0||text.length()>4096)throw new Exception("value size rejected");
        char[] chars=new char[text.length()];for(int i=0;i<chars.length;i++)chars[i]=text.charAt(i);
        java.nio.ByteBuffer encoded=null;
        try{encoded=java.nio.charset.StandardCharsets.UTF_8.newEncoder().encode(java.nio.CharBuffer.wrap(chars));if(encoded.remaining()>4096)throw new Exception("value size rejected");bytes=new byte[encoded.remaining()];encoded.get(bytes);}
        finally{Arrays.fill(chars,'\0');if(encoded!=null&&encoded.hasArray())Arrays.fill(encoded.array(),(byte)0);}
    }
    byte[] take(){byte[] result=bytes;bytes=null;return result;}
    public void close(){if(bytes!=null){Arrays.fill(bytes,(byte)0);bytes=null;}}
    public String toString(){return "ApprovalInput([redacted])";}
}
