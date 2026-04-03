import { LLMClient, LLMConfig, Message, getSystemPrompt } from '../llm/index.js';
import { ToolExecutor } from '../tools/executor.js';
import { BrowserManager } from '../core/index.js';
import { BrowserToolName } from '../tools/definitions.js';
import { ToolExecutionConfig, ToolExecutionResult } from '../tools/types.js';

interface AgentOptions {
  /** LLM configuration */
  llmConfig: LLMConfig;
  /** Optional custom instructions appended to system prompt */
  customInstructions?: string;
  /** Maximum number of tool call rounds (default: 50) */
  maxRounds?: number;
  /** Tool execution configuration */
  toolConfig?: {
    /** Enable parallel execution where safe (default: false) */
    parallel?: boolean;
    /** Continue on tool failure (default: true) */
    continueOnError?: boolean;
    /** Default retry configuration for all tools */
    defaultRetryConfig?: ToolExecutionConfig;
  };
}

/**
 * Browser Agent that manages LLM conversation with tool calling
 * 
 * Each Agent instance maintains its own conversation history and
 * can be associated with a specific browser instance if needed.
 */
export class Agent {
  private llm: LLMClient;
  private toolExecutor: ToolExecutor;
  private messages: Message[] = [];
  private maxRounds: number;
  private browserManager: BrowserManager;
  private isRunning = false;
  private toolConfig: AgentOptions['toolConfig'];

  constructor(browserManager: BrowserManager, options: AgentOptions) {
    this.browserManager = browserManager;
    this.llm = new LLMClient(options.llmConfig);
    this.toolExecutor = new ToolExecutor(browserManager);
    this.maxRounds = options.maxRounds ?? 50;
    this.toolConfig = {
      parallel: false,
      continueOnError: true,
      ...options.toolConfig,
    };

    // Initialize with system prompt
    this.messages.push({
      role: 'system',
      content: getSystemPrompt(options.customInstructions),
    });
  }

  /**
   * Send a user message and process the response, including any tool calls
   * 
   * This is the main entry point for interacting with the agent.
   * It will:
   * 1. Add the user message to conversation
   * 2. Call the LLM with tools
   * 3. Execute any tool calls requested (with parallel execution if enabled)
   * 4. Return the final response
   */
  async chat(userMessage: string): Promise<string> {
    if (this.isRunning) {
      throw new Error('Agent is already processing a message');
    }

    this.isRunning = true;

    try {
      // Add user message
      this.messages.push({
        role: 'user',
        content: userMessage,
      });

      let rounds = 0;

      // Keep looping while the LLM wants to make tool calls
      while (rounds < this.maxRounds) {
        rounds++;

        // Call LLM
        const response = await this.llm.chat(this.messages);

        // If no tool calls, we're done
        if (!response.toolCalls || response.toolCalls.length === 0) {
          // Add assistant message
          this.messages.push({
            role: 'assistant',
            content: response.content ?? '',
          });

          return response.content ?? '';
        }

        // Add assistant message with tool calls
        this.messages.push({
          role: 'assistant',
          content: response.content ?? '',
          tool_calls: response.toolCalls.map((call) => ({
            id: call.id,
            type: 'function',
            function: {
              name: call.name,
              arguments: JSON.stringify(call.arguments),
            },
          })),
        });

        // Execute tool calls
        const toolResults = await this.executeToolCalls(response.toolCalls);

        // Check if we should continue based on results and configuration
        const hasFailures = toolResults.some(r => !r.success);
        if (hasFailures && !this.toolConfig?.continueOnError) {
          // Abort the conversation due to tool failure
          const errorMessage = 'Tool execution failed. Aborting conversation.';
          this.messages.push({
            role: 'assistant',
            content: errorMessage,
          });
          return errorMessage;
        }

        // Add all tool results to conversation
        for (const result of toolResults) {
          // Find the original tool call ID
          const toolCall = response.toolCalls.find(t => 
            t.name === result.name && 
            JSON.stringify(t.arguments) === JSON.stringify(result.args)
          );
          
          this.messages.push({
            role: 'tool',
            tool_call_id: toolCall?.id || 'unknown',
            content: result.success 
              ? (result.content || '')
              : `Error: ${result.error || 'Unknown error'}\n\nPartial result: ${result.content || 'none'}`,
          });
        }
      }

      if (rounds >= this.maxRounds) {
        return 'Maximum number of tool call rounds reached. The agent may be stuck in a loop.';
      }

      return '';
    } finally {
      this.isRunning = false;
    }
  }

