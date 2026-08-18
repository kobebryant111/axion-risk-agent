import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = { children: ReactNode };
type State = { error: Error | null };

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI crashed", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex min-h-screen flex-col items-center justify-center gap-4 bg-[#f6f7fb] p-8 text-center">
          <div className="text-lg font-bold text-red-600">界面渲染出错</div>
          <pre className="max-w-xl whitespace-pre-wrap rounded-2xl bg-white p-4 text-left text-xs text-slate-700 shadow">
            {this.state.error.message}
          </pre>
          <button
            type="button"
            className="rounded-full bg-[#5b5cff] px-5 py-2.5 text-sm font-semibold text-white"
            onClick={() => {
              this.setState({ error: null });
              window.location.reload();
            }}
          >
            重新加载
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
