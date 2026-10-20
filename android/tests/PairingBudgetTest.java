package dev.teamy.broker.dummy;
public final class PairingBudgetTest {
 interface Case {void run()throws Exception;}
 static void reject(Case c)throws Exception{try{c.run();throw new AssertionError("unsafe deadline accepted");}catch(AssertionError e){throw e;}catch(Exception expected){}}
 public static void main(String[]args)throws Exception{
  long expires=1300000,mono=30100,initial=300100;
  PairingBudget normal=PairingBudget.map(true,1030000,mono,initial,initial,expires,expires);if(normal.pairUntil!=initial||normal.bindingPairMs!=270000)throw new AssertionError("normal mapping");
  PairingBudget ahead=PairingBudget.map(true,1290000,mono,initial,initial,expires,expires);if(ahead.bindingPairMs!=10000||ahead.pairUntil-mono!=10000)throw new AssertionError("positive skew reproducer");
  PairingBudget behind=PairingBudget.map(true,1020000,mono,initial,initial,expires,expires);if(behind.pairUntil!=initial)throw new AssertionError("negative skew extended local cap");
  reject(()->PairingBudget.map(true,999999,mono,initial,initial,expires,expires));reject(()->PairingBudget.map(true,expires,mono,initial,initial,expires,expires));reject(()->PairingBudget.map(true,Long.MIN_VALUE,mono,initial,initial,Long.MAX_VALUE,Long.MAX_VALUE));reject(()->PairingBudget.map(true,1030000,Long.MAX_VALUE,initial,initial,expires,expires));reject(()->PairingBudget.map(false,1030000,mono,initial,initial,expires,expires));
  PairingTiming t=new PairingTiming();t.start=100;t.ready=9000;t.bindingBudget=270000;t.budgetAtReady=261100;t.setupTimerCancelled=true;t.foreground=PairingTiming.Foreground.BACKGROUND;t.closed(18000);String report=t.format(18000,PairingDiagnostics.Close.DEADLINE);if(!report.contains("cancelled: true")||!report.contains("close elapsed_ms: 17900")||!report.contains("BACKGROUND"))throw new AssertionError("timing projection");t.bindingBudget=Long.MAX_VALUE;t.budgetAtReady=-99;report=t.format(Long.MAX_VALUE,PairingDiagnostics.Close.NONE);if(report.contains(Long.toString(Long.MAX_VALUE))||!report.contains("unavailable"))throw new AssertionError("numeric bounds");
  System.out.println("Positive skew can shorten effective budget to10s; negative skew cannot extend original local cap; expired/overflow/legacy bounds reject; fixed timing fields bounded");
 }
}
