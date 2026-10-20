package dev.teamy.broker.dummy;

/** Pure event/geometry policy; stores no view, input text or authentication state. */
public final class FocusReveal {
    private boolean pending;
    private int top = Integer.MIN_VALUE, bottom = Integer.MIN_VALUE, range = Integer.MIN_VALUE;

    public void request() { pending = true; }
    public void clear() { pending = false; top = bottom = range = Integer.MIN_VALUE; }

    public void viewport(int top, int bottom, int range) {
        if (this.top != top || this.bottom != bottom || this.range != range) request();
        this.top = top; this.bottom = bottom; this.range = range;
    }

    /** Defer consumption until layout is complete; later geometry can request again. */
    public boolean consume(boolean laidOut) {
        if (!laidOut || !pending) return false;
        pending = false;
        return true;
    }

    public static int target(int current, int maximum, int fieldTop, int fieldBottom,
        int visibleTop, int visibleBottom, int margin) {
        int limit = Math.max(0, maximum);
        if (visibleBottom <= visibleTop || fieldBottom <= fieldTop) return Math.max(0, Math.min(limit, current));
        int space = Math.max(0, (visibleBottom - visibleTop - (fieldBottom - fieldTop)) / 2);
        int gap = Math.max(0, Math.min(margin, space));
        long delta = 0;
        if (fieldBottom - fieldTop > visibleBottom - visibleTop) delta = (long)fieldTop - visibleTop;
        else if (fieldBottom + gap > visibleBottom) delta = (long)fieldBottom + gap - visibleBottom;
        else if (fieldTop - gap < visibleTop) delta = (long)fieldTop - gap - visibleTop;
        return (int)Math.max(0L, Math.min((long)limit, (long)current + delta));
    }
}
