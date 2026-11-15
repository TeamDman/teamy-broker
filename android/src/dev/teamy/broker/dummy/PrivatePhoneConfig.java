package dev.teamy.broker.dummy;

/** Immutable, validated runtime configuration. Never include contents in diagnostics. */
final class PrivatePhoneConfig {
    static final int PORT = 43188;
    final String desktop, phone, desktopNode, phoneNode;

    PrivatePhoneConfig(String desktop, String phone, String desktopNode, String phoneNode) throws Exception {
        this.desktop = address(desktop);
        this.phone = address(phone);
        if (this.desktop.equals(this.phone)) throw new Exception("phone configuration rejected");
        this.desktopNode = node(desktopNode);
        this.phoneNode = node(phoneNode);
    }
    private static String address(String value) throws Exception {
        if (value == null || !value.matches("(0|[1-9][0-9]{0,2})(\\.(0|[1-9][0-9]{0,2})){3}")) throw new Exception("phone configuration rejected");
        String[] parts = value.split("\\.");
        int[] octets = new int[4];
        for (int i=0;i<4;i++) { octets[i]=Integer.parseInt(parts[i]); if(octets[i]>255)throw new Exception("phone configuration rejected"); }
        if (octets[0]!=100 || octets[1]<64 || octets[1]>127) throw new Exception("phone configuration rejected");
        return value;
    }
    private static String node(String value) throws Exception {
        if(value==null || !value.matches("[A-Za-z0-9._-]{1,255}"))throw new Exception("phone configuration rejected");
        return value;
    }
    boolean samePeer(PrivatePhoneConfig other) {
        return other != null && desktop.equals(other.desktop) && phone.equals(other.phone)
                && desktopNode.equals(other.desktopNode) && phoneNode.equals(other.phoneNode);
    }
    @Override public String toString(){return "PrivatePhoneConfig([redacted])";}
}
