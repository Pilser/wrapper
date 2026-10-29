import React from "react";
import { api, wsUrl, fmtBytes, ago, statusClass } from "../api";
import { Page, Section, Btn, errMsg } from "../ui";
import { RefreshCw, Users } from "lucide-react";

export function AdminDashboard() {
  const [tenants, setTenants] = React.useState<any[]>([]);
  const [sel, setSel] = React.useState<string>("all");
  const [snaps, setSnaps] = React.useState<any[]>([]);
  const [wsState, setWsState] = React.useState<"off" | "connecting" | "live" | "fallen">("off");
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);

  React.useEffect(() => {
    api.tenantsList()
      .then((r) => setTenants(r.data.tenants ?? []))
      .catch((e) => setM({ t: "bad", s: errMsg(e) }));
  }, []);

  const nameOf = React.useCallback(
    (id: string) => tenants.find((t) => t.id === id)?.username ?? id.slice(0, 8),
    [tenants]
  );

  // REST fallback snapshot poll
  const restPoll = React.useCallback(async () => {
    try {
      const r = await api.dashboard(sel);
      setSnaps(sel === "all" ? r.data.tenants ?? [] : [r.data]);
      setWsState((s) => (s === "live" ? s : "fallen"));
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, [sel]);

  React.useEffect(() => {
    if (wsState === "live") return;
    restPoll();
    const t = setInterval(restPoll, 5000);
    return () => clearInterval(t);
  }, [restPoll, wsState]);

  // WS live stream, ?token= auth (browsers can't set WS headers)
  React.useEffect(() => {
    if (!sel || wsState === "fallen") return;
    let dead = false;
    let ws: WebSocket | null = null;
    try {
      ws = new WebSocket(wsUrl("/api/admin/stream", { tenant: sel }));
      setWsState("connecting");
      ws.onopen = () => { if (!dead) setWsState("live"); };
      ws.onmessage = (ev) => {
        if (dead) return;
        try {
          const d = JSON.parse(ev.data);
          setSnaps(Array.isArray(d.tenants) ? d.tenants : [d]);
        } catch { /* skip frame */ }
      };
      ws.onclose = () => { if (!dead) setWsState("fallen"); };
      ws.onerror = () => { if (!dead) setWsState("fallen"); };
    } catch {
      setWsState("fallen");
    }
    return () => {
      dead = true;
      ws?.close();
    };
  }, [sel, wsState === "fallen"]);

  function Balloon(props: { b: any }) {
    const b = props.b;
    if (!b) return <>none</>;
    return (
      <span>
        {b.actual_mb?.toFixed(0)}/{b.limit_mb?.toFixed(0)} MB · {b.cpu_percent}%
      </span>
    );
  }

  return (
    <Page title="OVERVIEW" sub="fleet at a glance">
      {sel === "all" ? (
        <Section
          title="fleet"
          note={
            <span style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <span className="muted">
                stream: {wsState === "live" ? <span className="ok">live</span> : <span className="warn">{wsState === "connecting" ? "connecting" : "polling"}</span>}
                {m ? <span className={m.t === "bad" ? "bad" : "ok"}> · {m.s}</span> : null}
              </span>
              <select value={sel} onChange={(e) => { setSel(e.target.value); setSnaps([]); }}>
                <option value="all">all tenants</option>
                {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
              </select>
              <Btn className="bare" onClick={() => { setWsState("off"); setSnaps([]); }}><RefreshCw /> refresh</Btn>
            </span>
          }
        >
          <table>
            <thead>
              <tr><th>tenant</th><th>machine</th><th>address</th><th>category</th><th>size</th><th>last seen</th><th>mem · cpu</th><th>disk</th></tr>
            </thead>
            <tbody>
              {snaps.map((s) => (
                <tr key={s.tenant}>
                  <td className="mono">{nameOf(s.tenant)}</td>
                  <td className={statusClass(s.vm?.status)}>{s.vm?.status ?? "none"}</td>
                  <td className="mono">{s.vm?.ip ?? "none"}</td>
                  <td>{s.category?.name ?? "none"}</td>
                  <td className="muted">{s.template ? `${s.template.vcpu_count}c/${s.template.memory_mb}m` : "none"}</td>
                  <td className="num">{ago(s.heartbeat_age_secs)}</td>
                  <td className="num"><Balloon b={s.balloon} /></td>
                  <td className="num">{fmtBytes(s.disk?.size_bytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : snaps.length ? (
        <Section
          title={nameOf(sel)}
          note={
            <span style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <span className="muted">
                stream: {wsState === "live" ? <span className="ok">live</span> : <span className="warn">{wsState === "connecting" ? "connecting" : "polling"}</span>}
              </span>
              <Btn className="bare" onClick={() => { setSel("all"); setSnaps([]); }}><Users /> all tenants</Btn>
              <Btn className="bare" onClick={restPoll}><RefreshCw /> refresh</Btn>
            </span>
          }
        >
          {snaps[0] ? (
            <div>
              {[
                ["machine state", String(snaps[0].vm?.status ?? "none")],
                ["address", snaps[0].vm?.ip ?? "none"],
                ["network tap", snaps[0].vm?.tap ?? "none"],
                ["category", snaps[0].category?.name ?? "none"],
                ["template", snaps[0].template?.name ?? "none"],
                ["cores / memory", snaps[0].template ? `${snaps[0].template.vcpu_count} cores / ${snaps[0].template.memory_mb} MB` : "none"],
                ["last seen", ago(snaps[0].heartbeat_age_secs)],
                ["memory · cpu in use", scr(snaps[0].balloon)],
                ["disk size", fmtBytes(snaps[0].disk?.size_bytes)],
              ].map(([k, v]) => (
                <div className="rowline" key={k}><b>{k}</b><span>{v}</span></div>
              ))}
            </div>
          ) : null}
        </Section>
      ) : null}
    </Page>
  );
}

function scr(b: any) {
  if (!b) return "not reported (allocated only)";
  return `${b.actual_mb?.toFixed(0)}/${b.limit_mb?.toFixed(0)} MB · ${b.cpu_percent}%`;
}
