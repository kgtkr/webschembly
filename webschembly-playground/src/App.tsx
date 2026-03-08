import { useEffect, useRef, useState } from "react";
import { JitGraph, type JitLogEvent } from "./JitGraph";
import playgroundWorker from "./playground.worker?worker";
import type { WorkerRequest, WorkerResponse } from "./worker-types";

const exampleCode = `(define (sum n)
  (define (sum-rec n m)
    (if (= n 0)
      m
      (sum-rec (- n 1) (+ m n))))
  (sum-rec n 0))

(write (sum 100))
(newline)
`;

export default function App() {
  const [src, setSrc] = useState(exampleCode);
  const [enableJitLog, setEnableJitLog] = useState(false);
  const [jitLogs, setJitLogs] = useState<JitLogEvent[]>([]);
  const [stdout, setStdout] = useState("");
  const [stderr, setStderr] = useState("");
  const [exitCode, setExitCode] = useState<number | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [runtimeModule, setRuntimeModule] = useState<WebAssembly.Module | null>(null);
  const workerRef = useRef<Worker | null>(null);
  const [finalDurationMs, setFinalDurationMs] = useState<number | null>(null);

  const formatTime = () => {
    if (!isRunning && finalDurationMs !== null) {
      return `${finalDurationMs.toFixed(2)} ms`;
    } else if (isRunning) {
      return "Running...";
    }
    return "";
  };

  const handleStop = () => {
    if (workerRef.current) {
      workerRef.current.terminate();
      workerRef.current = null;
    }
    setIsRunning(false);
    setStderr((prev) => prev + (prev ? "\n" : "") + "Execution terminated by user.");
    setExitCode((prev) => prev === null ? 130 : prev);
  };

  useEffect(() => {
    fetch(import.meta.env.BASE_URL + "wasm/webschembly_runtime.wasm")
      .then((res) => res.arrayBuffer())
      .then((buf) => {
        setRuntimeModule(new WebAssembly.Module(buf));
      });
  }, []);

  const handleRun = () => {
    if (!runtimeModule || isRunning) return;
    setIsRunning(true);
    setStdout("");
    setStderr("");
    setExitCode(null);

    setFinalDurationMs(null);
    setJitLogs([]);

    const worker = new playgroundWorker();
    workerRef.current = worker;
    const req: WorkerRequest = { src, runtimeModule, enableJitLog };
    worker.postMessage(req);

    worker.addEventListener("message", (event: MessageEvent<WorkerResponse>) => {
      const res = event.data;

      if (res.kind === "finish") {
        setExitCode(res.exitCode);
        setStdout(res.stdout);
        setStderr(res.stderr);
        setFinalDurationMs(res.durationMs);
        setIsRunning(false);
        worker.terminate();
        workerRef.current = null;
      } else if (res.kind === "jit_log") {
        setJitLogs((prev) => [...prev, res.data]);
      }
    });
  };

  return (
    <div className="app-container">
      <header className="header">
        <h1>Webschembly</h1>
        <div className="badge">Playground</div>
      </header>
      <main className="main-content">
        <div className="editor-section panel">
          <div className="section-header">
            <h2>Source Code</h2>
            <div className="editor-controls">
              {(isRunning || finalDurationMs !== null) && <div className="timer">{formatTime()}</div>}
              {isRunning
                ? (
                  <button className="stop-button" onClick={handleStop}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                      <rect x="6" y="6" width="12" height="12" fill="currentColor" />
                    </svg>
                    Stop
                  </button>
                )
                : (
                  <button
                    className={`run-button`}
                    onClick={handleRun}
                    disabled={!runtimeModule}
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                      <path
                        d="M5 3L19 12L5 21V3Z"
                        fill="currentColor"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                    Run Code
                  </button>
                )}
              <div className="visualize-toggle">
                <label>
                  <input
                    type="checkbox"
                    checked={enableJitLog}
                    onChange={(e) => setEnableJitLog(e.target.checked)}
                  />
                  Visualize JIT CFG (Pre-alpha)
                </label>
              </div>
            </div>
          </div>
          <textarea
            className="code-editor"
            value={src}
            onChange={(e) => setSrc(e.target.value)}
            spellCheck="false"
          />
        </div>
        <div className="output-section">
          <div className="output-panel panel">
            <h3>stdout</h3>
            <pre className="output-box">{stdout}</pre>
          </div>
          <div className="output-panel panel">
            <h3>stderr</h3>
            <pre className="output-box error">{stderr}</pre>
          </div>
          <div className="output-panel panel exit-code-panel">
            <h3>Exit Code</h3>
            <div className={`exit-code ${exitCode === 0 ? "success" : exitCode !== null ? "error" : ""}`}>
              {exitCode !== null ? exitCode : "-"}
            </div>
          </div>
          {enableJitLog && (
            <div className="output-panel panel graph-panel">
              <h3>JIT CFG</h3>
              <div className="graph-container" style={{ height: "400px" }}>
                <JitGraph logs={jitLogs} />
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
