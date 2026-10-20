package dev.teamy.broker.dummy;

public final class PrivatePhoneConfigTest {
    public static void main(String[] args) throws Exception {
        boolean expected = Boolean.parseBoolean(args[0]);
        if (PrivatePhoneConfig.CONFIGURED != expected) throw new AssertionError("configuration state");
        try {
            PrivatePhoneConfig.requireConfigured();
            if (!expected) throw new AssertionError("unconfigured build accepted");
        } catch (Exception rejected) {
            if (expected) throw rejected;
        }
    }
}
