export declare function interceptTestApis(): () => void;
export declare function wrapRequestHandler<T, TRequest extends Request>(handler: (req: TRequest, fn: () => T) => T): (req: TRequest, fn: () => T) => T;
