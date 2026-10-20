package dev.teamy.broker.dummy;
final class SupplyAdmission {
 private static final byte[] FIXTURE="teamy-fixture-input-v1".getBytes(java.nio.charset.StandardCharsets.US_ASCII);
 static boolean enqueue(byte[] bytes,boolean fixture,InputSender queue)throws Exception {
  boolean transferred=false;try{if(bytes==null||bytes.length==0||bytes.length>4096||(fixture&&!java.util.Arrays.equals(bytes,FIXTURE)))return false;transferred=queue.send(bytes);return transferred;}finally{if(!transferred&&bytes!=null)java.util.Arrays.fill(bytes,(byte)0);}
 }
}
