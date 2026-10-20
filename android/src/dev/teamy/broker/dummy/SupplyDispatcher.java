package dev.teamy.broker.dummy;
final class SupplyDispatcher {
 interface Adapter<S>{boolean ready(S state);SupplyRequest pending(S state);boolean offer(S state,String id,byte[] bytes);}
 static <S> boolean enqueue(ServiceGeneration<S> gate,long generation,String id,byte[] bytes,Adapter<S> adapter){
  final boolean[] queued={false};try{
   gate.update(generation,current->{SupplyRequest request=adapter.pending(current);if(!adapter.ready(current)||request==null||!request.live()||!request.supplyAllowed()||!request.id().equals(id)||request.submitted())return null;
    try{queued[0]=SupplyAdmission.enqueue(bytes,request.fixture(),value->adapter.offer(current,id,value));}catch(Exception ignored){queued[0]=false;}
    if(queued[0])request.markSubmitted();return current;
   },ignored->{});return queued[0];
  }finally{if(!queued[0]&&bytes!=null)java.util.Arrays.fill(bytes,(byte)0);}
 }
}
