package dev.teamy.broker.dummy;

/** Validate bootstrap before changing to pending UI or scheduling any network work. */
public final class BootstrapPolicy {
    private BootstrapPolicy() {}

    public static String error(CharSequence key, CharSequence pin, boolean tls) {
        if (key.length() == 0) return "Enter the temporary desktop session key before Refresh.";
        if (!hex256(key)) return "Session key must contain exactly 64 hexadecimal characters.";
        if (tls && pin.length() == 0) return "Enter the temporary desktop TLS certificate pin before Refresh.";
        if (tls && !hex256(pin)) return "TLS certificate pin must contain exactly 64 hexadecimal characters.";
        return null;
    }

    private static boolean hex256(CharSequence value) {
        if (value.length() != 64) return false;
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (!(c >= '0' && c <= '9') && !(c >= 'a' && c <= 'f') && !(c >= 'A' && c <= 'F')) return false;
        }
        return true;
    }
}
