package dev.teamy.broker.dummy;
final class CredentialInputHandoff{
    static boolean send(ApprovalInput input,InputAction clear,InputAction cancelAutofill,InputSender sender)throws Exception{
        byte[] owned=input.take();try{clear.run();cancelAutofill.run();if(sender.send(owned)){owned=null;return true;}return false;}finally{if(owned!=null)java.util.Arrays.fill(owned,(byte)0);}
    }
}
