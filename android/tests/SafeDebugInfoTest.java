package dev.teamy.broker.dummy;
public final class SafeDebugInfoTest {
 public static void main(String[] args){
  String secret="token-secret 192.0.2.11 PAIR-CODE certificate";
  String[] input={secret,"Connected · paired desktop "+secret,"Disconnected · retrying over Tailscale · EXCHANGE_READ/"+secret,"Disconnected · retrying over Tailscale · "+secret};
  for(String s:input){String v=SafeDebugInfo.format(secret,19,s);if(v.contains(secret)||v.contains("100.123")||v.contains("PAIR-CODE"))throw new AssertionError("raw diagnostic leak");}
  String ok=SafeDebugInfo.format("0.19",19,"Disconnected · retrying over Tailscale · EXCHANGE_READ/TIMEOUT");if(!ok.contains("Version: 0.19")||!ok.contains("Connection: retrying")||!ok.contains("Failure: EXCHANGE_READ/TIMEOUT"))throw new AssertionError("fixed diagnostic lost");
  if(!SafeDebugInfo.format("0.19",19,"Connected · paired desktop").contains("Connection: connected"))throw new AssertionError("connected");
  if(!SafeDebugInfo.format("0.20",20,"Stopped",true).contains("Mode: private release; explicit request permission required"))throw new AssertionError("release mode");
  System.out.println("Safe debug projection: hostile status/version redacted; only fixed state/enum fields copied");
 }
}
