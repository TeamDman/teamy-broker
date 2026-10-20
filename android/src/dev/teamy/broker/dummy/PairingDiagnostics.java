package dev.teamy.broker.dummy;
/** Only fixed enum names and a local creation counter; no comparison, identity or credential. */
final class PairingDiagnostics {
 enum Stage {NONE,CONNECTING,COMPARISON,CONFIRMING,ENROLLING,CLOSED}
 enum Close {NONE,EXPLICIT_REJECT,PAUSE_UNREADY,PAUSE_CONFIRMING,ACTIVITY_DESTROYED,PROCESS_RECREATED,DEADLINE,TRANSPORT,ENROLLED,UNKNOWN}
 static Close parseClose(String text){try{return Close.valueOf(text);}catch(RuntimeException ignored){return Close.UNKNOWN;}}
 static String format(long creations,Stage stage,Close close){return "Activity creation: "+Math.max(0,creations)+"\nPairing stage: "+stage.name()+"\nPairing last close: "+close.name();}
}
