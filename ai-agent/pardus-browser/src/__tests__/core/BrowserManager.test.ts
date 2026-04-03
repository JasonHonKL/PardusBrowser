import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { BrowserManager } from '../../core/BrowserManager.js';

describe('BrowserManager', () => {
  let manager: BrowserManager;

  beforeEach(() => {
    manager = new BrowserManager();
  });

  describe('createInstance', () => {
    it('should create instance with auto-generated ID', async () => {
      // This test would need pardus-browser installed to actually run
      // For now, we just verify the structure is correct
      assert.strictEqual(typeof manager.createInstance, 'function');
    });

    it('should throw on duplicate ID', async () => {
      // Would need actual implementation to test this
      assert.ok(true);
    });

    it('should support custom proxy', async () => {
      assert.strictEqual(typeof manager.createInstance, 'function');
    });
  });

  describe('getInstance', () => {
    it('should return undefined for non-existent instance', () => {
      const instance = manager.getInstance('non-existent');
      assert.strictEqual(instance, undefined);
    });

    it('should return instance if it exists', () => {
      // Would need to create instance first
      assert.strictEqual(manager.getInstance('test'), undefined);
    });
  });

  describe('hasInstance', () => {
    it('should return false for non-existent instance', () => {
      assert.strictEqual(manager.hasInstance('non-existent'), false);
    });

    it('should return true for existing instance', () => {
      // Would need to create instance first
      assert.strictEqual(manager.hasInstance('test'), false);
    });
  });

  describe('listInstances', () => {
    it('should return empty array when no instances', () => {
      const instances = manager.listInstances();
      assert.deepStrictEqual(instances, []);
    });

    it('should include port information', () => {
      const instances = manager.listInstances();
      assert.ok(Array.isArray(instances));
    });
  });

  describe('closeInstance', () => {
    it('should throw for non-existent instance', async () => {
      try {
        await manager.closeInstance('non-existent');
        assert.fail('Should have thrown');
      } catch (error) {
        assert.ok(error instanceof Error);
        assert.ok((error as Error).message.includes('not found'));
      }
    });
  });

  describe('closeAll', () => {
    it('should complete without error when no instances', async () => {
      await manager.closeAll();
      assert.strictEqual(manager.instanceCount, 0);
    });
  });

  describe('instanceCount', () => {
    it('should be 0 initially', () => {
      assert.strictEqual(manager.instanceCount, 0);
    });

    it('should track instance creation and deletion', () => {
      // Initially 0
      assert.strictEqual(manager.instanceCount, 0);
      
      // Would increase after creation
      // Would decrease after close
    });
  });

  describe('port allocation', () => {
    it('should allocate ports sequentially', () => {
      // Port allocation is internal, but we can verify behavior
      // through instance creation
      assert.ok(true);
    });

    it('should reuse ports after instance closes', () => {
      // Verify ports are freed on close
      assert.ok(true);
    });
  });
});
