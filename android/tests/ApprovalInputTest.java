package dev.teamy.broker.dummy;
public final class ApprovalInputTest{
    public static void main(String[] args)throws Exception{
        try(ApprovalInput input=new ApprovalInput("synthetic-token")){if(input.toString().contains("synthetic-token"))throw new AssertionError();byte[] bytes=input.take();if(bytes.length!=15)throw new AssertionError();java.util.Arrays.fill(bytes,(byte)0);}
        for(String invalid:new String[]{"",new String(new char[4097]).replace('\0','x'),new String(new char[2049]).replace('\0','é')}){try(ApprovalInput ignored=new ApprovalInput(invalid)){throw new AssertionError();}catch(Exception expected){}}
        try(ApprovalInput valid=new ApprovalInput(new String(new char[4096]).replace('\0','x'))){byte[] bytes=valid.take();if(bytes.length!=4096)throw new AssertionError();java.util.Arrays.fill(bytes,(byte)0);}
        System.out.println("Masked-input ownership, UTF-8 bounds and redaction contracts passed");
    }
}
