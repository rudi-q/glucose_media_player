export interface VttCue {
  start: number;
  end: number;
  text: string;
}

export function parseTimestamp(ts: string): number {
  const parts = ts.trim().split(':');
  if (parts.length === 3) {
    const hours = parseInt(parts[0], 10);
    const minutes = parseInt(parts[1], 10);
    const seconds = parseFloat(parts[2]);
    return hours * 3600 + minutes * 60 + seconds;
  }
  const minutes = parseInt(parts[0], 10);
  const seconds = parseFloat(parts[1]);
  return minutes * 60 + seconds;
}

export function parseVtt(content: string): VttCue[] {
  const normalized = content.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  const blocks = normalized.split(/\n\n+/);
  const cues: VttCue[] = [];

  for (const block of blocks) {
    const lines = block.trim().split('\n');
    if (lines.length === 0) continue;

    if (lines[0].startsWith('WEBVTT') || lines[0].startsWith('NOTE')) continue;

    let arrowIndex = lines.findIndex((l) => l.includes('-->'));
    if (arrowIndex === -1) continue;

    const timingLine = lines[arrowIndex];
    const arrowPos = timingLine.indexOf('-->');
    const startRaw = timingLine.slice(0, arrowPos).trim();
    const afterArrow = timingLine.slice(arrowPos + 3).trim();
    const endRaw = afterArrow.split(' ')[0];

    const start = parseTimestamp(startRaw);
    const end = parseTimestamp(endRaw);

    const textLines = lines.slice(arrowIndex + 1);
    const text = textLines.join('\n').trim();
    if (!text) continue;

    cues.push({ start, end, text });
  }

  return cues;
}
