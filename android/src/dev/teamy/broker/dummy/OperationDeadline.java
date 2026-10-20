package dev.teamy.broker.dummy;
final class OperationDeadline implements AutoCloseable {
 interface Ticket {void cancel();}
 interface Scheduler {Ticket schedule(Runnable action,long milliseconds);}
 private final Ticket ticket;private volatile boolean expired;
 OperationDeadline(Scheduler scheduler,long milliseconds,Runnable onExpiry){ticket=scheduler.schedule(()->{expired=true;onExpiry.run();},milliseconds);}
 boolean expired(){return expired;}
 public void close(){ticket.cancel();}
}
