import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// Supply document metadata normally supplied by index.html.
// axe runs with its default rules; no violations are filtered or disabled.
document.documentElement.lang = 'en';
document.title = 'EconGraph';
afterEach(cleanup);
