import { useState, useEffect, useRef } from 'react';
import playgroundWorker from './playground.worker?worker';

const exampleCode =
    `(define (factorial n)
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))
(write (factorial 5))
(newline)
`;

export default function App() {
    const [src, setSrc] = useState(exampleCode);
    const [stdout, setStdout] = useState('');
    const [stderr, setStderr] = useState('');
    const [exitCode, setExitCode] = useState<number | null>(null);
    const [isRunning, setIsRunning] = useState(false);
    const [runtimeModule, setRuntimeModule] = useState<WebAssembly.Module | null>(null);

    useEffect(() => {
        fetch(import.meta.env.BASE_URL + 'wasm/webschembly_runtime.wasm')
            .then((res) => res.arrayBuffer())
            .then((buf) => {
                setRuntimeModule(new WebAssembly.Module(buf))
            });
    }, []);

    const handleRun = () => {
        if (!runtimeModule || isRunning) return;
        setIsRunning(true);
        setStdout('');
        setStderr('');
        setExitCode(null);

        const worker = new playgroundWorker();
        worker.postMessage({ src, runtimeModule });

        worker.addEventListener('message', (event) => {
            const { exitCode: code, stdout: out, stderr: err } = event.data;
            setExitCode(code);
            setStdout(out);
            setStderr(err);
            setIsRunning(false);
            worker.terminate();
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
                        <button
                            className={`run-button ${isRunning ? 'running' : ''}`}
                            onClick={handleRun}
                            disabled={isRunning || !runtimeModule}
                        >
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <path d="M5 3L19 12L5 21V3Z" fill="currentColor" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                            </svg>
                            {isRunning ? 'Running...' : 'Run Code'}
                        </button>
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
                        <div className={`exit-code ${exitCode === 0 ? 'success' : exitCode !== null ? 'error' : ''}`}>
                            {exitCode !== null ? exitCode : '-'}
                        </div>
                    </div>
                </div>
            </main>
        </div>
    );
}
