import type { ServerResponse, IncomingMessage } from 'http';
export declare function getAttachNodejsDebuggerMiddleware(): (req: IncomingMessage, res: ServerResponse, next: () => void) => Promise<void>;
