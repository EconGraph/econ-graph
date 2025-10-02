/**
 * CI-specific test helpers to handle timing and resource constraints
 * in GitHub Actions environment
 */

import { waitFor } from '@testing-library/react';

// CI-specific wait options with longer timeouts
export const CI_WAIT_OPTIONS = {
  timeout: process.env.CI ? 15000 : 5000, // 15s in CI, 5s locally
  interval: process.env.CI ? 100 : 50, // Check every 100ms in CI, 50ms locally
};

// Robust waitFor wrapper that handles CI timing issues
export const waitForCI = async (
  callback: () => void | Promise<void>,
  options: any = {}
) => {
  const mergedOptions = { ...CI_WAIT_OPTIONS, ...options };
  
  try {
    await waitFor(callback, mergedOptions);
  } catch (error) {
    // In CI, provide more detailed error information
    if (process.env.CI) {
      console.error('[CI Test Helper] WaitFor failed:', error);
      console.error('[CI Test Helper] Current DOM state:', document.body.innerHTML);
    }
    throw error;
  }
};

// Memory pressure detection for CI
export const checkMemoryPressure = () => {
  if (process.env.CI && typeof process !== 'undefined') {
    const memUsage = process.memoryUsage();
    const heapUsedMB = memUsage.heapUsed / 1024 / 1024;
    
    if (heapUsedMB > 1000) { // 1GB threshold
      console.warn(`[CI Test Helper] High memory usage: ${heapUsedMB.toFixed(2)}MB`);
      
      // Force garbage collection if available
      if (typeof global !== 'undefined' && global.gc) {
        global.gc();
      }
    }
  }
};

// CI-specific test timeout wrapper
export const withCITimeout = <T extends any[], R>(
  fn: (...args: T) => R | Promise<R>,
  timeoutMs: number = process.env.CI ? 15000 : 5000
) => {
  return async (...args: T): Promise<R> => {
    return Promise.race([
      fn(...args),
      new Promise<never>((_, reject) => 
        setTimeout(() => reject(new Error(`Test timed out after ${timeoutMs}ms`)), timeoutMs)
      )
    ]);
  };
};

// Resource cleanup for CI
export const cleanupCIResources = () => {
  // Clear all timers more aggressively in CI
  for (let i = 1; i < 100000; i++) {
    clearTimeout(i);
    clearInterval(i);
  }
  
  // Clear any remaining promises
  if (typeof global !== 'undefined' && global.gc) {
    global.gc();
  }
  
  // Log memory usage in CI
  if (process.env.CI && typeof process !== 'undefined') {
    const memUsage = process.memoryUsage();
    console.log(`[CI Test Helper] Memory usage: ${(memUsage.heapUsed / 1024 / 1024).toFixed(2)}MB`);
  }
};
