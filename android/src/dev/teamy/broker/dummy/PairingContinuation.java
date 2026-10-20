package dev.teamy.broker.dummy;
/** Retains only already displayed, unconfirmed comparison within its original deadline. */
final class PairingContinuation {
 static boolean retainComparison(boolean live,boolean ready,boolean busy){return live&&ready&&!busy;}
 static String copyValue(String code,boolean foreground,boolean live,boolean busy){
  return foreground&&retainComparison(live,code!=null,busy)&&code.matches("[0-9A-F]{4}(?: [0-9A-F]{4}){3}")?code:null;
 }
}
