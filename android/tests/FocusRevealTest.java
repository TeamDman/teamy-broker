import dev.teamy.broker.dummy.FocusReveal;

public final class FocusRevealTest {
    private static int checks;
    public static void main(String[] args) {
        // Field looks visible against full height900, but is covered by IME above600.
        equal(236, FocusReveal.target(0, 900, 760, 820, 24, 600, 16));
        equal(0, FocusReveal.target(0, 900, 760, 820, 24, 900, 16));
        equal(16, FocusReveal.target(236, 900, -180, -120, 24, 600, 16));
        equal(100, FocusReveal.target(100, 900, 40, 100, 24, 600, 16));
        equal(900, FocusReveal.target(850, 900, 650, 710, 24, 600, 16));
        equal(0, FocusReveal.target(50, 900, -100, -40, 24, 600, 16));
        equal(100, FocusReveal.target(100, 900, 40, 100, 600, 600, 16));
        equal(110, FocusReveal.target(100, 900, 30, 300, 20, 120, 16));
        equal(136, FocusReveal.target(100, 900, 100, 160, 24, 140, 16));
        equal(120, FocusReveal.target(100, 900, 100, 160, 24, 140, 0));

        FocusReveal events = new FocusReveal();
        events.viewport(24, 900, 500); events.request(); // Focus before IME.
        yes(!events.consume(false)); // Dirty layout may not consume the request.
        yes(events.consume(true));
        yes(!events.consume(true));
        events.viewport(24, 600, 800); // IME arrives after first reveal, same focus.
        yes(events.consume(true));
        events.viewport(24, 550, 850); // Subsequent animation frame.
        yes(!events.consume(false)); yes(events.consume(true));
        events.viewport(24, 550, 850); // Stable geometry/manual scroll: no snap back.
        yes(!events.consume(true));
        events.request(); // Typing or tapping already focused field requests correction.
        yes(events.consume(true));
        events.viewport(24, 900, 500); // Keyboard hides, geometry rechecked.
        yes(events.consume(true));
        events.request(); events.clear(); // Pause/rotation loses pending request.
        yes(!events.consume(true));
        events.viewport(24, 400, 1000); // New Activity/landscape viewport.
        yes(events.consume(true));
        System.out.println("Focus viewport geometry and event-order checks passed: " + checks);
    }
    private static void equal(int expected, int actual) { yes(expected == actual); }
    private static void yes(boolean condition) { checks++; if (!condition) throw new AssertionError("focus geometry/event-order check " + checks); }
}
