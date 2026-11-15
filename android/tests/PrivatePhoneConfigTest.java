package dev.teamy.broker.dummy;

public final class PrivatePhoneConfigTest {
    public static void main(String[] args) throws Exception {
        String one=String.join(".","100","64","0","1");
        String two=String.join(".","100","64","0","2");
        PrivatePhoneConfig a=new PrivatePhoneConfig(one,two,"desktop-test","phone-test");
        PrivatePhoneConfig b=new PrivatePhoneConfig(two,one,"other-desktop","other-phone");
        if(a.samePeer(b) || !a.toString().equals("PrivatePhoneConfig([redacted])"))throw new AssertionError("runtime selection/redaction");
        if(!a.samePeer(new PrivatePhoneConfig(one,two,"desktop-test","phone-test")))throw new AssertionError("same peer rejected");
        if(a.samePeer(new PrivatePhoneConfig(one,two,"other-desktop","phone-test")))throw new AssertionError("different desktop node accepted");
        if(a.samePeer(new PrivatePhoneConfig(one,two,"desktop-test","other-phone")))throw new AssertionError("different phone node accepted");
        reject(null,two,"desktop-test","phone-test");
        reject(one,null,"desktop-test","phone-test");
        reject(one,one,"desktop-test","phone-test");
        reject("127.0.0.1",two,"desktop-test","phone-test");
        reject("100.63.255.255",two,"desktop-test","phone-test");
        reject("100.128.0.1",two,"desktop-test","phone-test");
        reject("100.64.256.1",two,"desktop-test","phone-test");
        reject("100.064.0.1",two,"desktop-test","phone-test");
        reject("100.64.0.1\n",two,"desktop-test","phone-test");
        reject(one,two,"bad node","phone-test");
        reject(one,two,"desktop-test",null);
    }
    private static void reject(String a,String b,String c,String d)throws Exception {
        try {new PrivatePhoneConfig(a,b,c,d);throw new AssertionError("invalid profile accepted");}
        catch(AssertionError error){throw error;}catch(Exception expected){}
    }
}
