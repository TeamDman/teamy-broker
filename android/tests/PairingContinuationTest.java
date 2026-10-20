package dev.teamy.broker.dummy;
public final class PairingContinuationTest {
 public static void main(String[] args){
  if(!PairingContinuation.retainComparison(true,true,false))throw new AssertionError("ready comparison must survive chat pause");
  for(int bits=0;bits<8;bits++){boolean live=(bits&1)!=0,ready=(bits&2)!=0,busy=(bits&4)!=0;if(PairingContinuation.retainComparison(live,ready,busy)!=(live&&ready&&!busy))throw new AssertionError("unready/expired/inflight retention");}
  String code="ABCD 1234 DCBA 5678";
  if(!code.equals(PairingContinuation.copyValue(code,true,true,false)))throw new AssertionError("exact comparison copy");
  for(String bad:new String[]{null,"abcd 1234 DCBA 5678","ABCD1234DCBA5678","token-secret",code+" extra"})if(PairingContinuation.copyValue(bad,true,true,false)!=null)throw new AssertionError("noncomparison copied");
  if(PairingContinuation.copyValue(code,false,true,false)!=null||PairingContinuation.copyValue(code,true,false,false)!=null||PairingContinuation.copyValue(code,true,true,true)!=null)throw new AssertionError("background/expired/confirming copy");
  System.out.println("Pending comparison survives pause only within original live deadline; exact foreground comparison copy excludes malformed/expired/inflight/background values");
 }
}
