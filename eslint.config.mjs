import antfu from '@antfu/eslint-config'

export default antfu({
  react: true,
  formatters: {
    css: true,
    html: true,
    markdown: 'prettier',
  },
  ignores: [
    'spec/**',
    'src-tauri/**',
    'src/generated/**',
  ],
}, {
  files: [
    'src/routes/**/*.tsx',
    'src/shared/components/ui/**/*.tsx',
  ],
  rules: {
    'react-refresh/only-export-components': 'off',
  },
})
