import React from 'react';
import { AbsoluteFill, Composition, interpolate, useCurrentFrame } from 'remotion';

const shell = `curl -fsSL https://raw.githubusercontent.com/austindixson/orca-agent/main/scripts/install-orca.sh | bash`;

const card: React.CSSProperties = {
  background: 'rgba(255,255,255,0.06)',
  border: '1px solid rgba(255,255,255,0.12)',
  borderRadius: 18,
  padding: '24px 28px',
  boxShadow: '0 20px 60px rgba(0,0,0,0.35)',
};

const heading: React.CSSProperties = {
  fontSize: 54,
  fontWeight: 700,
  letterSpacing: 0.4,
  marginBottom: 10,
};

const row = (label: string, value: string, y: number) => ({ label, value, y });

const installRows = [
  row('1', 'Run installer', 0),
  row('2', 'Choose prebuilt or source', 1),
  row('3', 'Run setup wizard', 2),
  row('4', 'Launch: orca', 3),
];

const setupRows = [
  row('Provider', 'Z.AI (GLM)', 0),
  row('Base URL', 'https://api.z.ai/api/coding/paas/v4', 1),
  row('Model', 'GLM-4.7', 2),
  row('Run', 'orca setup --defaults', 3),
];

const Frame: React.FC<{ title: string; subtitle: string; lines: { label: string; value: string; y: number }[]; cmd?: string }> = ({
  title,
  subtitle,
  lines,
  cmd,
}) => {
  const f = useCurrentFrame();
  const fade = interpolate(f, [0, 20], [0, 1], { extrapolateRight: 'clamp' });
  return (
    <AbsoluteFill
      style={{
        fontFamily: 'Inter, ui-sans-serif, system-ui',
        color: '#f8fafc',
        background: 'radial-gradient(circle at 20% 10%, #1e293b 0%, #0b1020 50%, #05070d 100%)',
        opacity: fade,
        padding: 64,
        display: 'flex',
        gap: 24,
      }}
    >
      <div style={{ ...card, flex: 1 }}>
        <div style={heading}>{title}</div>
        <div style={{ opacity: 0.8, fontSize: 28, marginBottom: 20 }}>{subtitle}</div>
        <div style={{ display: 'grid', gap: 14 }}>
          {lines.map((line, i) => (
            <div
              key={line.label}
              style={{
                background: 'rgba(15,23,42,0.65)',
                border: '1px solid rgba(148,163,184,0.35)',
                borderRadius: 12,
                padding: '12px 14px',
                fontSize: 26,
                transform: `translateY(${interpolate(f, [8 + i * 6, 22 + i * 6], [8, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })}px)`,
                opacity: interpolate(f, [8 + i * 6, 22 + i * 6], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' }),
              }}
            >
              <span style={{ color: '#38bdf8', fontWeight: 700, marginRight: 10 }}>{line.label}</span>
              <span>{line.value}</span>
            </div>
          ))}
        </div>
      </div>
      {cmd ? (
        <div style={{ ...card, width: 760, alignSelf: 'flex-end' }}>
          <div style={{ fontSize: 24, opacity: 0.75, marginBottom: 8 }}>Command</div>
          <div
            style={{
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
              fontSize: 20,
              lineHeight: 1.5,
              color: '#a7f3d0',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            }}
          >
            {cmd}
          </div>
        </div>
      ) : null}
    </AbsoluteFill>
  );
};

export const InstallFlow: React.FC = () => (
  <Frame title="Orca CLI quick install" subtitle="macOS / Linux + setup handoff" lines={installRows} cmd={shell} />
);

export const SetupFlow: React.FC = () => (
  <Frame
    title="GLM setup preset"
    subtitle="Provider parity in CLI wizard"
    lines={setupRows}
    cmd={`PORT=9001 ZAI_API_KEY=*** ORCA_LLM_BASE_URL=https://api.z.ai/api/coding/paas/v4 ORCA_MODEL=GLM-4.7 orca setup --defaults`}
  />
);

export const Root: React.FC = () => {
  return (
    <>
      <Composition id="InstallFlow" component={InstallFlow} durationInFrames={120} fps={30} width={1600} height={900} />
      <Composition id="SetupFlow" component={SetupFlow} durationInFrames={120} fps={30} width={1600} height={900} />
    </>
  );
};
