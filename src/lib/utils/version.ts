/// <reference types="vite/client" />

// Vite injects this at build time
declare const __APP_VERSION__: string | undefined;

/**
 * Get the current application version
 * Fallback chain: __APP_VERSION__ → import.meta.env.VITE_APP_VERSION → "0.0.0"
 * @returns The semantic version string (e.g., "2.3.0")
 */
export function getAppVersion(): string {
  // First try build-time injected version
  if (typeof __APP_VERSION__ !== 'undefined' && __APP_VERSION__) {
    return __APP_VERSION__;
  }
  
  // Fall back to environment variable (for dev/test)
  if (typeof import.meta.env.VITE_APP_VERSION === 'string' && import.meta.env.VITE_APP_VERSION) {
    return import.meta.env.VITE_APP_VERSION;
  }
  
  // Final fallback for test environments
  return '0.0.0';
}

/**
 * Get a formatted version string with "v" prefix
 * @returns Formatted version string (e.g., "v2.3.0")
 */
export function getFormattedVersion(): string {
  return `v${getAppVersion()}`;
}
