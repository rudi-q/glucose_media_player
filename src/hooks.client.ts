import type { HandleClientError } from "@sveltejs/kit";

// SvelteKit hides unexpected errors behind a generic "Internal Error". Glucose runs
// locally, so pass the real message to the error page, where it is useful in bug
// reports, and log the full error for the dev console.
export const handleError: HandleClientError = ({ error, status, message }) => {
  console.error(`[glucose] ${status} ${message}`, error);
  return {
    message: error instanceof Error ? error.message : message,
  };
};
