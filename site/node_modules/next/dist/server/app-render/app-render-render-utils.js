"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    pipelineInSequentialTasks: null,
    scheduleInSequentialTasks: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    pipelineInSequentialTasks: function() {
        return pipelineInSequentialTasks;
    },
    scheduleInSequentialTasks: function() {
        return scheduleInSequentialTasks;
    }
});
const _invarianterror = require("../../shared/lib/invariant-error");
const _apprenderscheduling = require("./app-render-scheduling");
const _fastsetimmediateexternal = require("../node-environment-extensions/fast-set-immediate.external");
function scheduleInSequentialTasks(render, followup) {
    if (process.env.NEXT_RUNTIME === 'edge') {
        throw Object.defineProperty(new _invarianterror.InvariantError('`scheduleInSequentialTasks` should not be called in edge runtime.'), "__NEXT_ERROR_CODE", {
            value: "E591",
            enumerable: false,
            configurable: true
        });
    } else {
        return new Promise((resolve, reject)=>{
            const scheduleTimeout = (0, _apprenderscheduling.createAtomicTimerGroup)();
            let pendingResult;
            scheduleTimeout(()=>{
                try {
                    (0, _fastsetimmediateexternal.DANGEROUSLY_runPendingImmediatesAfterCurrentTask)();
                    pendingResult = render();
                } catch (err) {
                    reject(err);
                }
            });
            scheduleTimeout(()=>{
                try {
                    (0, _fastsetimmediateexternal.expectNoPendingImmediates)();
                    followup();
                    resolve(pendingResult);
                } catch (err) {
                    reject(err);
                }
            });
        });
    }
}
function pipelineInSequentialTasks(one, two, three) {
    if (process.env.NEXT_RUNTIME === 'edge') {
        throw Object.defineProperty(new _invarianterror.InvariantError('`pipelineInSequentialTasks` should not be called in edge runtime.'), "__NEXT_ERROR_CODE", {
            value: "E875",
            enumerable: false,
            configurable: true
        });
    } else {
        return new Promise((resolve, reject)=>{
            const scheduleTimeout = (0, _apprenderscheduling.createAtomicTimerGroup)();
            let oneResult;
            scheduleTimeout(()=>{
                try {
                    (0, _fastsetimmediateexternal.DANGEROUSLY_runPendingImmediatesAfterCurrentTask)();
                    oneResult = one();
                } catch (err) {
                    clearTimeout(twoId);
                    clearTimeout(threeId);
                    clearTimeout(fourId);
                    reject(err);
                }
            });
            let twoResult;
            const twoId = scheduleTimeout(()=>{
                // if `one` threw, then this timeout would've been cleared,
                // so if we got here, we're guaranteed to have a value.
                try {
                    (0, _fastsetimmediateexternal.DANGEROUSLY_runPendingImmediatesAfterCurrentTask)();
                    twoResult = two(oneResult);
                } catch (err) {
                    clearTimeout(threeId);
                    clearTimeout(fourId);
                    reject(err);
                }
            });
            let threeResult;
            const threeId = scheduleTimeout(()=>{
                // if `two` threw, then this timeout would've been cleared,
                // so if we got here, we're guaranteed to have a value.
                try {
                    (0, _fastsetimmediateexternal.expectNoPendingImmediates)();
                    threeResult = three(twoResult);
                } catch (err) {
                    clearTimeout(fourId);
                    reject(err);
                }
            });
            // We wait a task before resolving/rejecting
            const fourId = scheduleTimeout(()=>{
                resolve(threeResult);
            });
        });
    }
}

//# sourceMappingURL=app-render-render-utils.js.map