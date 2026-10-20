package dev.teamy.broker.dummy;
public final class CredentialInputHandoffTest{
    static byte[] bytes(ApprovalInput input)throws Exception{java.lang.reflect.Field f=ApprovalInput.class.getDeclaredField("bytes");f.setAccessible(true);return(byte[])f.get(input);}
    static void zero(byte[] value){for(byte b:value)if(b!=0)throw new AssertionError("owned bytes not cleared");}
    public static void main(String[] args)throws Exception{
        for(int fault=0;fault<4;fault++){final int point=fault;try(ApprovalInput input=new ApprovalInput("synthetic-value")){byte[] retained=bytes(input);try{CredentialInputHandoff.send(input,()->{if(point==0)throw new Exception("clear failed");},()->{if(point==1)throw new Exception("autofill failed");},value->{if(point==2)throw new Exception("sender failed");return false;});}catch(Exception expected){}zero(retained);}}
        try(ApprovalInput input=new ApprovalInput("synthetic-owned")){byte[] retained=bytes(input);if(!CredentialInputHandoff.send(input,()->{},()->{},value->true))throw new AssertionError();if(retained[0]==0)throw new AssertionError("successful ownership transfer cleared too early");java.util.Arrays.fill(retained,(byte)0);}
        System.out.println("Clear/autofill/sender exception and rejection cleanup; accepted handoff ownership passed");
    }
}
