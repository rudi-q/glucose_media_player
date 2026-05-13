import { convertFileSrc } from "@tauri-apps/api/core";

export const thumbnailCache = new Map<string, string>();
const thumbnailPromises = new Map<string, Promise<string>>();
const thumbnailQueue: Array<{ run: () => void; cancel: () => void }> = [];
let activeThumbnailJobs = 0;
const MAX_THUMBNAIL_JOBS = 2;
let thumbnailCacheVersion = 0;

export function scheduleThumbnailJob(job: () => Promise<string>): Promise<string> {
  return new Promise((resolve) => {
    const run = () => {
      activeThumbnailJobs += 1;
      job()
        .then(resolve)
        .catch(() => resolve(''))
        .finally(() => {
          activeThumbnailJobs -= 1;
          thumbnailQueue.shift()?.run();
        });
    };

    if (activeThumbnailJobs < MAX_THUMBNAIL_JOBS) {
      run();
    } else {
      thumbnailQueue.push({ run, cancel: () => resolve('') });
    }
  });
}

function createThumbnail(
  videoPath: string,
  seekTime: number | undefined,
  hasSeek: boolean,
  cacheKey: string,
  isDestroyed: () => boolean,
  capturedVersion: number
): Promise<string> {
  return new Promise((resolve) => {
    const video = document.createElement('video');
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    let settled = false;
    const timeout = setTimeout(() => settle(''), 8000);

    function cleanup() {
      clearTimeout(timeout);
      video.onloadedmetadata = null;
      video.onseeked = null;
      video.onerror = null;
      try {
        video.removeAttribute('src');
        video.load();
      } catch {}
    }

    function settle(thumbnail: string) {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(thumbnail);
    }

    function capture() {
      try {
        const targetWidth = 320;
        const aspectRatio = video.videoWidth / video.videoHeight;

        if (!Number.isFinite(aspectRatio) || aspectRatio <= 0) {
          settle('');
          return;
        }

        canvas.width = targetWidth;
        canvas.height = Math.round(targetWidth / aspectRatio);

        ctx!.drawImage(video, 0, 0, canvas.width, canvas.height);
        canvas.toBlob((blob) => {
          if (!blob) {
            settle('');
            return;
          }
          const url = URL.createObjectURL(blob);
          if (isDestroyed() || thumbnailCacheVersion !== capturedVersion) {
            URL.revokeObjectURL(url);
            settle('');
            return;
          }
          thumbnailCache.set(cacheKey, url);
          settle(url);
        }, 'image/jpeg', 0.75);
      } catch (err) {
        if (import.meta.env.DEV) {
          console.log('Thumbnail generation skipped:', videoPath, err);
        }
        settle('');
      }
    }

    if (!ctx) {
      settle('');
      return;
    }

    video.muted = true;
    video.preload = 'metadata';
    video.playsInline = true;
    video.crossOrigin = 'anonymous';

    video.onloadedmetadata = () => {
      const defaultTime = Number.isFinite(video.duration) ? Math.min(1, video.duration * 0.1) : 0;
      const targetTime = hasSeek ? seekTime! : defaultTime;
      if (targetTime <= 0) {
        capture();
        return;
      }
      try {
        video.currentTime = targetTime;
      } catch {
        capture();
      }
    };

    video.onseeked = capture;

    video.onerror = () => settle('');
    video.src = convertFileSrc(videoPath);
  });
}

export async function generateThumbnail(
  videoPath: string,
  seekTime?: number,
  isDestroyed: () => boolean = () => false
): Promise<string> {
  const hasSeek = seekTime != null && seekTime > 0;
  const cacheKey = hasSeek ? `${videoPath}@${Math.floor(seekTime!)}` : videoPath;
  
  if (thumbnailCache.has(cacheKey)) {
    return thumbnailCache.get(cacheKey)!;
  }
  if (thumbnailPromises.has(cacheKey)) {
    return thumbnailPromises.get(cacheKey)!;
  }

  const capturedVersion = thumbnailCacheVersion;
  const promise = scheduleThumbnailJob(() => createThumbnail(videoPath, seekTime, hasSeek, cacheKey, isDestroyed, capturedVersion))
    .finally(() => thumbnailPromises.delete(cacheKey));
  
  thumbnailPromises.set(cacheKey, promise);
  return promise;
}

export function clearThumbnailCache() {
  thumbnailCacheVersion++;
  for (const item of thumbnailQueue) item.cancel();
  thumbnailQueue.length = 0;
  thumbnailPromises.clear();
  for (const thumbnail of thumbnailCache.values()) {
    if (thumbnail.startsWith('blob:')) {
      URL.revokeObjectURL(thumbnail);
    }
  }
  thumbnailCache.clear();
}
