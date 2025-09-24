/**
 * Jest Global Teardown
 * Cleanup and memory management after test execution
 */

module.exports = async () => {
  // Force garbage collection if available
  if (global.gc) {
    global.gc();
  }

  // Clear any remaining timers
  if (global.clearTimeout) {
    global.clearTimeout();
  }

  console.log('🧹 Jest global teardown: Memory cleanup completed');
};
