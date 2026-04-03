import OpenAI from 'openai';
import { browserTools } from '../tools/definitions.js';

export interface LLMConfig {
  /** OpenAI API key */
  apiKey: string;
  /** Base URL for API (default: https://api.openai.com/v1) */
  baseURL?: string;
  /** Model to use (default: gpt-4) */
  model?: string;
  /** Temperature (default: 0.7) */
  temperature?: number;
  /** Max tokens (default: 4000) */
  maxTokens?: number;
}

export interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  tool_calls?: Array<{
    id: string;
    type: 'function';
    function: {
      name: string;
      arguments: string;
    };
  }>;
  tool_call_id?: string;
  name?: string;
}

export class LLMClient {
  private client: OpenAI;
  private config: Required<LLMConfig>;

  constructor(config: LLMConfig) {
    this.config = {
      apiKey: config.apiKey,
      baseURL: config.baseURL ?? 'https://api.openai.com/v1',
      model: config.model ?? 'gpt-4',
      temperature: config.temperature ?? 0.7,
      maxTokens: config.maxTokens ?? 4000,
    };

    this.client = new OpenAI({
      apiKey: this.config.apiKey,
      baseURL: this.config.baseURL,
    });
  }

  /**
   * Send a conversation to the LLM with browser tools enabled
   */
  async chat(messages: Message[]): Promise<{
    content: string | null;
    toolCalls?: Array<{
      id: string;
      name: string;
      arguments: Record<string, unknown>;
    }>;
  }> {
    const response = await this.client.chat.completions.create({
      model: this.config.model,
      messages: messages as OpenAI.Chat.ChatCompletionMessageParam[],
      tools: browserTools as unknown as OpenAI.Chat.ChatCompletionTool[],
      tool_choice: 'auto',
      temperature: this.config.temperature,
      max_tokens: this.config.maxTokens,
    });

    const choice = response.choices[0];
    const message = choice.message;

    // Parse tool calls if present
    const toolCalls = message.tool_calls?.map((call) => ({
      id: call.id,
      name: call.function.name,
      arguments: JSON.parse(call.function.arguments) as Record<string, unknown>,
    }));

    return {
      content: message.content,
      toolCalls,
    };
  }

  /**
   * Stream a conversation (for interactive CLI)
   */
  async *streamChat(messages: Message[]): AsyncGenerator<string, void, unknown> {
    const stream = await this.client.chat.completions.create({
      model: this.config.model,
      messages: messages as OpenAI.Chat.ChatCompletionMessageParam[],
      tools: browserTools as unknown as OpenAI.Chat.ChatCompletionTool[],
      tool_choice: 'auto',
      temperature: this.config.temperature,
      max_tokens: this.config.maxTokens,
      stream: true,
    });

    for await (const chunk of stream) {
      const delta = chunk.choices[0]?.delta?.content;
      if (delta) {
        yield delta;
      }
    }
  }

  getModel(): string {
    return this.config.model;
  }
}
