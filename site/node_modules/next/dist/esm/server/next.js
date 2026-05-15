import './require-hook';
import './node-polyfill-crypto';
import * as log from '../build/output/log';
import loadConfig from './config';
import path from 'node:path';
import { NON_STANDARD_NODE_ENV } from '../lib/constants';
import { PHASE_DEVELOPMENT_SERVER, SERVER_FILES_MANIFEST } from '../shared/lib/constants';
import { PHASE_PRODUCTION_SERVER } from '../shared/lib/constants';
import { getTracer } from './lib/trace/tracer';
import { NextServerSpan } from './lib/trace/constants';
import { formatUrl } from '../shared/lib/router/utils/format-url';
import { AsyncCallbackSet } from './lib/async-callback-set';
import { RouterServerContextSymbol, routerServerGlobal } from './lib/router-utils/router-server-context';
let ServerImpl;
const getServerImpl = async ()=>{
    if (ServerImpl === undefined) {
        ServerImpl = (await Promise.resolve(require('./next-server'))).default;
    }
    return ServerImpl;
};
const SYMBOL_LOAD_CONFIG = Symbol('next.load_config');
/** The wrapper server used by `next start` */ export class NextServer {
    constructor(options){
        this.options = options;
    }
    get hostname() {
        return this.options.hostname;
    }
    get port() {
        return this.options.port;
    }
    getRequestHandler() {
        return async (req, res, parsedUrl)=>{
            return getTracer().trace(NextServerSpan.getRequestHandler, async ()=>{
                const requestHandler = await this.getServerRequestHandler();
                return requestHandler(req, res, parsedUrl);
            });
        };
    }
    /**
   * @internal - this method is internal to Next.js and should not be used
   * directly by end-users, only used in testing
   */ getRequestHandlerWithMetadata(meta) {
        return async (req, res, parsedUrl)=>{
            return getTracer().trace(NextServerSpan.getRequestHandlerWithMetadata, async ()=>{
                const server = await this.getServer();
                const handler = server.getRequestHandlerWithMetadata(meta);
                return handler(req, res, parsedUrl);
            });
        };
    }
    getUpgradeHandler() {
        return async (req, socket, head)=>{
            const server = await this.getServer();
            // @ts-expect-error we mark this as protected so it
            // causes an error here
            return server.handleUpgrade.apply(server, [
                req,
                socket,
                head
            ]);
        };
    }
    setAssetPrefix(assetPrefix) {
        if (this.server) {
            this.server.setAssetPrefix(assetPrefix);
        } else {
            this.preparedAssetPrefix = assetPrefix;
        }
    }
    logError(...args) {
        if (this.server) {
            this.server.logError(...args);
        }
    }
    async logErrorWithOriginalStack(err, type) {
        const server = await this.getServer();
        // this is only available on dev server
        if (server.logErrorWithOriginalStack) {
            return server.logErrorWithOriginalStack(err, type);
        }
    }
    async revalidate(...args) {
        const server = await this.getServer();
        return server.revalidate(...args);
    }
    async render(...args) {
        const server = await this.getServer();
        return server.render(...args);
    }
    async renderToHTML(...args) {
        const server = await this.getServer();
        return server.renderToHTML(...args);
    }
    async renderError(...args) {
        const server = await this.getServer();
        return server.renderError(...args);
    }
    async renderErrorToHTML(...args) {
        const server = await this.getServer();
        return server.renderErrorToHTML(...args);
    }
    async render404(...args) {
        const server = await this.getServer();
        return server.render404(...args);
    }
    async prepare(serverFields) {
        const server = await this.getServer();
        if (serverFields) {
            Object.assign(server, serverFields);
        }
        // We shouldn't prepare the server in production,
        // because this code won't be executed when deployed
        if (this.options.dev) {
            await server.prepare();
        }
    }
    async close() {
        if (this.server) {
            await this.server.close();
        }
    }
    async createServer(options) {
        let ServerImplementation;
        if (options.dev) {
            ServerImplementation = require('./dev/next-dev-server').default;
        } else {
            ServerImplementation = await getServerImpl();
        }
        const server = new ServerImplementation(options);
        return server;
    }
    async [SYMBOL_LOAD_CONFIG]() {
        const dir = path.resolve(/* turbopackIgnore: true */ this.options.dir || '.');
        const config = await loadConfig(this.options.dev ? PHASE_DEVELOPMENT_SERVER : PHASE_PRODUCTION_SERVER, dir, {
            customConfig: this.options.conf,
            silent: true
        });
        // check serialized build config when available
        if (!this.options.dev) {
            try {
                const serializedConfig = require(/* turbopackIgnore: true */ path.join(/* turbopackIgnore: true */ dir, config.distDir, SERVER_FILES_MANIFEST + '.json')).config;
                config.experimental.isExperimentalCompile = serializedConfig.experimental.isExperimentalCompile;
            } catch (_) {
            // if distDir is customized we don't know until we
            // load the config so fallback to loading the config
            // from next.config.js
            }
        }
        return config;
    }
    async getServer() {
        if (!this.serverPromise) {
            this.serverPromise = this[SYMBOL_LOAD_CONFIG]().then(async (conf)=>{
                if (!this.options.dev) {
                    if (conf.output === 'standalone') {
                        if (!process.env.__NEXT_PRIVATE_STANDALONE_CONFIG) {
                            log.warn(`"next start" does not work with "output: standalone" configuration. Use "node .next/standalone/server.js" instead.`);
                        }
                    } else if (conf.output === 'export') {
                        throw Object.defineProperty(new Error(`"next start" does not work with "output: export" configuration. Use "npx serve@latest out" instead.`), "__NEXT_ERROR_CODE", {
                            value: "E375",
                            enumerable: false,
                            configurable: true
                        });
                    }
                }
                this.server = await this.createServer({
                    ...this.options,
                    conf
                });
                if (this.preparedAssetPrefix) {
                    this.server.setAssetPrefix(this.preparedAssetPrefix);
                }
                return this.server;
            });
        }
        return this.serverPromise;
    }
    async getServerRequestHandler() {
        if (this.reqHandler) return this.reqHandler;
        // Memoize request handler creation
        if (!this.reqHandlerPromise) {
            this.reqHandlerPromise = this.getServer().then((server)=>{
                this.reqHandler = getTracer().wrap(NextServerSpan.getServerRequestHandler, server.getRequestHandler().bind(server));
                delete this.reqHandlerPromise;
                return this.reqHandler;
            });
        }
        return this.reqHandlerPromise;
    }
}
/** The wrapper server used for `import next from "next" (in a custom server)` */ class NextCustomServer {
    constructor(options){
        this.didWebSocketSetup = false;
        this.options = options;
    }
    getInit() {
        if (!this.init) {
            throw Object.defineProperty(new Error('prepare() must be called before performing this operation'), "__NEXT_ERROR_CODE", {
                value: "E355",
                enumerable: false,
                configurable: true
            });
        }
        return this.init;
    }
    get requestHandler() {
        return this.getInit().requestHandler;
    }
    get upgradeHandler() {
        return this.getInit().upgradeHandler;
    }
    get server() {
        return this.getInit().server;
    }
    get hostname() {
        return this.options.hostname;
    }
    get port() {
        return this.options.port;
    }
    async prepare() {
        const { getRequestHandlers } = require('./lib/start-server');
        let onDevServerCleanup;
        if (this.options.dev) {
            this.cleanupListeners = new AsyncCallbackSet();
            onDevServerCleanup = this.cleanupListeners.add.bind(this.cleanupListeners);
        }
        const initResult = await getRequestHandlers({
            dir: this.options.dir,
            port: this.options.port || 3000,
            isDev: !!this.options.dev,
            onDevServerCleanup,
            hostname: this.options.hostname || 'localhost',
            minimalMode: this.options.minimalMode,
            quiet: this.options.quiet
        });
        this.init = initResult;
    }
    setupWebSocketHandler(customServer, _req) {
        if (!this.didWebSocketSetup) {
            var _req_socket;
            this.didWebSocketSetup = true;
            customServer = customServer || (_req == null ? void 0 : (_req_socket = _req.socket) == null ? void 0 : _req_socket.server);
            if (customServer) {
                customServer.on('upgrade', async (req, socket, head)=>{
                    this.upgradeHandler(req, socket, head);
                });
            }
        }
    }
    getRequestHandler() {
        return async (req, res, parsedUrl)=>{
            this.setupWebSocketHandler(this.options.httpServer, req);
            if (parsedUrl) {
                req.url = formatUrl(parsedUrl);
            }
            return this.requestHandler(req, res);
        };
    }
    async render(...args) {
        let [req, res, pathname, query, parsedUrl] = args;
        this.setupWebSocketHandler(this.options.httpServer, req);
        if (!pathname.startsWith('/')) {
            console.error(`Cannot render page with path "${pathname}"`);
            pathname = `/${pathname}`;
        }
        pathname = pathname === '/index' ? '/' : pathname;
        req.url = formatUrl({
            ...parsedUrl,
            pathname,
            query
        });
        await this.requestHandler(req, res);
        return;
    }
    setAssetPrefix(assetPrefix) {
        var _routerServerGlobal_RouterServerContextSymbol_relativeProjectDir, _routerServerGlobal_RouterServerContextSymbol;
        this.server.setAssetPrefix(assetPrefix);
        // update the router-server nextConfig instance as
        // this is the source of truth for "handler" in serverful
        const relativeProjectDir = path.relative(process.cwd(), this.options.dir || '');
        if ((_routerServerGlobal_RouterServerContextSymbol = routerServerGlobal[RouterServerContextSymbol]) == null ? void 0 : (_routerServerGlobal_RouterServerContextSymbol_relativeProjectDir = _routerServerGlobal_RouterServerContextSymbol[relativeProjectDir]) == null ? void 0 : _routerServerGlobal_RouterServerContextSymbol_relativeProjectDir.nextConfig) {
            routerServerGlobal[RouterServerContextSymbol][relativeProjectDir].nextConfig.assetPrefix = assetPrefix;
        }
    }
    getUpgradeHandler() {
        return this.server.getUpgradeHandler();
    }
    logError(...args) {
        this.server.logError(...args);
    }
    logErrorWithOriginalStack(err, type) {
        return this.server.logErrorWithOriginalStack(err, type);
    }
    async revalidate(...args) {
        return this.server.revalidate(...args);
    }
    async renderToHTML(...args) {
        return this.server.renderToHTML(...args);
    }
    async renderError(...args) {
        return this.server.renderError(...args);
    }
    async renderErrorToHTML(...args) {
        return this.server.renderErrorToHTML(...args);
    }
    async render404(...args) {
        return this.server.render404(...args);
    }
    async close() {
        var _this_init, _this_cleanupListeners;
        await Promise.allSettled([
            (_this_init = this.init) == null ? void 0 : _this_init.server.close(),
            (_this_cleanupListeners = this.cleanupListeners) == null ? void 0 : _this_cleanupListeners.runAll()
        ]);
    }
}
// This file is used for when users run `require('next')`
function createServer(options) {
    // next sets customServer to false when calling this function, in that case we don't want to modify the environment variables
    const isCustomServer = (options == null ? void 0 : options.customServer) ?? true;
    if (isCustomServer) {
        const selectTurbopack = options && (options.turbo || options.turbopack || process.env.IS_TURBOPACK_TEST);
        const selectWebpack = options && (options.webpack || process.env.IS_WEBPACK_TEST);
        if (selectTurbopack && selectWebpack) {
            throw Object.defineProperty(new Error('Pass either `webpack` or `turbopack`, not both.'), "__NEXT_ERROR_CODE", {
                value: "E851",
                enumerable: false,
                configurable: true
            });
        }
        if (selectTurbopack || !selectWebpack) {
            process.env.TURBOPACK ??= selectTurbopack ? '1' : 'auto';
        }
    } else {
        if (options && (options.webpack || options.turbo || options.turbopack)) {
            throw Object.defineProperty(new Error('Only custom servers can pass `webpack`, `turbo`, or `turbopack`.'), "__NEXT_ERROR_CODE", {
                value: "E852",
                enumerable: false,
                configurable: true
            });
        }
    }
    // The package is used as a TypeScript plugin.
    if (options && 'typescript' in options && 'version' in options.typescript) {
        const pluginMod = require('./next-typescript');
        return pluginMod.createTSPlugin(options);
    }
    if (options == null) {
        throw Object.defineProperty(new Error('The server has not been instantiated properly. https://nextjs.org/docs/messages/invalid-server-options'), "__NEXT_ERROR_CODE", {
            value: "E75",
            enumerable: false,
            configurable: true
        });
    }
    if (!('isNextDevCommand' in options) && process.env.NODE_ENV && ![
        'production',
        'development',
        'test'
    ].includes(process.env.NODE_ENV)) {
        log.warn(NON_STANDARD_NODE_ENV);
    }
    if (options.dev && typeof options.dev !== 'boolean') {
        console.warn("Warning: 'dev' is not a boolean which could introduce unexpected behavior. https://nextjs.org/docs/messages/invalid-server-options");
    }
    // When the caller is a custom server (using next()).
    if (options.customServer !== false) {
        const dir = path.resolve(/* turbopackIgnore: true */ options.dir || '.');
        return new NextCustomServer({
            ...options,
            dir
        });
    }
    // When the caller is Next.js internals (i.e. render worker, start server, etc)
    return new NextServer(options);
}
// Support commonjs `require('next')`
module.exports = createServer;
// exports = module.exports
// Support `import next from 'next'`
export default createServer;

//# sourceMappingURL=next.js.map