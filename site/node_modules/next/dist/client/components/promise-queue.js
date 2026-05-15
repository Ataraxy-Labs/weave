/*
    This is a simple promise queue that allows you to limit the number of concurrent promises
    that are running at any given time. It's used to limit the number of concurrent
    prefetch requests that are being made to the server but could be used for other
    things as well.
*/ "use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "PromiseQueue", {
    enumerable: true,
    get: function() {
        return PromiseQueue;
    }
});
class PromiseQueue {
    #maxConcurrency;
    #runningCount;
    #queue;
    constructor(maxConcurrency = 5){
        this.#maxConcurrency = maxConcurrency;
        this.#runningCount = 0;
        this.#queue = [];
    }
    enqueue(promiseFn) {
        let taskResolve;
        let taskReject;
        const taskPromise = new Promise((resolve, reject)=>{
            taskResolve = resolve;
            taskReject = reject;
        });
        const task = async ()=>{
            try {
                this.#runningCount++;
                const result = await promiseFn();
                taskResolve(result);
            } catch (error) {
                taskReject(error);
            } finally{
                this.#runningCount--;
                this.#processNext();
            }
        };
        const enqueueResult = {
            promiseFn: taskPromise,
            task
        };
        // wonder if we should take a LIFO approach here
        this.#queue.push(enqueueResult);
        this.#processNext();
        return taskPromise;
    }
    bump(promiseFn) {
        const index = this.#queue.findIndex((item)=>item.promiseFn === promiseFn);
        if (index > -1) {
            const bumpedItem = this.#queue.splice(index, 1)[0];
            this.#queue.unshift(bumpedItem);
            this.#processNext(true);
        }
    }
    #processNext(forced = false) {
        if ((this.#runningCount < this.#maxConcurrency || forced) && this.#queue.length > 0) {
            this.#queue.shift()?.task();
        }
    }
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=promise-queue.js.map