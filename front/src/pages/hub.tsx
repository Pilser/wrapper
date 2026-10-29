import React from "react";
import { api } from "../api";
import { RefreshCw, Ban, Plus, Trash2 } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

export function AdminHub() {
  const [tenants, setTenants] = React.useState<any[]>([]);
  const [owner, setOwner] = React.useState("");
  const [view, setView] = React.useState<any>(null);
  const [grantee, setGrantee] = React.useState("");
  const [expires, setExpires] = React.useState("");
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try { setTenants((await api.tenantsList()).data.tenants ?? []); }
    catch (e) { setM({ t: "bad", s: errMsg(e) }); }
  }, []);
  React.useEffect(() => { load(); }, [load]);

  async function pull() {
    if (!owner) return;
    setM(null);
    try {
      setView((await api.hubGrants(owner)).data);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function run(fn: () => Promise<any>, ok: string) {
    setB(true); setM(null);
    try {
      await fn();
      setM({ t: "ok", s: ok });
      pull();
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  return (
    <Page title="HUB GRANTS" sub="portfolio sharing">
      <Section title="owner">
        <Field label="owner">
          <select value={owner} onChange={(e) => { setOwner(e.target.value); setView(null); }}>
            <option value="">(select)</option>
            {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
          </select>
        </Field>
        <Actions><Btn disabled={!owner} onClick={pull}><RefreshCw /> load shares</Btn></Actions>
      </Section>

      {view ? (
        <>
          <Section title="direct grants" note="share codes are kept private">
            <table>
              <thead><tr><th>grantee</th><th>created</th><th>expires</th><th></th></tr></thead>
              <tbody>
                {(view.grants ?? []).map((g: any, i: number) => (
                  <tr key={i}>
                    <td className="mono">{g.username ?? g.tenant}</td>
                    <td className="muted">{g.created_at?.slice(0, 19)}</td>
                    <td className="muted">{g.expires_at?.slice(0, 19) ?? "never"}</td>
                    <td><Btn className="bad" disabled={busy} onClick={() =>
                      run(() => api.hubGrantDelete({ owner_tenant: owner, grantee_tenant: g.tenant }), "grant revoked")
                    }><Ban /> revoke</Btn></td>
                  </tr>
                ))}
                {(view.grants ?? []).length === 0 ? (
                  <tr><td colSpan={4} className="muted">no direct grants</td></tr>
                ) : null}
              </tbody>
            </table>
            <p className="muted">share codes: {(view.codes ?? []).length} active · blocked: {(view.blocked ?? []).length}</p>
          </Section>

          <Section title="create grant">
            <Field label="grantee">
              <select value={grantee} onChange={(e) => setGrantee(e.target.value)}>
                <option value="">(select)</option>
                {tenants.filter((t) => t.id !== owner).map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
              </select>
            </Field>
            <Field label="expires at"><input value={expires} onChange={(e) => setExpires(e.target.value)} placeholder="RFC3339 (optional)" /></Field>
            <Actions><Btn disabled={busy || !grantee} onClick={() => {
              run(() => api.hubGrantCreate({
                owner_tenant: owner, grantee_tenant: grantee,
                expires_at: expires || undefined,
              }), "grant created").then(() => setGrantee(""));
            }}><Plus /> grant</Btn></Actions>
          </Section>
        </>
      ) : null}

      <Msg m={m} />
    </Page>
  );
}
