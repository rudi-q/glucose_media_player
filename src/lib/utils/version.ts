/// <reference types="vite/client" />

// Vite injects this at build time
declare const __APP_VERSION__: string;

/**
 * Get the current application version
 * @returns The semantic version string (e.g., "2.3.0")
 */
export function getAppVersion(): string {
  return typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0';
}

/**
 * Get a formatted version string with "v" prefix
 * @returns Formatted version string (e.g., "v2.3.0")
 */
export function getFormattedVersion(): string {
  return `v${getAppVersion()}`;
}
