import { motion, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import {
  Check,
  CircleDashed,
  Cpu,
  DownloadCloud,
  FileSearch,
  PackageCheck,
  Radar,
  Rocket,
  ScanSearch,
} from "lucide-react";
import type {
  AgentInstallPhase,
  AgentInstallProgress,
  AgentInstallStage,
} from "@/lib/api/agentInstall";

const PHASE_LABELS: Record<AgentInstallPhase, string> = {
  prepare: "准备环境 / Prepare",
  install: "正在安装 / Installing",
  verify: "验证结果 / Verify",
  complete: "安装完成 / Complete",
};

const ONBOARDING_STEPS = [
  { label: "环境检测 / Check", icon: ScanSearch },
  { label: "准备运行时 / Runtime", icon: Cpu },
  { label: "安装 Agent / Install", icon: Rocket },
];

const DETAIL_STAGE_STEPS: Array<{
  id: AgentInstallStage;
  label: string;
  icon: typeof Cpu;
}> = [
  { id: "prepare", label: "环境准备 / Prepare", icon: Cpu },
  { id: "resolve", label: "解析来源 / Resolve", icon: FileSearch },
  { id: "download", label: "下载文件 / Download", icon: DownloadCloud },
  { id: "verify", label: "完整性校验 / Verify", icon: PackageCheck },
  { id: "install", label: "执行安装 / Install", icon: Rocket },
  { id: "complete", label: "完成检测 / Finish", icon: Check },
];

const PHASE_DEFAULT_STAGES: Record<AgentInstallPhase, AgentInstallStage> = {
  prepare: "prepare",
  install: "install",
  verify: "verify",
  complete: "complete",
};

function activeStepForPhase(phase: AgentInstallPhase | null): number {
  if (phase === "prepare") return 1;
  if (phase === "install" || phase === "verify") return 2;
  if (phase === "complete") return 3;
  return 0;
}

function formatBytes(bytes: number): string {
  const trimDecimal = (value: number, digits: number) =>
    value.toFixed(digits).replace(/\.0+$/, "");
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${trimDecimal(bytes / 1024, 1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${trimDecimal(bytes / (1024 * 1024), 1)} MB`;
  return `${trimDecimal(bytes / (1024 * 1024 * 1024), 2)} GB`;
}

function progressDetail(progress: AgentInstallProgress): string | null {
  if (progress.current == null) return null;
  if (progress.unit === "bytes") {
    const current = formatBytes(progress.current);
    return progress.total != null
      ? `${current} / ${formatBytes(progress.total)}`
      : `${current} 已处理`;
  }
  if (progress.unit === "lines") {
    return progress.total != null
      ? `${progress.current} / ${progress.total} 条输出`
      : `${progress.current} 条输出`;
  }
  return progress.total != null
    ? `${progress.current} / ${progress.total}`
    : `${progress.current} 项`;
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
  const activeStage = progress.stage ?? PHASE_DEFAULT_STAGES[progress.phase];
  const activeStageIndex = DETAIL_STAGE_STEPS.findIndex(
    (step) => step.id === activeStage,
  );
  const detail = progressDetail(progress);

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
        <ol
          aria-label="安装细分阶段"
          className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6"
        >
          {DETAIL_STAGE_STEPS.map(({ id, label, icon: Icon }, index) => {
            const stageComplete = complete || index < activeStageIndex;
            const stageCurrent = !complete && index === activeStageIndex;
            return (
              <li
                key={id}
                className={`flex min-w-0 items-center gap-2 rounded-md border px-2 py-2 text-xs ${
                  stageComplete || stageCurrent
                    ? "border-cyan-300/40 bg-cyan-300/10 text-cyan-100"
                    : "border-slate-800 bg-slate-900/70 text-slate-500"
                }`}
              >
                <Icon className="h-3.5 w-3.5 shrink-0" />
                <span className="truncate">{label}</span>
              </li>
            );
          })}
        </ol>
        {detail && (
          <div
            data-testid="agent-install-progress-detail"
            className="flex items-center justify-between gap-3 rounded-md border border-slate-800 bg-slate-900/70 px-3 py-2 font-mono text-[11px] text-slate-300"
          >
            <span>
              {progress.unit === "bytes"
                ? "下载量 / Download"
                : "安装活动 / Activity"}
            </span>
            <span className="text-cyan-200">{detail}</span>
          </div>
        )}
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
