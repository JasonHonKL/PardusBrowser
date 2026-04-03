import { BrowserInstance } from './BrowserInstance.js';
import { BrowserInstanceInfo } from './types.js';

export class BrowserManager {
  private instances = new Map<string, BrowserInstance>();
  private basePort = 9222;
  private maxPort = 9322; // Support up to 100 instances
  private usedPorts = new Set<number>();

  /**
   * Create a new browser instance
   */
  async createInstance(options?: { 
    id?: string; 
    proxy?: string;
    timeout?: number; // Timeout in ms for operations
  }): Promise<BrowserInstance> {
    const id = options?.id ?? this.generateInstanceId();
    
    if (this.instances.has(id)) {
      throw new Error(`Browser instance with id "${id}" already exists`);
    }

    const port = this.allocatePort();
    if (!port) {
      throw new Error('No available ports for new browser instance');
    }

    const instance = new BrowserInstance(id, port);
    
    // Set custom timeout if provided
    if (options?.timeout) {
      (instance as unknown as { requestTimeout: number }).requestTimeout = options.timeout;
    }

    try {
      await instance.spawn(options?.proxy);
      this.instances.set(id, instance);
      
      // Clean up port allocation on process exit
      instance.on('exit', () => {
        this.usedPorts.delete(port);
        this.instances.delete(id);
      });

      return instance;
    } catch (error) {
      this.usedPorts.delete(port);
      throw error;
    }
  }

  /**
   * Get an existing instance by ID
   */
  getInstance(id: string): BrowserInstance | undefined {
    return this.instances.get(id);
  }

  /**
   * Check if an instance exists
   */
  hasInstance(id: string): boolean {
    return this.instances.has(id);
  }

  /**
   * Kill and remove a browser instance
   */
  async closeInstance(id: string): Promise<void> {
    const instance = this.instances.get(id);
    if (!instance) {
      throw new Error(`Browser instance "${id}" not found`);
    }

    instance.kill();
    this.instances.delete(id);
    this.usedPorts.delete(instance.port);
  }

  /**
   * Kill all browser instances
   */
  async closeAll(): Promise<void> {
    const promises: Promise<void>[] = [];
    
    for (const [id, instance] of this.instances) {
      promises.push(
        new Promise((resolve) => {
          instance.kill();
          resolve();
        })
      );
    }

    await Promise.all(promises);
    this.instances.clear();
    this.usedPorts.clear();
  }

  /**
   * List all active instances
   */
  listInstances(): BrowserInstanceInfo[] {
    const infos: BrowserInstanceInfo[] = [];
    
    for (const [id, instance] of this.instances) {
      infos.push({
        id,
        url: instance.currentUrl,
        connected: instance.isConnected(),
        port: instance.port,
      });
    }

    return infos;
  }

  /**
   * Get the count of active instances
   */
  get instanceCount(): number {
    return this.instances.size;
  }

  /**
   * Generate a unique instance ID
   */
  private generateInstanceId(): string {
    const timestamp = Date.now().toString(36);
    const random = Math.random().toString(36).substring(2, 6);
    return `browser_${timestamp}_${random}`;
  }

  /**
   * Allocate an available port
   */
  private allocatePort(): number | null {
    for (let port = this.basePort; port <= this.maxPort; port++) {
      if (!this.usedPorts.has(port)) {
        this.usedPorts.add(port);
        return port;
      }
    }
    return null;
  }
}

// Export singleton instance for global management
export const browserManager = new BrowserManager();
