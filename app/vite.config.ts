import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  test: {
    // The kcp codec and INFO parsers are pure functions and the transport is
    // exercised with a fake HID device, so the suite needs no DOM.
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
