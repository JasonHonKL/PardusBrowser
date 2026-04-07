/**
 * Integration tests
 * 
 * These tests verify the interaction between components
 * without requiring actual browser/LLM processes.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { BrowserManager } from '../core/BrowserManager.js';
import { ToolExecutor } from '../tools/executor.js';
import { Agent } from '../agent/Agent.js';
import { createMockBrowserManager } from './test-utils.js';

describe('Integration', () => {
  describe('ToolExecutor + BrowserManager', () => {
    let manager: BrowserManager;
    let executor: ToolExecutor;

    beforeEach(() => {
      manager = createMockBrowserManager();
      executor = new ToolExecutor(manager);
    });

    it('should create and navigate in sequence', async () => {
      // Create browser
      const newResult = await executor.executeTool('browser_new', {
        instance_id: 'integration-test',
      });
      assert.strictEqual(newResult.success, true);

      // Navigate
      const navResult = await executor.executeTool('browser_navigate', {
        instance_id: 'integration-test',
        url: 'https://example.com',
      });
      assert.strictEqual(navResult.success, true);
      assert.ok(navResult.content.includes('Navigation Result'));

      // Close
      const closeResult = await executor.executeTool('browser_close', {
        instance_id: 'integration-test',
      });
      assert.strictEqual(closeResult.success, true);
    });

    it('should handle form interaction workflow', async () => {
      // Setup
      await executor.executeTool('browser_new', { instance_id: 'form-test' });
      await executor.executeTool('browser_navigate', {
        instance_id: 'form-test',
        url: 'https://example.com/form',
      });

      // Fill form
      const fillResult = await executor.executeTool('browser_fill', {
        instance_id: 'form-test',
        element_id: '#1',
        value: 'test@example.com',
      });
      assert.strictEqual(fillResult.success, true);

      // Submit
      const submitResult = await executor.executeTool('browser_submit', {
        instance_id: 'form-test',
      });
      assert.strictEqual(submitResult.success, true);
      assert.ok(submitResult.content.toLowerCase().includes('navigated'));
    });

    it('should manage multiple instances', async () => {
      // Create multiple browsers
      await executor.executeTool('browser_new', { instance_id: 'browser-a' });
      await executor.executeTool('browser_new', { instance_id: 'browser-b' });

      // Navigate to different URLs
      await executor.executeTool('browser_navigate', {
        instance_id: 'browser-a',
        url: 'https://site-a.com',
      });

      await executor.executeTool('browser_navigate', {
        instance_id: 'browser-b',
        url: 'https://site-b.com',
      });

      // List should show both
      const listResult = await executor.executeTool('browser_list', {});
      assert.strictEqual(listResult.success, true);
      assert.ok(listResult.content.includes('browser-a'));
      assert.ok(listResult.content.includes('browser-b'));

      // Close one
      await executor.executeTool('browser_close', { instance_id: 'browser-a' });

      // List should show one
      const listAfter = await executor.executeTool('browser_list', {});
      assert.ok(listAfter.content.includes('browser-b'));
    });
  });

  describe('Agent + BrowserManager', () => {
    let manager: BrowserManager;
    let agent: Agent;

    beforeEach(() => {
      manager = createMockBrowserManager();
      agent = new Agent(manager, {
        llmConfig: {
          apiKey: 'test-key',
          baseURL: 'http://localhost',
          model: 'test',
        },
      });
    });

    afterEach(async () => {
      await agent.cleanup();
    });

    it('should initialize with empty history', () => {
      const history = agent.getHistory();
      assert.strictEqual(history.length, 1); // Just system prompt
      assert.strictEqual(history[0].role, 'system');
    });

    it('should clear history while preserving system prompt', () => {
      agent.clearHistory();
      const history = agent.getHistory();
      assert.strictEqual(history.length, 1);
      assert.strictEqual(history[0].role, 'system');
    });

    it('should cleanup all browsers on exit', async () => {
      // Create browsers through manager
      await manager.createInstance({ id: 'browser-1' });
      await manager.createInstance({ id: 'browser-2' });
      await manager.createInstance({ id: 'browser-3' });

      assert.strictEqual(manager.instanceCount, 3);

      await agent.cleanup();

      assert.strictEqual(manager.instanceCount, 0);
    });
  });

  describe('Error handling', () => {
    let manager: BrowserManager;
    let executor: ToolExecutor;

    beforeEach(() => {
      manager = createMockBrowserManager();
      executor = new ToolExecutor(manager);
    });

    it('should handle operations on non-existent instances gracefully', async () => {
      const operations = [
        executor.executeTool('browser_navigate', {
          instance_id: 'non-existent',
          url: 'https://example.com',
        }),
        executor.executeTool('browser_click', {
          instance_id: 'non-existent',
          element_id: '#1',
        }),
        executor.executeTool('browser_fill', {
          instance_id: 'non-existent',
          element_id: '#1',
          value: 'test',
        }),
        executor.executeTool('browser_get_state', {
          instance_id: 'non-existent',
        }),
        executor.executeTool('browser_close', {
          instance_id: 'non-existent',
        }),
      ];

      for (const operation of operations) {
        const result = await operation;
        assert.strictEqual(result.success, false);
        assert.ok(result.error);
      }
    });

    it('should handle missing required parameters', async () => {
      // Create instance first
      await executor.executeTool('browser_new', { instance_id: 'param-test' });

      const missingUrl = await executor.executeTool('browser_navigate', {
        instance_id: 'param-test',
      });
      assert.strictEqual(missingUrl.success, false);

      const missingElement = await executor.executeTool('browser_click', {
        instance_id: 'param-test',
      });
      assert.strictEqual(missingElement.success, false);

      const missingValue = await executor.executeTool('browser_fill', {
        instance_id: 'param-test',
        element_id: '#1',
      });
      assert.strictEqual(missingValue.success, false);
    });
  });

  describe('State consistency', () => {
    let manager: BrowserManager;
    let executor: ToolExecutor;

    beforeEach(() => {
      manager = createMockBrowserManager();
      executor = new ToolExecutor(manager);
    });

    it('should maintain instance isolation', async () => {
      // Create two instances
      await executor.executeTool('browser_new', { instance_id: 'isolated-a' });
      await executor.executeTool('browser_new', { instance_id: 'isolated-b' });

      // Navigate each to different URLs
      const navA = await executor.executeTool('browser_navigate', {
        instance_id: 'isolated-a',
        url: 'https://site-a.com',
      });
      const navB = await executor.executeTool('browser_navigate', {
        instance_id: 'isolated-b',
        url: 'https://site-b.com',
      });

      // Each should have its own URL
      assert.ok(navA.content.includes('site-a'));
      assert.ok(navB.content.includes('site-b'));

      // Get state for each
      const stateA = await executor.executeTool('browser_get_state', {
        instance_id: 'isolated-a',
      });
      const stateB = await executor.executeTool('browser_get_state', {
        instance_id: 'isolated-b',
      });

      // States should be independent
      assert.ok(stateA.success);
      assert.ok(stateB.success);
    });
  });
});
