package dev.teamy.broker.dummy;

public final class DurablePhoneConfigTest {
    private static final class Store implements DurablePhoneConfig.Store {
        PrivatePhoneConfig committed;
        boolean failCommit, failRead;
        int writes;

        @Override public PrivatePhoneConfig read() throws Exception {
            if (failRead) throw new Exception("configuration storage unavailable");
            return committed;
        }

        @Override public void commit(PrivatePhoneConfig profile) throws Exception {
            writes++;
            if (failCommit) throw new Exception("configuration publication failed");
            committed = profile;
        }
    }

    private interface Action { void run() throws Exception; }

    private static void rejected(Action action, String message) throws Exception {
        try {
            action.run();
            throw new AssertionError(message);
        } catch (AssertionError failure) {
            throw failure;
        } catch (Exception expected) {
            // The caller receives failure, without a replacement configuration.
        }
    }

    public static void main(String[] args) throws Exception {
        PrivatePhoneConfig first = new PrivatePhoneConfig("100.64.0.1", "100.64.0.2", "desktop-one", "phone-one");
        PrivatePhoneConfig same = new PrivatePhoneConfig("100.64.0.1", "100.64.0.2", "desktop-one", "phone-one");
        PrivatePhoneConfig second = new PrivatePhoneConfig("100.64.0.3", "100.64.0.4", "desktop-two", "phone-two");
        Store store = new Store();
        DurablePhoneConfig configuration = new DurablePhoneConfig(store);
        rejected(configuration::load, "missing profile accepted");
        rejected(() -> configuration.save(first, true), "paired identity accepted an unknown profile");
        if (store.writes != 0) throw new AssertionError("paired save wrote configuration");

        configuration.save(first, false);
        if (!configuration.load().samePeer(first)) throw new AssertionError("initial profile unavailable");
        configuration.save(same, true);
        if (store.writes != 1) throw new AssertionError("unchanged paired profile was rewritten");
        rejected(() -> configuration.save(second, true), "paired peer changed");
        if (store.writes != 1 || !configuration.load().samePeer(first)) throw new AssertionError("rejected peer changed storage");

        store.failRead = true;
        rejected(() -> configuration.save(same, true), "unavailable paired profile accepted");
        if (store.writes != 1) throw new AssertionError("failed read reached publication");
        store.failRead = false;
        store.failCommit = true;
        rejected(() -> configuration.save(second, false), "failed publication reported success");
        if (!configuration.load().samePeer(first)) throw new AssertionError("failed publication exposed staged profile");
        if (!new DurablePhoneConfig(store).load().samePeer(first)) throw new AssertionError("reloaded profile lost committed configuration");

        store.failCommit = false;
        configuration.save(second, false);
        if (!new DurablePhoneConfig(store).load().samePeer(second)) throw new AssertionError("replacement profile unavailable after reload");
    }
}
