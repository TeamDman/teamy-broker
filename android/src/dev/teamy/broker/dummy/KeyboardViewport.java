package dev.teamy.broker.dummy;

import android.app.Activity;
import android.graphics.Insets;
import android.graphics.Rect;
import android.os.Build;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewTreeObserver;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowManager;
import android.widget.EditText;
import android.widget.ScrollView;
import java.util.List;

/** Reconciles focus against the actual visible viewport after layout/IME events. */
final class KeyboardViewport implements ViewTreeObserver.OnPreDrawListener {
    private final ScrollView scroll;
    private final FocusReveal reveal = new FocusReveal();
    private final Rect windowVisible = new Rect();
    private final int[] viewportPosition = new int[2], fieldPosition = new int[2];
    private boolean active;

    private KeyboardViewport(ScrollView scroll) { this.scroll = scroll; }

    static KeyboardViewport install(Activity activity, ScrollView scroll) {
        KeyboardViewport controller = new KeyboardViewport(scroll);
        scroll.setFillViewport(true);
        scroll.setClipToPadding(true);
        scroll.setSaveEnabled(false);
        scroll.setSaveFromParentEnabled(false);
        activity.getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        if (Build.VERSION.SDK_INT >= 30) {
            activity.getWindow().setDecorFitsSystemWindows(false);
            scroll.setOnApplyWindowInsetsListener((view, insets) -> {
                controller.insets(insets);
                return WindowInsets.CONSUMED;
            });
            scroll.setWindowInsetsAnimationCallback(new WindowInsetsAnimation.Callback(WindowInsetsAnimation.Callback.DISPATCH_MODE_STOP) {
                @Override public WindowInsets onProgress(WindowInsets insets, List<WindowInsetsAnimation> running) {
                    controller.insets(insets);
                    return insets;
                }
                @Override public void onEnd(WindowInsetsAnimation animation) {
                    WindowInsets finalInsets = scroll.getRootWindowInsets();
                    if (finalInsets != null) controller.insets(finalInsets);
                    controller.request();
                }
            });
        } else {
            scroll.setFitsSystemWindows(true);
        }
        scroll.addOnLayoutChangeListener((view, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) -> controller.request());
        scroll.getViewTreeObserver().addOnGlobalFocusChangeListener((oldFocus, newFocus) -> controller.request());
        scroll.getViewTreeObserver().addOnPreDrawListener(controller);
        activity.setContentView(scroll, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        return controller;
    }

    private void insets(WindowInsets insets) {
        if (Build.VERSION.SDK_INT < 30) return;
        Insets safe = insets.getInsets(WindowInsets.Type.systemBars()
            | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
        // Use the union's maximum per edge, never add overlapping IME/navigation space.
        scroll.setPadding(safe.left, safe.top, safe.right, safe.bottom);
        request();
    }

    void watch(EditText field) {
        field.setOnFocusChangeListener((view, focused) -> { if (focused) request(); });
        // Covers tapping an already focused field to open the IME, with no focus change.
        field.setOnTouchListener((view, event) -> {
            if (event.getActionMasked() == MotionEvent.ACTION_UP) request();
            return false;
        });
        field.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence ignored, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence ignored, int start, int before, int count) {}
            @Override public void afterTextChanged(Editable ignored) { if (field.hasFocus()) request(); }
        });
    }

    private void request() {
        if (!active) return;
        reveal.request();
        scroll.postInvalidateOnAnimation();
    }

    void resume() { active = true; request(); scroll.requestApplyInsets(); }
    void pause() { active = false; reveal.clear(); }

    @Override public boolean onPreDraw() {
        if (!active || scroll.getChildCount() == 0) return true;
        scroll.getLocationOnScreen(viewportPosition);
        scroll.getWindowVisibleDisplayFrame(windowVisible);
        int top = Math.max(viewportPosition[1] + scroll.getPaddingTop(), windowVisible.top);
        int bottom = Math.min(viewportPosition[1] + scroll.getHeight() - scroll.getPaddingBottom(), windowVisible.bottom);
        int maximum = Math.max(0, scroll.getChildAt(0).getHeight()
            - (scroll.getHeight() - scroll.getPaddingTop() - scroll.getPaddingBottom()));
        reveal.viewport(top, bottom, maximum);
        if (!reveal.consume(scroll.isLaidOut() && !scroll.isLayoutRequested())) return true;
        View focused = scroll.findFocus();
        if (focused == null || focused == scroll || !focused.isShown()) return true;
        focused.getLocationOnScreen(fieldPosition);
        int margin = (int)(16 * scroll.getResources().getDisplayMetrics().density);
        int target = FocusReveal.target(scroll.getScrollY(), maximum, fieldPosition[1],
            fieldPosition[1] + focused.getHeight(), top, bottom, margin);
        if (target != scroll.getScrollY()) scroll.scrollTo(scroll.getScrollX(), target);
        return true;
    }
}
