import { motion, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import {
  Check,
  CircleDashed,
  Cpu,
  DownloadCloud,
  Radar,
  Rocket,
  ScanSearch,
} from "lucide-react";
import type {
  AgentInstallPhase,
  AgentInstallProgress,
} from "@/lib/api/agentInstall";

const PHASE_LABELS: Record<AgentInstallPhase, string> = {
  prepare: "准备环境",
  install: "正在安装",
  verify: "验证结果",
  complete: "安装完成",
};

const ONBOARDING_STEPS = [
  { label: "环境检测", icon: ScanSearch },
  { label: "准备运行时", icon: Cpu },
  { label: "安装 Agent", icon: Rocket },
];

function activeStepForPhase(phase: AgentInstallPhase | null): number {
  if (phase === "prepare") return 1;
  if (phase === "install" || phase === "verify") return 2;
  if (phase === "complete") return 3;
  return 0;
}

export function InstallOnboardingVisual({
  phase,
}: {
  phase: AgentInstallPhase | null;
}) {
  const prefersReducedMotion = useReducedMotion();
  const [demoStep, setDemoStep] = useState(1);

  useEffect(() => {
    if (phase || prefersReducedMotion) return;
    const timer = window.setInterval(() => {
      setDemoStep((step) => (step >= ONBOARDING_STEPS.length ? 1 : step + 1));
    }, 1_400);
    return () => window.clearInterval(timer);
  }, [phase, prefersReducedMotion]);

  const activeStep = phase ? activeStepForPhase(phase) : demoStep;

  return (
    <motion.section
      data-testid="agent-install-onboarding"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="relative overflow-hidden rounded-xl border border-cyan-400/30 bg-slate-950 px-5 py-5 text-slate-100 shadow-lg shadow-cyan-950/20"
    >
      <motion.div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-px bg-cyan-300/80"
        animate={{ x: ["-100%", "100%"] }}
        transition={{ duration: 2.8, repeat: Infinity, ease: "linear" }}
      />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 opacity-20"
        style={{
          backgroundImage:
            "linear-gradient(rgba(34,211,238,0.18) 1px, transparent 1px), linear-gradient(90deg, rgba(34,211,238,0.18) 1px, transparent 1px)",
          backgroundSize: "24px 24px",
        }}
      />
      <div className="relative space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Radar className="h-4 w-4 text-cyan-300" />
            <span className="text-xs font-semibold uppercase tracking-[0.18em] text-cyan-200">
              Agent Install Flow
            </span>
          </div>
          <span className="font-mono text-[10px] text-slate-400">
            CC-LAUNCH / READY
          </span>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {ONBOARDING_STEPS.map(({ label, icon: Icon }, index) => {
            const step = index + 1;
            const complete = activeStep > step;
            const current = activeStep === step;
            return (
              <div key={label} className="flex min-w-0 items-center gap-2">
                <motion.div
                  className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border ${
                    complete || current
                      ? "border-cyan-300/80 bg-cyan-300/15 text-cyan-200"
                      : "border-slate-700 bg-slate-900 text-slate-500"
                  }`}
                  animate={current ? { scale: [1, 1.08, 1] } : { scale: 1 }}
                  transition={
                    current
                      ? { duration: 1.4, repeat: Infinity, ease: "easeInOut" }
                      : { duration: 0.2 }
                  }
                >
                  {complete ? (
                    <Check className="h-4 w-4" />
                  ) : (
                    <Icon className="h-4 w-4" />
                  )}
                </motion.div>
                <span
                  className={`truncate text-xs ${
                    complete || current ? "text-slate-100" : "text-slate-500"
                  }`}
                >
                  {label}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </motion.section>
  );
}

export function InstallProgressVisual({
  progress,
  stepIndex,
  totalSteps,
}: {
  progress: AgentInstallProgress;
  stepIndex: number;
  totalSteps: number;
}) {
  const hasProgress = progress.progress !== null;
  const localProgress = Math.max(0, Math.min(100, progress.progress ?? 0));
  const overallProgress = totalSteps
    ? Math.round(((stepIndex + localProgress / 100) / totalSteps) * 100)
    : localProgress;
  const failed = progress.status === "error";
  const complete = progress.status === "success";

  return (
    <motion.section
      data-testid="agent-install-progress"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className={`relative overflow-hidden rounded-xl border px-5 py-4 ${
        failed
          ? "border-red-400/40 bg-red-950/20"
          : complete
            ? "border-emerald-400/40 bg-emerald-950/20"
            : "border-blue-400/30 bg-slate-950 text-slate-100"
      }`}
    >
      {!failed && !complete && (
        <motion.div
          aria-hidden="true"
          className="pointer-events-none absolute inset-y-0 left-0 w-1/3 bg-cyan-300/10 blur-xl"
          animate={{ x: ["-120%", "360%"] }}
          transition={{ duration: 2.2, repeat: Infinity, ease: "linear" }}
        />
      )}
      <div className="relative space-y-3">
        <div className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 items-start gap-3">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-cyan-300/30 bg-cyan-300/10 text-cyan-200">
              {complete ? (
                <Check className="h-4 w-4" />
              ) : failed ? (
                <CircleDashed className="h-4 w-4 text-red-300" />
              ) : (
                <DownloadCloud className="h-4 w-4" />
              )}
            </div>
            <div className="min-w-0">
              <div className="font-mono text-[10px] uppercase tracking-[0.16em] text-cyan-200/80">
                {PHASE_LABELS[progress.phase]}
              </div>
              <div className="mt-1 truncate text-sm font-medium text-slate-100">
                {progress.message}
              </div>
            </div>
          </div>
          <div className="shrink-0 text-right font-mono text-sm text-cyan-200">
            {hasProgress ? `${localProgress}%` : "进行中"}
            <div className="mt-1 text-[10px] text-slate-500">
              总进度 {overallProgress}%
            </div>
          </div>
        </div>
        <div
          role="progressbar"
          aria-label="Agent 安装进度"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={hasProgress ? localProgress : undefined}
          className="h-1.5 overflow-hidden rounded-full bg-slate-800"
        >
          <motion.div
            className={`h-full rounded-full ${
              failed
                ? "bg-red-400"
                : complete
                  ? "bg-emerald-400"
                  : "bg-cyan-300"
            }`}
            initial={{ width: 0 }}
            animate={{ width: hasProgress ? `${localProgress}%` : "38%" }}
            transition={
              hasProgress
                ? { duration: 0.35, ease: "easeOut" }
                : { duration: 1.2, repeat: Infinity, repeatType: "reverse" }
            }
          />
        </div>
      </div>
    </motion.section>
  );
}
