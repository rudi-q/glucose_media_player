export interface SubtitleStyle {
  id: string;
  name: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: string;
  color: string;
  backgroundType: 'none' | 'pill' | 'box' | 'stripe' | 'frosted';
  backgroundColor: string;
  textShadow: string;
  textStroke: string;
  letterSpacing: string;
  /** Vertical position as a percentage from the bottom of the viewport */
  position: number;
  lineHeight: number;
}

export const SUBTITLE_PRESETS: SubtitleStyle[] = [
  {
    id: 'classic',
    name: 'Classic',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 22,
    fontWeight: '500',
    color: '#ffffff',
    backgroundType: 'pill',
    backgroundColor: 'rgba(0,0,0,0.78)',
    textShadow: 'none',
    textStroke: 'none',
    letterSpacing: 'normal',
    position: 8,
    lineHeight: 1.5,
  },
  {
    id: 'cinema',
    name: 'Cinema',
    fontFamily: "'Georgia', 'Times New Roman', serif",
    fontSize: 26,
    fontWeight: '400',
    color: '#f5e6c8',
    backgroundType: 'none',
    backgroundColor: 'transparent',
    textShadow: '0 2px 12px rgba(0,0,0,1), 0 0 30px rgba(0,0,0,0.8)',
    textStroke: 'none',
    letterSpacing: '0.03em',
    position: 8,
    lineHeight: 1.5,
  },
  {
    id: 'bold',
    name: 'Bold',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 28,
    fontWeight: '800',
    color: '#ffffff',
    backgroundType: 'none',
    backgroundColor: 'transparent',
    textShadow: '0 1px 6px rgba(0,0,0,0.9)',
    textStroke: '1.5px #000000',
    letterSpacing: '-0.02em',
    position: 8,
    lineHeight: 1.35,
  },
  {
    id: 'frosted',
    name: 'Frosted',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 21,
    fontWeight: '500',
    color: '#ffffff',
    backgroundType: 'frosted',
    backgroundColor: 'rgba(255,255,255,0.12)',
    textShadow: 'none',
    textStroke: 'none',
    letterSpacing: 'normal',
    position: 8,
    lineHeight: 1.5,
  },
  {
    id: 'neon',
    name: 'Neon',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 23,
    fontWeight: '700',
    color: '#00ffc8',
    backgroundType: 'none',
    backgroundColor: 'transparent',
    textShadow:
      '0 0 12px rgba(0,255,200,0.9), 0 0 30px rgba(0,255,200,0.5), 0 2px 6px rgba(0,0,0,0.9)',
    textStroke: 'none',
    letterSpacing: '0.04em',
    position: 8,
    lineHeight: 1.5,
  },
  {
    id: 'minimal',
    name: 'Minimal',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 18,
    fontWeight: '400',
    color: 'rgba(255,255,255,0.92)',
    backgroundType: 'none',
    backgroundColor: 'transparent',
    textShadow: '0 1px 4px rgba(0,0,0,0.85)',
    textStroke: 'none',
    letterSpacing: 'normal',
    position: 8,
    lineHeight: 1.5,
  },
  {
    id: 'highlight',
    name: 'Highlight',
    fontFamily: "'Inter Variable', Inter, system-ui, sans-serif",
    fontSize: 21,
    fontWeight: '600',
    color: '#1a1a1a',
    backgroundType: 'box',
    backgroundColor: 'rgba(255,214,0,0.95)',
    textShadow: 'none',
    textStroke: 'none',
    letterSpacing: '0.01em',
    position: 8,
    lineHeight: 1.6,
  },
];

export const DEFAULT_PRESET_ID = 'classic';

if (!SUBTITLE_PRESETS.some((p) => p.id === DEFAULT_PRESET_ID)) {
  throw new Error(`DEFAULT_PRESET_ID '${DEFAULT_PRESET_ID}' not found in SUBTITLE_PRESETS`);
}
