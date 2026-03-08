export type WorkerRequest = {
  src: string;
  runtimeModule: WebAssembly.Module;
  enableJitLog: boolean;
};

export type WorkerResponse = {
  kind: "finish";
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
} | {
  kind: "jit_log";
  data: any;
};
