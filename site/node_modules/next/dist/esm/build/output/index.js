import createStore from 'next/dist/compiled/unistore';
import formatWebpackMessages from '../../shared/lib/format-webpack-messages';
import { store as consoleStore } from './store';
import { COMPILER_NAMES } from '../../shared/lib/constants';
const buildStore = createStore({
    // @ts-expect-error initial value
    client: {},
    // @ts-expect-error initial value
    server: {},
    // @ts-expect-error initial value
    edgeServer: {}
});
let buildWasDone = false;
let clientWasLoading = true;
let serverWasLoading = true;
let edgeServerWasLoading = false;
buildStore.subscribe((state)=>{
    const { client, server, edgeServer, trigger, url } = state;
    const { appUrl } = consoleStore.getState();
    if (client.loading || server.loading || (edgeServer == null ? void 0 : edgeServer.loading)) {
        consoleStore.setState({
            bootstrap: false,
            appUrl: appUrl,
            // If it takes more than 3 seconds to compile, mark it as loading status
            loading: true,
            trigger,
            url
        }, true);
        clientWasLoading = !buildWasDone && clientWasLoading || client.loading;
        serverWasLoading = !buildWasDone && serverWasLoading || server.loading;
        edgeServerWasLoading = !buildWasDone && edgeServerWasLoading || edgeServer.loading;
        buildWasDone = false;
        return;
    }
    buildWasDone = true;
    let partialState = {
        bootstrap: false,
        appUrl: appUrl,
        loading: false,
        typeChecking: false,
        totalModulesCount: (clientWasLoading ? client.totalModulesCount : 0) + (serverWasLoading ? server.totalModulesCount : 0) + (edgeServerWasLoading ? (edgeServer == null ? void 0 : edgeServer.totalModulesCount) || 0 : 0),
        hasEdgeServer: !!edgeServer
    };
    if (client.errors && clientWasLoading) {
        // Show only client errors
        consoleStore.setState({
            ...partialState,
            errors: client.errors,
            warnings: null
        }, true);
    } else if (server.errors && serverWasLoading) {
        consoleStore.setState({
            ...partialState,
            errors: server.errors,
            warnings: null
        }, true);
    } else if (edgeServer.errors && edgeServerWasLoading) {
        consoleStore.setState({
            ...partialState,
            errors: edgeServer.errors,
            warnings: null
        }, true);
    } else {
        // Show warnings from all of them
        const warnings = [
            ...client.warnings || [],
            ...server.warnings || [],
            ...edgeServer.warnings || []
        ];
        consoleStore.setState({
            ...partialState,
            errors: null,
            warnings: warnings.length === 0 ? null : warnings
        }, true);
    }
});
export function watchCompilers(client, server, edgeServer) {
    buildStore.setState({
        client: {
            loading: true
        },
        server: {
            loading: true
        },
        edgeServer: {
            loading: true
        },
        trigger: 'initial',
        url: undefined
    });
    function tapCompiler(key, compiler, onEvent) {
        compiler.hooks.invalid.tap(`NextJsInvalid-${key}`, ()=>{
            onEvent({
                loading: true
            });
        });
        compiler.hooks.done.tap(`NextJsDone-${key}`, (stats)=>{
            const { errors, warnings } = formatWebpackMessages(stats.toJson({
                preset: 'errors-warnings',
                moduleTrace: true
            }));
            const hasErrors = !!(errors == null ? void 0 : errors.length);
            const hasWarnings = !!(warnings == null ? void 0 : warnings.length);
            onEvent({
                loading: false,
                totalModulesCount: stats.compilation.modules.size,
                errors: hasErrors ? errors : null,
                warnings: hasWarnings ? warnings : null
            });
        });
    }
    tapCompiler(COMPILER_NAMES.client, client, (status)=>{
        if (!status.loading && !buildStore.getState().server.loading && !buildStore.getState().edgeServer.loading && status.totalModulesCount > 0) {
            buildStore.setState({
                client: status,
                trigger: undefined,
                url: undefined
            });
        } else {
            buildStore.setState({
                client: status
            });
        }
    });
    tapCompiler(COMPILER_NAMES.server, server, (status)=>{
        if (!status.loading && !buildStore.getState().client.loading && !buildStore.getState().edgeServer.loading && status.totalModulesCount > 0) {
            buildStore.setState({
                server: status,
                trigger: undefined,
                url: undefined
            });
        } else {
            buildStore.setState({
                server: status
            });
        }
    });
    tapCompiler(COMPILER_NAMES.edgeServer, edgeServer, (status)=>{
        if (!status.loading && !buildStore.getState().client.loading && !buildStore.getState().server.loading && status.totalModulesCount > 0) {
            buildStore.setState({
                edgeServer: status,
                trigger: undefined,
                url: undefined
            });
        } else {
            buildStore.setState({
                edgeServer: status
            });
        }
    });
}
export function reportTrigger(trigger, url) {
    buildStore.setState({
        trigger,
        url
    });
}

//# sourceMappingURL=index.js.map