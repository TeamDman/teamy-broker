package dev.teamy.broker.dummy;
/** Existing absolute-wall deadline policy mapped to bounded local monotonic deadlines. */
final class PairingBudget {
 final long bindingPairMs,pairUntil,sessionUntil;
 private PairingBudget(long left,long pair,long session){bindingPairMs=left;pairUntil=pair;sessionUntil=session;}
 static PairingBudget map(long wall,long mono,long initialPair,long initialSession,long claimedPair,long claimedSession)throws Exception {
  long pairLeft=Math.subtractExact(claimedPair,wall),sessionLeft=Math.subtractExact(claimedSession,wall);
  if(pairLeft<=0||pairLeft>300000||sessionLeft<pairLeft||sessionLeft>300000)throw new Exception("pairing deadline rejected");
  return new PairingBudget(pairLeft,Math.min(initialPair,Math.addExact(mono,pairLeft)),Math.min(initialSession,Math.addExact(mono,sessionLeft)));
 }
}