  /**
   * Execute tool calls with parallel execution support
   */
  private async executeToolCalls(
    toolCalls: Array<{
      id: string;
      name: string;
      arguments: Record<string, unknown>;
    }>
  ): Promise<ToolExecutionResult[]> {
    if (!this.toolConfig?.parallel) {
      // Sequential execution
      const results: ToolExecutionResult[] = [];
      
      for (const call of toolCalls) {
        console.log(`[Tool] ${call.name}: ${JSON.stringify(call.arguments)}`);

        const result = await this.toolExecutor.executeTool(
          call.name as BrowserToolName,
          call.arguments,
          this.toolConfig?.defaultRetryConfig
        );

        results.push({
          name: call.name,
          args: call.arguments,
          success: result.success,
          content: result.content,
          error: result.error,
          durationMs: 0,
          attempts: 1,
        });

        // Print result for visibility
        if (result.success) {
          console.log(`[Tool Result] Success`);
        } else {
          console.log(`[Tool Error] ${result.error}`);
        }
      }
      
      return results;
    } else {
      // Parallel execution with grouping
      // Convert to format expected by executeTools
      const tools = toolCalls.map(call => ({
        name: call.name as BrowserToolName,
        args: call.arguments,
        config: this.toolConfig?.defaultRetryConfig,
      }));

      // Execute with parallel grouping
      const parallelResult = await this.toolExecutor.executeTools(tools, {
        parallel: true,
        continueOnError: this.toolConfig?.continueOnError,
      });

      // Log results
      for (const result of parallelResult.results) {
        console.log(`[Tool] ${result.name}: ${result.success ? 'Success' : 'Failed'} (${result.attempts} attempts, ${result.durationMs}ms)`);
        if (result.error) {
          console.log(`[Tool Error] ${result.error}`);
        }
      }
      
      return parallelResult.results;
    }
  }

  /**
   * Stream a response for interactive CLI
   * 
   * Note: Tool calls still happen after the stream completes
   */
  async *streamChat(userMessage: string): AsyncGenerator<string, string, unknown> {
    // For streaming, we currently don't support mid-stream tool calls
    // The LLM will respond with text, then we check for tool calls
    // This is a simplified version - full implementation would parse tool calls from stream

    this.messages.push({
      role: 'user',
      content: userMessage,
    });

    // For simplicity in streaming mode, we don't use tools
    // Full implementation would parse tool calls from stream
    const stream = this.llm.streamChat(this.messages);
    let fullResponse = '';

    for await (const chunk of stream) {
      fullResponse += chunk;
      yield chunk;
    }

    this.messages.push({
      role: 'assistant',
      content: fullResponse,
    });

    return fullResponse;
  }

  /**
   * Get conversation history (for debugging)
   */
  getHistory(): Message[] {
    return [...this.messages];
  }

  /**
   * Clear conversation history (except system prompt)
   */
  clearHistory(): void {
    const systemMessage = this.messages[0];
    this.messages = [systemMessage];
  }

  /**
   * Clean up all browser instances managed by this agent
   */
  async cleanup(): Promise<void> {
    await this.browserManager.closeAll();
  }

  /**
   * Update tool configuration at runtime
   */
  setToolConfig(config: Partial<AgentOptions['toolConfig']>): void {
    this.toolConfig = { ...this.toolConfig, ...config };
  }

  /**
   * Get current tool configuration
   */
  getToolConfig(): AgentOptions['toolConfig'] {
    return { ...this.toolConfig };
  }
}
