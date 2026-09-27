import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { writeUiError } from "@/lib/ui-error-log";

type PageErrorBoundaryProps = {
  children: ReactNode;
  resetKey: string;
  onReset: () => void;
};

type PageErrorBoundaryState = {
  error: Error | null;
};

export class PageErrorBoundary extends Component<
  PageErrorBoundaryProps,
  PageErrorBoundaryState
> {
  state: PageErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): PageErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("rDeckForge page render failed", error, info);
    writeUiError({
      source: "error-boundary",
      error,
      componentStack: info.componentStack,
    });
  }

  componentDidUpdate(previous: PageErrorBoundaryProps) {
    if (this.state.error && previous.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  private reset = () => {
    this.setState({ error: null });
    this.props.onReset();
  };

  render() {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <section className="page-error-panel" role="alert">
        <AlertTriangle size={26} />
        <div>
          <strong>当前页面发生异常</strong>
          <span>{this.state.error.message || "未知界面错误"}</span>
        </div>
        <Button variant="primary" onClick={this.reset}>
          <RotateCcw size={15} />
          返回生成页
        </Button>
      </section>
    );
  }
}
