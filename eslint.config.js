// ESLint 9 flat config (supersedes .eslintrc.cjs)
// 范围：src/ 下的 React + TypeScript 文件
import js from '@eslint/js';
import tsParser from '@typescript-eslint/parser';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';

export default [
  // 忽略构建产物、依赖、其它语言目录
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'src-tauri/**',
      'scripts/**',
      'public/**',
      '**/*.config.js',
      '**/*.config.cjs',
      '**/*.config.ts',
      '.github/**',
      'screenshots/**',
      'docs/**',
    ],
  },
  // 基础推荐规则
  js.configs.recommended,
  // React + TypeScript 配置
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
      globals: {
        // 浏览器 / Tauri 运行时常用全局
        window: 'readonly',
        document: 'readonly',
        console: 'readonly',
        fetch: 'readonly',
        URL: 'readonly',
        URLSearchParams: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        setInterval: 'readonly',
        clearInterval: 'readonly',
        process: 'readonly',
        HTMLElement: 'readonly',
        NodeJS: 'readonly',
      },
    },
    plugins: {
      react: reactPlugin,
      'react-hooks': reactHooksPlugin,
    },
    settings: {
      react: { version: 'detect' },
    },
    rules: {
      ...reactPlugin.configs.recommended.rules,
      ...reactHooksPlugin.configs.recommended.rules,
      // Vite + React 17+ JSX transform 不再需要 React 在作用域内
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',
      // Hooks 规则
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      // 未使用变量：下划线前缀允许
      'no-unused-vars': 'off',
    },
  },
];
