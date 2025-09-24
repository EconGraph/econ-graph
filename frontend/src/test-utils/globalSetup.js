/**
 * Jest Global Setup
 * Memory optimization and test environment preparation
 */

module.exports = async () => {
  // Set memory limits for Node.js processes
  process.env.NODE_OPTIONS = '--max-old-space-size=2048 --max-semi-space-size=128';

  // Configure Jest for memory efficiency
  process.env.JEST_WORKER_ID = '1';

  console.log('🧠 Jest global setup: Memory limits configured');
  console.log(`📊 Node.js memory limit: ${process.env.NODE_OPTIONS}`);
};
