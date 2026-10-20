package dev.teamy.broker.dummy;
import java.util.function.Function;
import java.util.function.Consumer;
/** Serializes generation-owned presentation changes and their notification side effects. */
final class ServiceGeneration<T> {
    private long next,current;
    private T state;
    ServiceGeneration(T initial){state=initial;}
    synchronized long begin(T initial,Runnable effect){current=Math.incrementExact(next);next=current;state=initial;effect.run();return current;}
    synchronized T snapshot(){return state;}
    synchronized boolean current(long token){return token!=0&&token==current;}
    synchronized boolean update(long token,Function<T,T> change,Consumer<T> effect){
        if(!current(token))return false;
        T value=change.apply(state);if(value==null)return false;state=value;effect.accept(value);return true;
    }
    synchronized boolean end(long token,T terminal,Runnable cleanup){
        if(!current(token))return false;state=terminal;current=0;cleanup.run();return true;
    }
}