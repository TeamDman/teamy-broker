package dev.teamy.broker.dummy;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
public final class PhoneKeyPolicyTest {
    private static void check(boolean value){if(!value)throw new AssertionError("key policy/publication contract failed");}
    interface Action {void run()throws Exception;}
    private static void rejects(Action action)throws Exception {try{action.run();throw new AssertionError("must reject");}catch(Exception expected){}}
    public static void main(String[]args)throws Exception {
        PhoneKeyPolicy.require(true,PhoneKeyPolicy.digests());rejects(()->PhoneKeyPolicy.require(false,PhoneKeyPolicy.digests()));rejects(()->PhoneKeyPolicy.require(true,new String[]{"SHA-256"}));rejects(()->PhoneKeyPolicy.require(true,new String[]{"NONE"}));
        String[] mutated=PhoneKeyPolicy.digests();mutated[0]="invalid";check("NONE".equals(PhoneKeyPolicy.digests()[0]));rejects(()->PhoneKeyPolicy.alias("unapproved"));
        AtomicReference<String> active=new AtomicReference<>("legacy");EnrollmentPublication cancelled=new EnrollmentPublication();cancelled.cancel();check(!cancelled.publish(true,()->active.set("replacement")));check(active.get().equals("legacy"));
        EnrollmentPublication incomplete=new EnrollmentPublication();check(!incomplete.publish(false,()->active.set("replacement")));check(active.get().equals("legacy"));
        rejects(()->incomplete.publish(true,()->{throw new Exception("simulated commit failure");}));check(active.get().equals("legacy"));
        EnrollmentPublication complete=new EnrollmentPublication();check(complete.publish(true,()->active.set("replacement")));complete.cancel();check(active.get().equals("replacement"));
        System.out.println("Hardware/raw-digest key policy and final-ack cancellation/publication contracts passed; device self-test remains required.");
    }
}