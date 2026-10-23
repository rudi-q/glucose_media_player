// Svelte action for modal dialogs: moves focus into the dialog, keeps Tab and focus
// inside it while it is open, and returns focus to where it was when it closes. The
// dialog element needs tabindex="-1" so it can take focus itself.

// Open dialogs, most recent last. Only the most recent one keeps focus, so a dialog that
// is still fading out does not pull focus back from one opened after it.
const openDialogs: HTMLElement[] = [];

export function trapFocus(node: HTMLElement) {
  const previous = document.activeElement as HTMLElement | null;
  const focusableSelector = [
    "a[href]",
    "button:not([disabled])",
    "input:not([disabled])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "[tabindex]:not([tabindex='-1'])",
  ].join(",");

  // Visible controls; getClientRects also counts position: fixed elements, which have no
  // offsetParent.
  function getFocusable() {
    return Array.from(node.querySelectorAll<HTMLElement>(focusableSelector))
      .filter((el) => !el.hasAttribute("disabled") && el.getClientRects().length > 0);
  }

  function focusModal() {
    node.focus();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== "Tab") return;

    const focusable = getFocusable();
    if (focusable.length === 0) {
      event.preventDefault();
      focusModal();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;

    if (event.shiftKey && (active === first || active === node)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function handleFocusIn(event: FocusEvent) {
    if (openDialogs[openDialogs.length - 1] !== node) return;
    if (event.target instanceof Node && !node.contains(event.target)) {
      focusModal();
    }
  }

  openDialogs.push(node);
  node.addEventListener("keydown", handleKeydown);
  document.addEventListener("focusin", handleFocusIn);
  queueMicrotask(focusModal);

  return {
    destroy() {
      const index = openDialogs.indexOf(node);
      if (index !== -1) openDialogs.splice(index, 1);
      node.removeEventListener("keydown", handleKeydown);
      document.removeEventListener("focusin", handleFocusIn);
      // Focus goes back only if it is still in this dialog (or nowhere): if something
      // else, such as a dialog opened from this one, has it now, it stays there.
      const active = document.activeElement;
      if (!active || active === document.body || node.contains(active)) previous?.focus();
    },
  };
}
