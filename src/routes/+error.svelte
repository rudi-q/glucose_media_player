<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import ErrorScreen from "$lib/components/ErrorScreen.svelte";

  function safeDecode(value: string): string {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  }
</script>

<ErrorScreen
  status={page.status}
  message={page.error?.message ?? "Unknown error"}
  screen={safeDecode(page.url.pathname)}
  onHome={() => goto("/", { replaceState: true })}
  onRetry={() => location.reload()}
  retryLabel="Reload Glucose"
/>
