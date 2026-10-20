package dev.teamy.broker.dummy;
import java.net.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
/** Adversarial lifecycle schedules use the exact production generation/socket ownership classes. */
public final class ServiceLifecycleTest {
    private static void check(boolean value){if(!value)throw new AssertionError("service ownership contract failed");}
    public static void main(String[] args)throws Exception {
        lateCleanupCannotEraseReplacement();cancellationOwnsUnfinishedHandshake();stoppedOwnerRejectsLateInstall();oldSocketReleaseCannotDetachReplacement();
        System.out.println("service generation and cancellable socket contracts passed");
    }
    private static void lateCleanupCannotEraseReplacement()throws Exception {
        ServiceGeneration<String> gate=new ServiceGeneration<>("Stopped");AtomicReference<String> notification=new AtomicReference<>();AtomicBoolean ongoing=new AtomicBoolean(true);
        long old=gate.begin("old-pending",()->notification.set("old-pending"));
        CountDownLatch cleanupReady=new CountDownLatch(1),releaseOld=new CountDownLatch(1);
        AtomicReference<Throwable> cleanupFailure=new AtomicReference<>();
        Thread cleanup=new Thread(()->{cleanupReady.countDown();try{releaseOld.await();}catch(InterruptedException e){throw new AssertionError(e);}
            check(!gate.update(old,ignored->"Disconnected",ignored->notification.set(null)));
            check(!gate.end(old,"Stopped",()->{notification.set(null);ongoing.set(false);}));
        });cleanup.setDaemon(true);cleanup.setUncaughtExceptionHandler((thread,error)->cleanupFailure.set(error));cleanup.start();check(cleanupReady.await(2,TimeUnit.SECONDS));
        long replacement=gate.begin("Connecting",()->notification.set(null));
        check(gate.update(replacement,ignored->"new-pending",ignored->notification.set("new-pending")));
        releaseOld.countDown();cleanup.join(2000);check(!cleanup.isAlive());check(cleanupFailure.get()==null);
        check(gate.snapshot().equals("new-pending"));check(notification.get().equals("new-pending"));check(ongoing.get());check(gate.current(replacement));
        check(!gate.update(old,ignored->"Connected",ignored->notification.set("old-pending")));
        check(gate.end(replacement,"Stopped",()->{notification.set(null);ongoing.set(false);}));check(notification.get()==null&&!ongoing.get());
    }
    private static void cancellationOwnsUnfinishedHandshake()throws Exception {
        SocketOwner owner=new SocketOwner();AtomicReference<AutoCloseable> completedConnection=new AtomicReference<>();
        ExecutorService actor=Executors.newSingleThreadExecutor();CountDownLatch reading=new CountDownLatch(1);
        try(ServerSocket listener=new ServerSocket(0,1,InetAddress.getLoopbackAddress());Socket socket=new Socket(listener.getInetAddress(),listener.getLocalPort());Socket peer=listener.accept()){
            check(owner.install(socket));
            Future<Boolean> blocked=actor.submit(()->{reading.countDown();try{socket.getInputStream().read();return false;}catch(Exception expected){return true;}});
            check(reading.await(2,TimeUnit.SECONDS));check(completedConnection.get()==null);
            owner.stop();check(blocked.get(2,TimeUnit.SECONDS));check(socket.isClosed());
        }finally{owner.stop();actor.shutdownNow();check(actor.awaitTermination(2,TimeUnit.SECONDS));}
    }
    private static void stoppedOwnerRejectsLateInstall(){SocketOwner owner=new SocketOwner();AtomicInteger closes=new AtomicInteger();owner.stop();check(!owner.install(closes::incrementAndGet));check(closes.get()==1);}
    private static void oldSocketReleaseCannotDetachReplacement(){
        SocketOwner owner=new SocketOwner();AtomicInteger oldClose=new AtomicInteger(),newClose=new AtomicInteger();
        AutoCloseable old=oldClose::incrementAndGet,replacement=newClose::incrementAndGet;
        check(owner.install(old));owner.release(old);check(owner.install(replacement));owner.release(old);owner.stop();check(newClose.get()==1);
    }
}