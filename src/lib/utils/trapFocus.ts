// Svelte action for modal dialogs: moves focus into the dialog, keeps Tab and focus
// inside it while it is open, and returns focus to where it was when it closes. The
// dialog element needs tabindex="-1" so it can take focus itself.
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

  function getFocusable() {
    return Array.from(node.querySelectorAll<HTMLElement>(focusableSelector))
      .filter((el) => !el.hasAttribute("disabled") && el.offsetParent !== null);
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
    if (event.target instanceof Node && !node.contains(event.target)) {
      focusModal();
    }
  }

  node.addEventListener("keydown", handleKeydown);
  document.addEventListener("focusin", handleFocusIn);
  queueMicrotask(focusModal);

  return {
    destroy() {
      node.removeEventListener("keydown", handleKeydown);
      document.removeEventListener("focusin", handleFocusIn);
      previous?.focus();
    },
  };
}
