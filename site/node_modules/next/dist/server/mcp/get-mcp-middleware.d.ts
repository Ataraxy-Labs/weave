import type { ServerResponse, IncomingMessage } from 'http';
import { type McpServerOptions } from './get-or-create-mcp-server';
export declare function getMcpMiddleware(options: McpServerOptions): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
