package dev.teamy.broker.dummy;
public final class CredentialReleasePolicyTest {
 public static void main(String[] args){
  String key=new String(new char[64]).replace('\0','a');
  check(CredentialReleasePolicy.allowed(true,true,"dev.teamy.broker.dummy",CredentialReleasePolicy.PUBLIC_DEBUG_SHA256));
  check(!CredentialReleasePolicy.allowed(false,true,"dev.teamy.broker.release",key));
  check(!CredentialReleasePolicy.allowed(false,false,"dev.teamy.broker.release",CredentialReleasePolicy.PUBLIC_DEBUG_SHA256));
  check(!CredentialReleasePolicy.allowed(false,false,"dev.teamy.broker.dummy",key));
  check(!CredentialReleasePolicy.allowed(false,false,"dev.teamy.broker.release",null));
  check(CredentialReleasePolicy.allowed(false,false,"dev.teamy.broker.release",key));
  System.out.println("Credential release identity gate passed");
 }
 static void check(boolean value){if(!value)throw new AssertionError();}
}
