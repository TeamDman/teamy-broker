package dev.teamy.broker.dummy;
import java.util.concurrent.ArrayBlockingQueue;
public final class SupplyDispatcherTest {
 static final class Request implements SupplyRequest {boolean live=true,submitted,fixture=true;public String id(){return "request";}public boolean live(){return live;}public boolean supplyAllowed(){return true;}public boolean fixture(){return fixture;}public boolean submitted(){return submitted;}public void markSubmitted(){submitted=true;}}
 static final class State {final Request request=new Request();boolean ready=true;}
 static final class Transport implements SupplyDispatcher.Adapter<State>{final ArrayBlockingQueue<byte[]> queue=new ArrayBlockingQueue<>(1);int calls;public boolean ready(State s){return s.ready;}public SupplyRequest pending(State s){return s.request;}public boolean offer(State s,String id,byte[] value){return queue.offer(value);}void run(){byte[] bytes;while((bytes=queue.poll())!=null){calls++;java.util.Arrays.fill(bytes,(byte)0);}}}
 static byte[] bytes(String value){return value.getBytes(java.nio.charset.StandardCharsets.UTF_8);}
 static void check(boolean value){if(!value)throw new AssertionError();}
 static void zero(byte[] bytes){for(byte value:bytes)check(value==0);}
 public static void main(String[] args){
  for(int caseId=0;caseId<5;caseId++){
   State state=new State();ServiceGeneration<State> gate=new ServiceGeneration<>(state);long generation=gate.begin(state,()->{});Transport transport=new Transport();
   if(caseId==1)state.request.live=false;if(caseId==2)state.ready=false;if(caseId==3)state.request.submitted=true;
   byte[] input=bytes(caseId==0?"accidental-real-looking-discord-token":"teamy-fixture-input-v1");
   check(!SupplyDispatcher.enqueue(gate,caseId==4?generation-1:generation,"request",input,transport));zero(input);transport.run();check(transport.calls==0&&transport.queue.isEmpty());
  }
  State state=new State();ServiceGeneration<State> gate=new ServiceGeneration<>(state);long generation=gate.begin(state,()->{});Transport transport=new Transport();byte[] input=bytes("teamy-fixture-input-v1");
  check(SupplyDispatcher.enqueue(gate,generation,"request",input,transport));check(state.request.submitted);transport.run();zero(input);check(transport.calls==1);
  System.out.println("Actual generation/request admission rejects arbitrary fixture bytes before queue/transport, wipes rejection, and transfers exact fixture only");
 }
}
