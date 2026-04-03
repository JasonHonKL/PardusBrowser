import { describe, it } from 'node:test';
import assert from 'node:assert';
import type {
  SemanticTree,
  BrowserNavigateResult,
  BrowserClickResult,
  BrowserFillResult,
  BrowserSubmitResult,
  BrowserScrollResult,
  ToolResult,
} from '../../core/types.js';

describe('Core Types', () => {
  describe('SemanticTree', () => {
    it('should have required properties', () => {
      const tree: SemanticTree = {
        url: 'https://example.com',
        title: 'Example',
        markdown: '# Example\n\n[#1 Link] Test',
        stats: {
          landmarks: 1,
          links: 2,
          headings: 1,
          actions: 3,
          forms: 0,
          totalNodes: 10,
        },
      };

      assert.strictEqual(tree.url, 'https://example.com');
      assert.strictEqual(tree.title, 'Example');
      assert.ok(tree.markdown.includes('# Example'));
      assert.strictEqual(tree.stats.links, 2);
      assert.strictEqual(tree.stats.totalNodes, 10);
    });

    it('should allow optional title', () => {
      const tree: SemanticTree = {
        url: 'https://example.com',
        markdown: 'Content',
        stats: {
          landmarks: 0,
          links: 0,
          headings: 0,
          actions: 0,
          forms: 0,
          totalNodes: 1,
        },
      };

      assert.strictEqual(tree.title, undefined);
    });
  });

  describe('BrowserNavigateResult', () => {
    it('should represent successful navigation', () => {
      const result: BrowserNavigateResult = {
        success: true,
        url: 'https://example.com',
        title: 'Example',
        markdown: '# Example',
        stats: { landmarks: 1, links: 2, headings: 1, actions: 3, forms: 0, totalNodes: 10 },
      };

      assert.strictEqual(result.success, true);
      assert.strictEqual(result.url, 'https://example.com');
    });

    it('should represent failed navigation', () => {
      const result: BrowserNavigateResult = {
        success: false,
        url: 'https://invalid.com',
        markdown: '',
        stats: { landmarks: 0, links: 0, headings: 0, actions: 0, forms: 0, totalNodes: 0 },
        error: 'Network error',
      };

      assert.strictEqual(result.success, false);
      assert.strictEqual(result.error, 'Network error');
    });
  });

  describe('ToolResult', () => {
    it('should represent successful tool execution', () => {
      const result: ToolResult = {
        success: true,
        content: 'Browser instance created',
      };

      assert.strictEqual(result.success, true);
      assert.ok(result.content.includes('created'));
    });

    it('should represent failed tool execution', () => {
      const result: ToolResult = {
        success: false,
        content: '',
        error: 'Instance not found',
      };

      assert.strictEqual(result.success, false);
      assert.strictEqual(result.error, 'Instance not found');
    });
  });

  describe('BrowserClickResult', () => {
    it('should track navigation status', () => {
      const result: BrowserClickResult = {
        success: true,
        navigated: true,
        url: 'https://example.com/new',
        markdown: '# New Page',
      };

      assert.strictEqual(result.navigated, true);
      assert.strictEqual(result.url, 'https://example.com/new');
    });

    it('should handle click without navigation', () => {
      const result: BrowserClickResult = {
        success: true,
        navigated: false,
      };

      assert.strictEqual(result.navigated, false);
      assert.strictEqual(result.url, undefined);
    });
  });

  describe('BrowserFillResult', () => {
    it('should be simple success/failure', () => {
      const success: BrowserFillResult = { success: true };
      const failure: BrowserFillResult = { success: false, error: 'Element not found' };

      assert.strictEqual(success.success, true);
      assert.strictEqual(failure.success, false);
      assert.strictEqual(failure.error, 'Element not found');
    });
  });

  describe('BrowserScrollResult', () => {
    it('should handle scroll directions', () => {
      const up: BrowserScrollResult = { success: true };
      const down: BrowserScrollResult = { success: true };
      const failure: BrowserScrollResult = { success: false, error: 'Page not scrollable' };

      assert.strictEqual(up.success, true);
      assert.strictEqual(down.success, true);
      assert.strictEqual(failure.success, false);
    });
  });
});
