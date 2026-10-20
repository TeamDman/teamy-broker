package dev.teamy.broker.dummy;
/** Existing absolute-wall deadline policy mapped to bounded local monotonic deadlines. */
final class PairingBudget {
 final long bindingPairMs,pairUntil,sessionUntil;
 private PairingBudget(long left,long pair,long session){bindingPairMs=left;pairUntil=pair;sessionUntil=session;}
 static PairingBudget map(boolean persistent,long wall,long mono,long initialPair,long initialSession,long claimedPair,long claimedSession)throws Exception {
  long pairLeft=Math.subtractExact(claimedPair,wall),sessionLeft=Math.subtractExact(claimedSession,wall);
  if(pairLeft<=0||pairLeft>(persistent?300000:90000)||sessionLeft<pairLeft||sessionLeft>(persistent?300000:180000))throw new Exception("pairing deadline rejected");
  return new PairingBudget(pairLeft,Math.min(initialPair,Math.addExact(mono,pairLeft)),Math.min(initialSession,Math.addExact(mono,sessionLeft)));
 }
}
