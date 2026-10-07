// A chart of a view or a selection (docs/ASK.md): bars, a line, or a
// scatter, drawn as SVG from the numbers `db.chart` gives.
//
// One axis, one baseline. Bars are thin with a rounded end and a gap of
// surface between them; lines are 2 px with ringed markers. Two or more
// series get a legend, and the series colours are four steps of the app's
// label colours in a fixed order, checked for colour-blind separation and
// contrast in both appearances (database.css). Text is always ink, never a
// series colour. Hovering reads every series at that place; the same
// numbers are a table one click away, so nothing depends on the hover.

import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { ChartData, ChartSpec } from './api';

const H = 240;
const PAD = { top: 12, right: 16, bottom: 44, left: 52 };
export const MAX_SERIES = 4;

/** Round steps for an axis: 0, 50, 100, never 0, 47.3, 94.6. */
function ticks(lo: number, hi: number, about = 4): number[] {
  if (!(hi > lo)) return [lo];
  const raw = (hi - lo) / about;
  const pow = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * pow).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(Math.round(v * 1e9) / 1e9);
  return out;
}

const fmt = (n: number) =>
  Math.abs(n) >= 1000 ? n.toLocaleString('en-US', { maximumFractionDigits: 0 }) : n.toLocaleString('en-US', { maximumFractionDigits: 2 });

/** A day count as a short date, for a date axis. */
function dayLabel(days: number): string {
  const d = new Date(Math.round(days) * 86_400_000);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' });
}

interface Props {
  spec: ChartSpec;
  data: ChartData | null;
  error: string | null;
}

