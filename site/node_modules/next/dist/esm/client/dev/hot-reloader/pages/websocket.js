import { logQueue } from '../../../../next-devtools/userspace/app/forward-logs';
import { HMR_MESSAGE_SENT_TO_BROWSER } from '../../../../server/dev/hot-reloader-types';
import { getSocketUrl } from '../get-socket-url';
import { WEB_SOCKET_MAX_RECONNECTIONS } from '../../../../lib/constants';
let source;
const messageCallbacks = [];
export function addMessageListener(callback) {
    messageCallbacks.push(callback);
}
export function sendMessage(data) {
    if (!source || source.readyState !== source.OPEN) return;
    return source.send(data);
}
let reconnections = 0;
let reloading = false;
let serverSessionId = null;
export function connectHMR(options) {
    let timer;
    function init() {
        if (source) source.close();
        function handleOnline() {
            logQueue.onSocketReady(source);
            reconnections = 0;
            window.console.log('[HMR] connected');
        }
        function handleMessage(event) {
            // While the page is reloading, don't respond to any more messages.
            // On reconnect, the server may send an empty list of changes if it was restarted.
            if (reloading) {
                return;
            }
            const message = JSON.parse(event.data);
            if (message.type === HMR_MESSAGE_SENT_TO_BROWSER.TURBOPACK_CONNECTED) {
                if (serverSessionId !== null && serverSessionId !== message.data.sessionId) {
                    // Either the server's session id has changed and it's a new server, or
                    // it's been too long since we disconnected and we should reload the page.
                    // There could be 1) unhandled server errors and/or 2) stale content.
                    // Perform a hard reload of the page.
                    window.location.reload();
                    reloading = true;
                    return;
                }
                serverSessionId = message.data.sessionId;
            }
            for (const messageCallback of messageCallbacks){
                messageCallback(message);
            }
        }
        function handleDisconnect() {
            source.onerror = null;
            source.onclose = null;
            source.close();
            reconnections++;
            // After 25 reconnects we'll want to reload the page as it indicates the dev server is no longer running.
            if (reconnections > WEB_SOCKET_MAX_RECONNECTIONS) {
                reloading = true;
                window.location.reload();
                return;
            }
            clearTimeout(timer);
            // Try again after 5 seconds
            timer = setTimeout(init, reconnections > 5 ? 5000 : 1000);
        }
        const url = getSocketUrl(options.assetPrefix);
        source = new window.WebSocket(`${url}${options.path}`);
        source.onopen = handleOnline;
        source.onerror = handleDisconnect;
        source.onclose = handleDisconnect;
        source.onmessage = handleMessage;
    }
    init();
}

//# sourceMappingURL=websocket.js.map