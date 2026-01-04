export default {
  content: ['./index.html', './src/**/*.{vue,ts}'],
  theme: {
    extend: {
      fontFamily: {
        display: ['Manrope', 'sans-serif'],
      },
      colors: {
        ink: {
          900: '#0c1322',
          700: '#1a2537',
          500: '#334155',
        },
        accent: {
          500: '#0f766e',
          400: '#14b8a6',
          200: '#5eead4',
        },
        sand: {
          50: '#f8f7f3',
          100: '#f1eee6',
          200: '#e5dfd1',
        },
        sun: {
          500: '#f59e0b',
          400: '#fbbf24',
        },
      },
      boxShadow: {
        panel: '0 24px 60px -40px rgba(15, 23, 42, 0.35)',
      },
      keyframes: {
        floatIn: {
          '0%': { opacity: '0', transform: 'translateY(12px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        pulseSoft: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
      },
      animation: {
        floatIn: 'floatIn 0.5s ease-out',
        pulseSoft: 'pulseSoft 1.6s ease-in-out infinite',
      },
    },
  },
  plugins: [],
};