export function Chart({ spec, data, error }: Props) {
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(560);
  const [hover, setHover] = useState<number | null>(null);
  const [asTable, setAsTable] = useState(false);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setWidth(Math.max(280, el.clientWidth));
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(el);
    return () => seen.disconnect();
  }, []);

  const series = useMemo(() => (data?.series ?? []).slice(0, MAX_SERIES), [data]);
  const n = data?.labels.length ?? 0;
  const innerW = width - PAD.left - PAD.right;
  const innerH = H - PAD.top - PAD.bottom;

  const scale = useMemo(() => {
    const values = series.flatMap((s) => s.values).filter((v): v is number => v !== null);
    const lo = Math.min(0, ...values);
    const hi = Math.max(0, ...values);
    const yt = ticks(lo, hi === lo ? lo + 1 : hi);
    const y0 = Math.min(lo, yt[0]);
    const y1 = Math.max(hi, yt[yt.length - 1]);
    const y = (v: number) => PAD.top + innerH - ((v - y0) / (y1 - y0 || 1)) * innerH;
    // A scatter, or a line over numbers or dates, places x by its value; the rest by its place in order.
    const xs = data?.xs ?? [];
    const numeric =
      spec.type !== 'bar' && data?.x.kind !== 'category' && xs.length > 0 && xs.every((v) => v !== null);
    const xlo = numeric ? Math.min(...(xs as number[])) : 0;
    const xhi = numeric ? Math.max(...(xs as number[])) : 1;
    const band = n > 0 ? innerW / n : innerW;
    const x = (i: number) =>
      numeric
        ? PAD.left + (xhi === xlo ? innerW / 2 : (((xs[i] as number) - xlo) / (xhi - xlo)) * innerW)
        : PAD.left + band * (i + 0.5);
    return { y, yt, x, band, numeric, xlo, xhi, zero: y(Math.max(y0, Math.min(0, y1))) };
  }, [series, data, spec.type, n, innerW, innerH]);

  // The box is always there, so its width is known before the first chart is drawn in it.
  const empty = error ?? (!data ? 'Drawing…' : n === 0 || series.length === 0 ? 'No rows to draw.' : null);
  if (empty !== null || !data) {
    return (
      <div className="db-chart" ref={box}>
        <p className="db-chart-empty">{empty}</p>
      </div>
    );
  }

  const xText = (i: number) => (data.x.kind === 'date' && data.xs[i] !== null ? dayLabel(data.xs[i] as number) : data.labels[i]);
  // As many x labels as fit, evenly spread, never overlapping.
  const every = Math.max(1, Math.ceil(n / Math.max(1, Math.floor(innerW / 64))));
  const xTicks = scale.numeric
    ? ticks(scale.xlo, scale.xhi, Math.max(2, Math.floor(innerW / 90))).map((v) => ({
        at: PAD.left + ((v - scale.xlo) / (scale.xhi - scale.xlo || 1)) * innerW,
        text: data.x.kind === 'date' ? dayLabel(v) : fmt(v),
      }))
    : data.labels.map((_, i) => ({ at: scale.x(i), text: xText(i) })).filter((_, i) => i % every === 0);

  const barW = Math.min(24, Math.max(2, (scale.band - 6) / series.length - 2));
  const nearest = (clientX: number, clientY: number, el: SVGSVGElement) => {
    const r = el.getBoundingClientRect();
    const px = ((clientX - r.left) / r.width) * width;
    const py = ((clientY - r.top) / r.height) * H;
    let best = 0;
    let bestD = Infinity;
    for (let i = 0; i < n; i++) {
      const dx = scale.x(i) - px;
      // A scatter is aimed at in both directions; the others only along the bottom.
      const v = series[0].values[i];
      const dy = spec.type === 'scatter' && v !== null ? scale.y(v) - py : 0;
      const d = dx * dx + dy * dy;
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    }
    setHover(best);
  };

  return (
    <div className="db-chart" ref={box}>
      <div className="db-chart-bar">
        {series.length > 1 && (
          <ul className="db-legend" aria-label="Series">
            {series.map((s, i) => (
              <li key={s.name}>
                <span className="db-key" data-series={i} data-shape={spec.type === 'bar' ? 'box' : 'line'} aria-hidden="true" />
                {s.name}
              </li>
            ))}
          </ul>
        )}
        {series.length === 1 && <span className="db-chart-title">{series[0].name} by {data.x.name}</span>}
        <button type="button" className="db-quiet" aria-pressed={asTable} onClick={() => setAsTable((t) => !t)}>
          {asTable ? 'Chart' : 'Table'}
        </button>
      </div>
      {asTable ? (
        <div className="db-chart-table">
          <table>
            <thead>
              <tr>
                <th>{data.x.name}</th>
                {series.map((s) => (
                  <th key={s.name}>{s.name}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.labels.map((label, i) => (
                <tr key={i}>
                  <td>{label}</td>
                  {series.map((s) => (
                    <td key={s.name} data-numeric>
                      {s.values[i] === null ? '' : fmt(s.values[i] as number)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="db-chart-plot">
          <svg
            viewBox={`0 0 ${width} ${H}`}
            width={width}
            height={H}
            role="img"
            aria-label={`${spec.type} chart of ${series.map((s) => s.name).join(', ')} by ${data.x.name}, ${n} points. The Table button shows the numbers.`}
            onPointerMove={(e) => nearest(e.clientX, e.clientY, e.currentTarget)}
            onPointerLeave={() => setHover(null)}
          >
            {scale.yt.map((t) => (
              <g key={t}>
                <line className="db-gridline" x1={PAD.left} x2={width - PAD.right} y1={scale.y(t)} y2={scale.y(t)} />
                <text className="db-axis-text" x={PAD.left - 8} y={scale.y(t)} textAnchor="end" dominantBaseline="middle">
                  {fmt(t)}
                </text>
              </g>
            ))}
            <line className="db-baseline" x1={PAD.left} x2={width - PAD.right} y1={scale.zero} y2={scale.zero} />
            {xTicks.map((t, i) => (
              <text key={i} className="db-axis-text" x={t.at} y={H - PAD.bottom + 16} textAnchor="middle">
                {t.text.length > 12 ? `${t.text.slice(0, 11)}…` : t.text}
              </text>
            ))}
            <text className="db-axis-name" x={PAD.left + innerW / 2} y={H - 6} textAnchor="middle">
              {data.x.name}
            </text>

            {hover !== null && spec.type !== 'bar' && (
              <line className="db-crosshair" x1={scale.x(hover)} x2={scale.x(hover)} y1={PAD.top} y2={PAD.top + innerH} />
            )}

            {spec.type === 'bar' &&
              series.map((s, si) =>
                s.values.map((v, i) => {
                  if (v === null) return null;
                  const x = scale.x(i) - (series.length * (barW + 2)) / 2 + si * (barW + 2) + 1;
                  const top = Math.min(scale.y(v), scale.zero);
                  const h = Math.max(1, Math.abs(scale.y(v) - scale.zero));
                  const r = Math.min(4, barW / 2, h);
                  // Rounded at the data end, square on the baseline.
                  const d =
                    v >= 0
                      ? `M${x},${top + h} V${top + r} Q${x},${top} ${x + r},${top} H${x + barW - r} Q${x + barW},${top} ${x + barW},${top + r} V${top + h} Z`
                      : `M${x},${top} V${top + h - r} Q${x},${top + h} ${x + r},${top + h} H${x + barW - r} Q${x + barW},${top + h} ${x + barW},${top + h - r} V${top} Z`;
                  return <path key={`${si}-${i}`} className="db-mark" data-series={si} data-lift={hover === i || undefined} d={d} />;
                }),
              )}

            {spec.type === 'line' &&
              series.map((s, si) => {
                const pts = s.values.map((v, i) => (v === null ? null : `${scale.x(i)},${scale.y(v)}`)).filter(Boolean);
                return (
                  <g key={si}>
                    <polyline className="db-line" data-series={si} points={pts.join(' ')} />
                    {(n <= 40 ? s.values : []).map((v, i) =>
                      v === null ? null : (
                        <circle key={i} className="db-dot" data-series={si} cx={scale.x(i)} cy={scale.y(v)} r={hover === i ? 5 : 4} />
                      ),
                    )}
                  </g>
                );
              })}

            {spec.type === 'scatter' &&
              series.map((s, si) =>
                s.values.map((v, i) =>
                  v === null ? null : (
                    <circle key={`${si}-${i}`} className="db-dot" data-series={si} cx={scale.x(i)} cy={scale.y(v)} r={hover === i ? 6 : 4} />
                  ),
                ),
              )}
          </svg>
          {hover !== null && (
            <div
              className="db-tip"
              role="status"
              style={{
                left: `${Math.min(Math.max(scale.x(hover), 80), width - 80)}px`,
                top: 0,
              }}
            >
              <div className="db-tip-x">{xText(hover)}</div>
              {series.map((s, si) => (
                <div key={s.name} className="db-tip-row">
                  <span className="db-key" data-series={si} data-shape="line" aria-hidden="true" />
                  <strong>{s.values[hover] === null ? '—' : fmt(s.values[hover] as number)}</strong>
                  <span data-text="secondary">{s.name}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
