package dev.teamy.broker.dummy;

/** Immutable dummy build boundary. A release flag cannot enable real credentials. */
public final class DummyPolicy {
    public static final String VALUE = "TEAMY-DUMMY-ONLY";
    public static final String DESKTOP = "Desktop (dummy session)";
    public static final String PHONE = "Phone (dummy session)";
    public static final String CONSUMER = "teamy-discord-dummy";
    public static final String REFERENCE = "op://Synthetic/teamy-discord/token";
    public static final String PURPOSE = "Dummy Discord authentication demonstration; no API call";
    private DummyPolicy() {}
    public static boolean allowedPrefix(CharSequence text) {
        if (text.length() > VALUE.length()) return false;
        for (int i = 0; i < text.length(); i++) if (text.charAt(i) != VALUE.charAt(i)) return false;
        return true;
    }
    public static boolean allowedValue(CharSequence text) {
        return text.length() == VALUE.length() && allowedPrefix(text);
    }
}
