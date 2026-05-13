import { getCurrentWindow } from '@tauri-apps/api/window';

const APP = 'Glucose';

export function setWindowTitle(page: string | null): void {
  getCurrentWindow().setTitle(page ? `${page} - ${APP}` : APP);
}

export function galleryPageTitle(
  sortBy: 'watched' | 'added',
  filterBy: 'all' | 'video' | 'audio'
): string {
  const sort = sortBy === 'watched' ? 'Recently Watched' : 'Recently Added';
  const filter = filterBy === 'video' ? ' Videos' : filterBy === 'audio' ? ' Audio' : '';
  return `${sort}${filter}`;
}
