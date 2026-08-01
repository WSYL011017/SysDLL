import {
  defineConfig,
  presetAttributify,
  presetIcons,
  presetTypography,
  presetWebFonts,
  presetWind3,
  transformerDirectives,
  transformerVariantGroup,
} from 'unocss'

// antfu-style semantic shortcuts. Keep tokens minimal — only what we actually reuse.
export default defineConfig({
  presets: [
    presetWind3(),
    presetAttributify(),
    presetTypography(),
    presetIcons({
      scale: 1.2,
      cdn: 'https://esm.sh/',
    }),
    presetWebFonts({
      provider: 'none', // we'll load Inter / JetBrains Mono via <link> in index.html
      fonts: {},
    }),
  ],
  transformers: [transformerDirectives(), transformerVariantGroup()],
  shortcuts: {
    // surfaces
    'bg-base': 'bg-white dark:bg-#111',
    'bg-secondary': 'bg-#f5f5f5 dark:bg-#1a1a1a',
    'bg-elevated': 'bg-white dark:bg-#161616 shadow-sm',
    'border-base': 'border-#8882',
    'color-base': 'text-neutral-800 dark:text-neutral-200',
    'color-mute': 'text-neutral-500 dark:text-neutral-400',

    // interactive
    'btn-action': 'inline-flex items-center gap-2 rounded-md border border-base px3 py1.5 text-sm hover:bg-active disabled:pointer-events-none disabled:op30 transition-colors',
    'btn-primary': 'inline-flex items-center gap-2 rounded-md bg-primary-600 px3 py1.5 text-sm text-white hover:bg-primary-700 disabled:pointer-events-none disabled:op50 transition-colors',
    'btn-danger': 'inline-flex items-center gap-2 rounded-md bg-red-600 px3 py1.5 text-sm text-white hover:bg-red-700 disabled:pointer-events-none disabled:op50 transition-colors',

    'bg-active': 'bg-neutral-100 dark:bg-neutral-800',
    'border-active': 'border-primary-500/40',

    // z layers
    'z-shell': 'z-50',
    'z-overlay': 'z-100',
  },
  theme: {
    colors: {
      primary: {
        50: '#eff6ff',
        100: '#dbeafe',
        500: '#3b82f6',
        600: '#2563eb',
        700: '#1d4ed8',
      },
    },
  },
  safelist: [
    // Severity swatches used dynamically in templates.
    'bg-red-500', 'bg-orange-500', 'bg-yellow-500', 'bg-blue-500',
    'text-red-500', 'text-orange-500', 'text-yellow-500', 'text-blue-500',
  ],
})
