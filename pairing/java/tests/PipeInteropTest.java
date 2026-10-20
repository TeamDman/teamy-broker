import dev.teamy.broker.pairing.Frames;
import dev.teamy.broker.pairing.VerifiedPairing;
import java.nio.charset.StandardCharsets;
import java.io.File;
import java.util.Arrays;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import org.junit.Test;
import static org.junit.Assert.*;

/** Same private binary pipe contract used by Rust, exercised against an independent phone engine. */
public final class PipeInteropTest {
    private byte[] call(Process process,int op,byte[] payload)throws Exception{
        byte[] command=new byte[1+payload.length];command[0]=(byte)op;System.arraycopy(payload,0,command,1,payload.length);
        byte[] response=null;
        try{
            Frames.write(process.getOutputStream(),command);response=Frames.read(process.getInputStream());
            assertTrue("worker operation rejected",response.length>=1&&response[0]==0);
            return Arrays.copyOfRange(response,1,response.length);
        }finally{Arrays.fill(command,(byte)0);if(response!=null)Arrays.fill(response,(byte)0);}
    }
    @Test public void actualDesktopWorkerBothMatchesDummyRoundTripAndEof()throws Exception{
        String executable=new File(System.getProperty("java.home"),"bin/java.exe").getPath();
        Process process=new ProcessBuilder(executable,"-Xmx64m","-cp",System.getProperty("teamy.worker.classpath"),"dev.teamy.broker.pairing.PipeWorker").redirectError(ProcessBuilder.Redirect.DISCARD).start();
        ScheduledExecutorService timer=Executors.newSingleThreadScheduledExecutor();timer.schedule(process::destroyForcibly,8,TimeUnit.SECONDS);
        byte[] binding="dummy-test:exact-live-session:desktop:phone".getBytes(StandardCharsets.UTF_8);
        try(VerifiedPairing phone=new VerifiedPairing(VerifiedPairing.Role.PHONE,binding)){
            call(process,0,binding);byte[] first=phone.nextHandshake();call(process,1,first);Arrays.fill(first,(byte)0);
            byte[] reply=call(process,2,new byte[0]);phone.acceptHandshake(reply);Arrays.fill(reply,(byte)0);
            byte[] finish=phone.nextHandshake();call(process,1,finish);Arrays.fill(finish,(byte)0);
            byte[] code=call(process,3,new byte[0]);
            assertTrue("comparison mismatch",Arrays.equals(code,phone.authenticationCode().getBytes(StandardCharsets.US_ASCII)));Arrays.fill(code,(byte)0);
            byte[] a=phone.confirmLocal(),b=call(process,4,new byte[0]);call(process,5,a);phone.acceptConfirmation(b);Arrays.fill(a,(byte)0);Arrays.fill(b,(byte)0);
            byte[] dummy="TEAMY-DUMMY-ONLY".getBytes(StandardCharsets.UTF_8),encrypted=phone.seal(dummy),opened=call(process,7,encrypted);
            assertArrayEquals(dummy,opened);Arrays.fill(opened,(byte)0);Arrays.fill(encrypted,(byte)0);
            encrypted=call(process,6,dummy);opened=phone.open(encrypted);assertArrayEquals(dummy,opened);Arrays.fill(dummy,(byte)0);Arrays.fill(encrypted,(byte)0);Arrays.fill(opened,(byte)0);
            process.getOutputStream().close();assertTrue("worker must stop on pipe EOF",process.waitFor(2,TimeUnit.SECONDS));
        }finally{process.destroyForcibly();timer.shutdownNow();Arrays.fill(binding,(byte)0);}
    }
}
