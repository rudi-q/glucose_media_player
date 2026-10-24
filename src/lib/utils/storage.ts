// localStorage that never throws. It can be blocked or full, and in some webviews even
// reading the `localStorage` property throws; the app keeps working without it, falling
// back to defaults.

export function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

// Returns whether the value was stored.
export function writeStorage(key: string, value: string): boolean {
  try {
    localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export function removeStorage(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    // Nothing stored that could be removed.
  }
}
