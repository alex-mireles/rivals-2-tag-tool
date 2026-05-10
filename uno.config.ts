import { defineConfig, presetWind3 } from 'unocss';

export default defineConfig({
  presets: [
    presetWind3(),
  ],

  theme: {
    colors: {
      // Deep Purple Base Palette
      base: {
        900: '#0d0716',
        800: '#120a1f',
        700: '#1a1030',
        600: '#1e1035',
        500: '#2a1845',
      },

      // Frosted surface layers (used via bg-surface-*)
      surface: {
        DEFAULT: 'rgba(255, 255, 255, 0.05)',
        hover: 'rgba(255, 255, 255, 0.08)',
        inset: 'rgba(0, 0, 0, 0.28)',
        'inset-deep': 'rgba(0, 0, 0, 0.35)',
      },

      // Indigo accent
      accent: {
        DEFAULT: 'rgba(99, 102, 241, 0.80)',
        hover: 'rgba(99, 102, 241, 0.90)',
        muted: 'rgba(99, 102, 241, 0.12)',
      },

      // Text tones (lavender-tinted whites)
      text: {
        primary: '#f0ecf8',               
        secondary: '#c8b8e8',             
        muted: 'rgba(200, 180, 230, 0.60)', 
        hint: 'rgba(200, 180, 230, 0.25)',
      },

      // Borders
      line: {
        subtle: 'rgba(255, 255, 255, 0.04)',
        DEFAULT: 'rgba(255, 255, 255, 0.08)',
        divider: 'rgba(255, 255, 255, 0.05)',
      },
    },

    borderRadius: {
      card: '20px',
      panel: '12px',
      button: '10px',
      badge: '6px',
    },
  },
});