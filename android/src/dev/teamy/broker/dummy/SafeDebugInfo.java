package dev.teamy.broker.dummy;
/** Chat-safe diagnostic projection: never emits raw status or request/identity data. */
final class SafeDebugInfo {
 static String format(String version,long code,String status){return format(version,code,status,false);}
 static String format(String version,long code,String status,boolean privateRelease){
  String safeVersion=version!=null&&version.matches("[0-9]+(?:\\.[0-9]+){1,3}")?version:"unknown";
  String state="unknown",failure="none";
  if("Stopped".equals(status))state="stopped";
  else if("Connecting".equals(status))state="connecting";
  else if("Connected · paired desktop".equals(status))state="connected";
  else if("Enable notifications and pair first".equals(status))state="permission-or-pairing-required";
  else if("Foreground service unavailable; open app and check permissions".equals(status))state="foreground-service-unavailable";
  else if("Notifications disabled; connection stopped".equals(status))state="notifications-disabled";
  else if(status!=null&&status.startsWith("Disconnected · retrying over Tailscale · ")){
   state="retrying";String tail=status.substring("Disconnected · retrying over Tailscale · ".length());
   if("UNAVAILABLE".equals(tail))failure=tail;
   else {String[] parts=tail.split("/",-1);if(parts.length==2)try{failure=ConnectionFailure.Phase.valueOf(parts[0]).name()+"/"+ConnectionFailure.Category.valueOf(parts[1]).name();}catch(IllegalArgumentException ignored){failure="unknown";}}
  }
  return "Teamy Broker\nVersion: "+safeVersion+"\nBuild: "+(code>=0?Long.toString(code):"unknown")+"\nMode: "+(privateRelease?"private release; explicit request permission required":"no-vault test")+"\nConnection: "+state+"\nProtocol: UKEY2 / TLS 1.3\nFailure: "+failure+"\nNo credentials, pairing codes or device identifiers included.";
 }
}
