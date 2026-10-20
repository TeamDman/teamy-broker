package dev.teamy.broker.dummy;
/** Selection is always read from committed storage, never a staging cache. */
final class DurableIdentitySelection {
    static final class Selection {
        final String alias,pin,previousAlias,previousPin;
        Selection(String alias,String pin,String previousAlias,String previousPin){this.alias=alias;this.pin=pin;this.previousAlias=previousAlias;this.previousPin=previousPin;}
        boolean paired(){return alias!=null&&pin!=null;}
    }
    interface Store {Selection read();void commit(Selection next)throws Exception;}
    private final Store store;
    DurableIdentitySelection(Store store){this.store=store;}
    Selection active(){return store.read();}
    void replace(String alias,String pin)throws Exception {
        Selection old=active();
        Selection next=new Selection(alias,pin,old.paired()&&!alias.equals(old.alias)?old.alias:old.previousAlias,old.paired()&&!alias.equals(old.alias)?old.pin:old.previousPin);
        store.commit(next);
    }
    void forget()throws Exception{store.commit(new Selection(null,null,null,null));}
}