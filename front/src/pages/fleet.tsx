import React from "react";
import { api, fmtBytes, statusClass } from "../api";
import { Moon, Square, PowerOff, Skull } from "lucide-react";
import { Page, Section, Msg, errMsg, Btn } from "../ui";

export function AdminFleet() {
  const [rows, setRows] = React.useState<any[]>([]);
  const [disks, setDisks] = React.useState<any[]>([]);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      setRows((await api.vmsList()).data.vms ?? []);
      setDisks((await api.disksList()).data.disks ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);
  React.useEffect(() => {
    load();
    const t = setInterval(load, 8000);
    return () => clearInterval(t);
  }, [load]);

  async function act(id: string, action: string, label: string) {
    setB(true); setM(null);
    try {
      const r = await api.vmAction(id, action);
      setM({ t: "ok", s: `${label}: ${r.data?.status ?? "ok"}` });
      load();
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  return (
    <Page title="FLEET" sub="running machines and their disks">
      <Section title="live machines" note={`${rows.length} awake`}>
        <table>
          <thead><tr><th>tenant</th><th>status</th><th>ip</th><th>tap / container</th><th></th></tr></thead>
          <tbody>
            {rows.map((v) => (
              <tr key={v.tenant_id}>
                <td className="mono">{v.username}</td>
                <td className={statusClass(v.status)}>{v.status}</td>
                <td className="mono">{v.ip ?? "none"}</td>
                <td className="muted mono">{v.tap ?? "none"}</td>
                <td>
                  <Btn disabled={busy} onClick={() => act(v.tenant_id, "sleep", "sleep")}><Moon /> sleep</Btn>{" "}
                  <Btn disabled={busy} onClick={() => act(v.tenant_id, "stop", "stop")}><Square /> stop</Btn>{" "}
                  <Btn disabled={busy} onClick={() => act(v.tenant_id, "shutdown", "shutdown")}><PowerOff /> shutdown</Btn>{" "}
                  <Btn disabled={busy} className="bad" onClick={() => {
                    if (confirm(`kill ${v.username}? container removed, data kept, next wake is fresh`))
                      act(v.tenant_id, "kill", "kill");
                  }}><Skull /> kill</Btn>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Section title="data disks" >
        <table>
          <thead><tr><th>name</th><th>path</th><th>size</th></tr></thead>
          <tbody>
            {disks.map((d) => (
              <tr key={d.path}>
                <td className="mono">{d.name}</td>
                <td className="muted mono">{d.path}</td>
                <td className="num">{fmtBytes(d.size_bytes)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Msg m={m} />
    </Page>
  );
}
