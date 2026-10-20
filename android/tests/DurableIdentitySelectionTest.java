package dev.teamy.broker.dummy;
public final class DurableIdentitySelectionTest {
    static final class Disk implements DurableIdentitySelection.Store {
        DurableIdentitySelection.Selection committed=new DurableIdentitySelection.Selection("old-key","old-pin",null,null),memory=committed;
        boolean fail;
        public DurableIdentitySelection.Selection read(){return committed;}
        public void commit(DurableIdentitySelection.Selection next)throws Exception{memory=next;if(fail)throw new Exception("disk failure after memory mutation");committed=next;}
    }
    static void check(boolean value){if(!value)throw new AssertionError();}
    public static void main(String[] args)throws Exception {
        Disk disk=new Disk();DurableIdentitySelection live=new DurableIdentitySelection(disk);disk.fail=true;
        try{live.replace("new-key","new-pin");throw new AssertionError();}catch(Exception expected){}
        check(disk.memory.alias.equals("new-key"));check(live.active().alias.equals("old-key")&&live.active().pin.equals("old-pin"));
        // Process death drops mutated in-memory state; committed selection stays old.
        disk.memory=disk.committed;live=new DurableIdentitySelection(disk);check(live.active().alias.equals("old-key"));
        disk.fail=false;live.replace("new-key","new-pin");check(live.active().alias.equals("new-key")&&live.active().pin.equals("new-pin"));check(live.active().previousAlias.equals("old-key")&&live.active().previousPin.equals("old-pin"));
        live=new DurableIdentitySelection(disk);check(live.active().alias.equals("new-key"));
        disk.fail=true;try{live.forget();throw new AssertionError();}catch(Exception expected){}check(live.active().paired());
        disk.fail=false;live.forget();check(!new DurableIdentitySelection(disk).active().paired());
        System.out.println("Durable identity: memory mutation, disk failure, restart, retry, retained prior identity and tombstone passed");
    }
}