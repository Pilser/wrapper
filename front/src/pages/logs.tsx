import React from "react";
import { api, wsUrl } from "../api";
import { Search, Radio } from "lucide-react";
import { Page, Section, Field, Actions, Btn, errMsg } from "../ui";

export function AdminLogs() {
  const [rows, setRows] = React.useState<any[]>([]);
  const [filters, setFilters] = React.useState({ tenant: "", level: "", kind: "", q: "", limit: "200" });
  const [busies, setBusy] = React.useState(false);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [live, setLive] = React.useState<"off" | "live" | "down">("off");
  const [exp, setExpanded] = React.useState<string | null>(null);

  async function pull() {
    setBusy(true);
    setM(null);
    try {
      const r = await api.logs(filters);
      setRows(r.data.logs ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setBusy(false);
    }
  }

  // live WS feed: each frame is one telemetry event, server-side filtered
  React.useEffect(() => {
    if (live !== "live") return;
    let ws: WebSocket | null = null;
    try {
      ws = new WebSocket(wsUrl("/api/admin/events", {
        level: filters.level || undefined,
        kind: filters.kind || undefined,
        tenant: filters.tenant || undefined,
      }));
      ws.onmessage = (ev) => {
        try { setRows((r) => [JSON.parse(ev.data), ...r].slice(0, 1000)); } catch { /* skip */ }
      };
      ws.onclose = () => setLive((l) => (l === "live" ? "down" : l));
      ws.onerror = () => setLive((l) => (l === "live" ? "down" : l));
    } catch {
      setLive("down");
    }
    return () => ws?.close();
  }, [live]);

  return (
    <Page title="TELEMETRY" sub="system events, newest first">
      <Section title="filter + tail">
        <Field label="tenant"><input value={filters.tenant} onChange={(e) => setFilters({ ...filters, tenant: e.target.value })} placeholder="tenant id or in message" /></Field>
        <Field label="level">
          <select value={filters.level} onChange={(e) => setFilters({ ...filters, level: e.target.value })}>
            <option value="">(any)</option>
            {["debug", "info", "warn", "error", "critical"].map((l) => <option key={l} value={l}>{l}</option>)}
          </select>
        </Field>
        <Field label="kind"><input value={filters.kind} onChange={(e) => setFilters({ ...filters, kind: e.target.value })} placeholder="vm.boot, auth.login, proxy…" /></Field>
        <Field label="contains"><input value={filters.q} onChange={(e) => setFilters({ ...filters, q: e.target.value })} /></Field>
        <Field label="limit">
          <input type="number" value={filters.limit} onChange={(e) => setFilters({ ...filters, limit: e.target.value })} />
        </Field>
        <Actions>
          <Btn disabled={busies} onClick={pull}><Search /> search</Btn>
          <Btn className={live === "live" ? "gold" : ""} onClick={() => setLive((l) => (l === "live" ? "off" : "live"))}>
            <Radio /> live {live === "off" ? "on" : "off"}
          </Btn>
        </Actions>
        <p className="muted">
          live {live === "off" ? "off" : <span className={live === "live" ? "ok" : "bad"}>{live}</span>}
          {m ? <> · <span className="bad">{m.s}</span></> : null}
        </p>
      </Section>

      <Section title="events" note={`${rows.length} shown`}>
        <table>
          <thead><tr><th>time</th><th>level</th><th>type</th><th>tenant</th><th>msg / data</th></tr></thead>
          <tbody>
            {rows.map((e, i) => {
              const key = e.id ?? String(i);
              const open = exp === key;
              return (
                <tr key={key + i} onClick={() => setExpanded(open ? null : key)} style={{ cursor: "pointer" }}>
                  <td className="muted mono">{(e.timestamp ?? "").slice(0, 19)}</td>
                  <td className={e.level === "error" || e.level === "critical" ? "bad" : e.level === "warn" ? "warn" : ""}>{e.level}</td>
                  <td className="mono">{e.type ?? e.kind ?? "none"}</td>
                  <td className="muted mono">{(e.tenant_id ?? "").slice(0, 8) || "none"}</td>
                  <td>
                    {String(e.message ?? "")}
                    {open && e.data ? (
                      <pre className="muted" style={{ marginTop: 4 }}>{JSON.stringify(e.data, null, 2)}</pre>
                    ) : null}
                    {open && e.error ? (
                      <pre className="bad" style={{ marginTop: 4 }}>{JSON.stringify(e.error, null, 2)}</pre>
                    ) : null}
                  </td>
                </tr>
              );
            })}
            {rows.length === 0 ? (
              <tr><td colSpan={5} className="muted">no events</td></tr>
            ) : null}
          </tbody>
        </table>
      </Section>
    </Page>
  );
}
