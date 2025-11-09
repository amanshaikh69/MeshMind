/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'neon': '#00bfa6', // legacy alias used in a few places
        'accent': '#00bfa6',
        'accent-secondary': '#5b8aff',
        'panel': '#1e2026',
        'panel-hover': '#23262d',
        'surface': '#181a1e',
        'surface-2': '#111315',
        'dark': '#0b0d10',
        'bright': '#e8eaed',
        'dim': '#9ba0a8',
        'divider': '#2a2d33',
      },
      animation: {
        'float': 'float 3s ease-in-out infinite',
        'fadeIn': 'fadeIn 0.5s ease-in-out',
        'blink': 'blink 1s step-end infinite',
        'slideIn': 'slideIn 0.3s ease-out',
        'pulse-glow': 'pulse-glow 3s ease-in-out infinite',
      },
      keyframes: {
        float: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-10px)' },
        },
        fadeIn: {
          '0%': { opacity: '0', transform: 'translateY(10px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        blink: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0' },
        },
        slideIn: {
          '0%': { transform: 'translateX(-20px)', opacity: '0' },
          '100%': { transform: 'translateX(0)', opacity: '1' },
        },
        'pulse-glow': {
          '0%, 100%': {
            opacity: '1',
            boxShadow: '0 0 16px rgba(0,191,166,0.18)',
          },
          '50%': {
            opacity: '0.9',
            boxShadow: '0 0 8px rgba(0,191,166,0.10)',
          },
        },
      },
      boxShadow: {
        'glow': '0 8px 24px rgba(0,0,0,0.35)',
        'soft': '0 4px 16px rgba(0,0,0,0.25)',
      },
      dropShadow: {
        'glow': '0 0 8px rgba(0,191,166,0.25)',
      },
    },
  },
  plugins: [],
};
