package dev.teamy.broker.dummy;
import java.util.*;
public final class OperationDeadlineTest {
 static final class Clock implements OperationDeadline.Scheduler {
  long now;final List<Event> events=new ArrayList<>();
  public OperationDeadline.Ticket schedule(Runnable action,long ms){Event event=new Event(now+ms,action);events.add(event);return ()->event.cancelled=true;}
  void advance(long ms){now+=ms;for(Event event:events)if(!event.cancelled&&event.at<=now){event.cancelled=true;event.action.run();}}
 }
 static final class Event {final long at;final Runnable action;boolean cancelled;Event(long at,Runnable action){this.at=at;this.action=action;}}
 static void check(boolean value){if(!value)throw new AssertionError();}
 public static void main(String[] args){
  Clock clock=new Clock();int[] closed={0};
  OperationDeadline exchange=new OperationDeadline(clock,PhoneTiming.EXCHANGE_MS,()->closed[0]++);
  OperationDeadline write=new OperationDeadline(clock,PhoneTiming.OUTBOUND_MS,()->closed[0]++);
  clock.advance(4000);write.close();clock.advance(6000);clock.advance(5000);clock.advance(6000);clock.advance(5000);
  check(closed[0]==0&&!exchange.expired());exchange.close();clock.advance(100000);check(closed[0]==0);
  OperationDeadline timeout=new OperationDeadline(clock,PhoneTiming.EXCHANGE_MS,()->closed[0]++);clock.advance(PhoneTiming.EXCHANGE_MS);check(timeout.expired()&&closed[0]==1);
  OperationDeadline stalledWrite=new OperationDeadline(clock,PhoneTiming.OUTBOUND_MS,()->closed[0]++);clock.advance(PhoneTiming.OUTBOUND_MS);check(stalledWrite.expired()&&closed[0]==2);
  OperationDeadline connecting=new OperationDeadline(clock,PhoneTiming.CONNECT_MS,()->closed[0]++);clock.advance(3000+3000+6000+5000+5000);check(!connecting.expired());connecting.close();clock.advance(100000);check(closed[0]==2);
  System.out.println("Bounded slow-stage, outbound timeout, constructor and cancelled deadline contracts passed");
 }
}
