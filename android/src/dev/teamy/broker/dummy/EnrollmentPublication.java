package dev.teamy.broker.dummy;
/** Cancellation and final authenticated publication have one winner; no implicit rollback. */
final class EnrollmentPublication {
    private boolean cancelled;
    interface Commit {void run()throws Exception;}
    synchronized void cancel(){cancelled=true;}
    synchronized boolean publish(boolean live,Commit commit)throws Exception {if(cancelled||!live)return false;commit.run();return true;}
}