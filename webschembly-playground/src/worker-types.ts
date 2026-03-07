export type WorkerRequest = {
    src: string;
    runtimeModule: WebAssembly.Module;
};

export type WorkerResponse =
    | {
        kind: 'progress';
        elapsedMs: number;
    }
    | {
        kind: 'finish';
        exitCode: number;
        stdout: string;
        stderr: string;
        durationMs: number;
    };
