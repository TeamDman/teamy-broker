package dev.teamy.broker.dummy;
/** In-memory numeric/enum projection only; no identity, code, reference or exception text. */
final class PairingTiming {
 enum Foreground {UNKNOWN,FOREGROUND,BACKGROUND}
 volatile long start=-1,ready=-1,bindingBudget=-1,budgetAtReady=-1,closedAt=-1;
 volatile Boolean setupTimerCancelled;
 volatile Foreground foreground=Foreground.UNKNOWN,foregroundAtClose=Foreground.UNKNOWN;
 void closed(long now){if(closedAt<0){closedAt=now;foregroundAtClose=foreground;}}
 private static String bounded(long value,long max){return value<0?"unavailable":Long.toString(Math.min(value,max));}
 private static long elapsed(long start,long now){if(start<0||now<start)return -1;try{return Math.subtractExact(now,start);}catch(ArithmeticException ignored){return -1;}}
 String format(long now,PairingDiagnostics.Close reason){return "Pairing setup timer cancelled: "+(setupTimerCancelled==null?"not_complete":setupTimerCancelled.toString())+"\nPairing binding remaining_ms: "+bounded(bindingBudget,300000)+"\nPairing budget at code_ms: "+bounded(budgetAtReady,300000)+"\nPairing setup elapsed_ms: "+bounded(elapsed(start,ready),600000)+"\nPairing local elapsed_ms: "+bounded(elapsed(start,now),600000)+"\nPairing local close: "+reason.name()+"\nPairing close elapsed_ms: "+bounded(elapsed(start,closedAt),600000)+"\nPairing foreground: "+foreground.name()+"\nPairing foreground at close: "+foregroundAtClose.name();}
}
