<script lang="ts" context="module">
	// Public API interface for external consumers
	export interface UpdateManagerAPI {
		manualCheckForUpdates(manual?: boolean): Promise<void> | null;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { browser, dev } from '$app/environment';
	import { updateStore } from '$lib/stores/updateStore';
	import type { Update, DownloadEvent } from '@tauri-apps/plugin-updater';

	// Props
  export let disableAutoCheck = false;
  export let onAutoCheckStart: (() => void) | undefined = undefined;
  export let lastAutoCheckTime: number = 0;
  
  const ONE_DAY_MS = 24 * 60 * 60 * 1000;
  
  // Check if we're in Tauri environment
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  
  // Disable updater in development mode
  const enableUpdater = !dev && isTauri;

  let checkPromise: Promise<void> | null = null;
  let isManualCheck = false;
  
  async function checkForUpdates() {
    if (!enableUpdater) {
      console.log('Updater disabled (development mode or not in Tauri environment)');
      return;
    }
    
    // For auto-checks, enforce daily limit
    if (!isManualCheck && lastAutoCheckTime > 0) {
      const timeSinceLastCheck = Date.now() - lastAutoCheckTime;
      if (timeSinceLastCheck < ONE_DAY_MS) {
        console.log(`Auto-check skipped - last check was ${Math.floor(timeSinceLastCheck / 1000)}s ago`);
        return;
      }
    }

    try {
      updateStore.setChecking(true);
      
      // Notify parent that auto-check is starting
      if (!isManualCheck && onAutoCheckStart) {
        onAutoCheckStart();
      }
      
      // Dynamic imports for Tauri plugins (only available in Tauri environment)
      const { check } = await import('@tauri-apps/plugin-updater');
      const { relaunch } = await import('@tauri-apps/plugin-process');

      console.log('Checking for updates...');
      const update = await check();
      
      if (update) {
        console.log(`Found update ${update.version} from ${update.date} with notes ${update.body}`);
        
        // setAvailable now clears checking flag automatically
        updateStore.setAvailable(true, update.version, update.date, update.body);
        
        // Auto-start download and installation
        await downloadAndInstallUpdate(update, relaunch);
      } else {
        console.log('No updates available');
        // Only show "up to date" message for manual checks
        if (isManualCheck) {
          // setUpToDate now clears availability and metadata automatically
          updateStore.setUpToDate(true);
        } else {
          // Clear any previous update state to avoid showing stale data
          // setAvailable now clears checking flag automatically
          updateStore.setAvailable(false);
        }
      }
    } catch (error) {
      console.error('Error checking for updates:', error);
      
      // Clear any stale available state
      updateStore.setAvailable(false);
      updateStore.setDownloading(false);
      
      // Check if this is a network connectivity issue
      const errorString = error?.toString() || '';
      const isNetworkError = errorString.includes('error sending request') || 
                           errorString.includes('network') || 
                           errorString.includes('timeout') ||
                           errorString.includes('connection') ||
                           errorString.includes('dns') ||
                           errorString.includes('offline');
      
      if (isNetworkError) {
        console.log('Update check skipped - no internet connection');
        // Don't show error notification for network issues - just log and continue
      } else {
        // Only show error for non-network related issues
        updateStore.setError(`Failed to check for updates: ${error}`);
      }
    } finally {
      // Always reset both flags regardless of success/failure
      updateStore.setChecking(false);
      updateStore.setDownloading(false);
    }
  }

  async function downloadAndInstallUpdate(update: Update, relaunch: () => Promise<void>) {
    try {
      updateStore.setDownloading(true);
      
      let downloaded = 0;
      let contentLength = 0;
      
      // Download and install the update with progress tracking
      await update.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0;
            updateStore.setProgress(0, contentLength);
            console.log(`Started downloading ${contentLength} bytes`);
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            updateStore.setProgress(downloaded, contentLength);
            console.log(`Downloaded ${downloaded} from ${contentLength}`);
            break;
          case 'Finished':
            // setCompleted now clears downloading flag automatically
            updateStore.setCompleted(true);
            console.log('Download finished');
            break;
        }
      });

      console.log('Update installed');
      
      // Give user a moment to see completion before restarting
      setTimeout(async () => {
        await relaunch();
      }, 2000);
      
    } catch (error) {
      console.error('Error downloading/installing update:', error);
      // setError automatically clears downloading flag
      updateStore.setError(`Failed to install update: ${error}`);
    }
  }

  onMount(() => {
    if (browser && enableUpdater && !disableAutoCheck) {
      // Check for updates on app start (automatic, not manual)
      console.log('Auto-checking for updates on app start');
      manualCheckForUpdates(false);
    } else if (disableAutoCheck) {
      console.log('Auto-check disabled - updates will only be checked manually');
    }
    
    return () => {
      // Cleanup if needed
    };
  });

  // Expose manual check function for external use
  export function manualCheckForUpdates(manual: boolean = true): Promise<void> | null {
    // Prevent concurrent checks - return null if already checking
    if (checkPromise) {
      console.log('Update check already in progress, ignoring request');
      return null;
    }
    
    isManualCheck = manual;
    // Create new promise and attach finalizer to clear it when settled
    checkPromise = checkForUpdates().finally(() => {
      checkPromise = null;
      isManualCheck = false;
    });
    
    return checkPromise;
  }
</script>

<!-- This component has no UI - it only handles the update logic -->
<!-- The UI is handled by the UpdateNotification component -->
