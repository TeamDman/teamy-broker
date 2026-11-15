package dev.teamy.broker.dummy;

/** Configuration reads use committed storage; changing peers never changes saved trust. */
final class DurablePhoneConfig {
    interface Store {
        PrivatePhoneConfig read() throws Exception;
        void commit(PrivatePhoneConfig profile) throws Exception;
    }

    private final Store store;

    DurablePhoneConfig(Store store) {
        this.store = store;
    }

    PrivatePhoneConfig load() throws Exception {
        PrivatePhoneConfig profile = store.read();
        if (profile == null) throw new Exception("phone configuration unavailable");
        return profile;
    }

    void save(PrivatePhoneConfig profile, boolean paired) throws Exception {
        if (profile == null) throw new Exception("phone configuration rejected");
        if (paired) {
            if (!profile.samePeer(store.read())) {
                throw new Exception("forget phone identity before changing connection settings");
            }
            return;
        }
        store.commit(profile);
    }
}
