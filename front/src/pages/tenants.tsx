import React from "react";
import { api, fmtBytes } from "../api";
import { Plus, Save, Pause, Play, CloudUpload, Rocket, Trash2 } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

export function AdminTenants() {
  const [rows, setRows] = React.useState<any[]>([]);
  const [cats, setCats] = React.useState<any[]>([]);
  const [sel, setSel] = React.useState<any | null>(null);
  const [edit, setEdit] = React.useState<any>({ display_name: "", category_id: "", status: "" });
  const [disk, setDisk] = React.useState<any | null>(null);
  const [newT, setNewT] = React.useState({ username: "", password: "", display_name: "", category_id: "" });
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const r = await api.tenantsList();
      setRows(r.data.tenants ?? []);
      const c = await api.categoriesList();
      setCats(c.data.categories ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);

  React.useEffect(() => { load(); }, [load]);

  async function open(id: string) {
    setM(null);
    setDisk(null);
    try {
      const r = await api.tenantGet(id);
      setSel(r.data.tenant);
      setEdit({
        display_name: r.data.tenant.display_name ?? "",
        category_id: r.data.tenant.category_id ?? "",
        status: r.data.tenant.status ?? "",
      });
      try { setDisk((await api.tenantDisk(id)).data); } catch { /* non-fatal */ }
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function run(fn: () => Promise<any>, ok: string) {
    setB(true);
    setM(null);
    try {
      await fn();
      setM({ t: "ok", s: ok });
      await load();
      if (sel) await open(sel.id);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  function create() {
    run(
      () => api.tenantCreate({
        username: newT.username,
        password: newT.password,
        display_name: newT.display_name || undefined,
        category_id: newT.category_id || undefined,
      }),
      "tenant created"
    ).then(() => setNewT({ username: "", password: "", display_name: "", category_id: "" }));
  }

  return (
    <Page title="TENANTS">
      <Section title="all tenants" note={`${rows.length} rows`}>
        <table>
          <thead>
            <tr><th>username</th><th>display</th><th>category</th><th>status</th><th>vm ip</th><th>last login</th><th></th></tr>
          </thead>
          <tbody>
            {rows.map((t) => (
              <tr key={t.id}>
                <td className="mono">{t.username}</td>
                <td>{t.display_name ?? "none"}</td>
                <td className="muted">{cats.find((c) => c.id === t.category_id)?.name ?? t.category_id}</td>
                <td className={t.status === "active" ? "ok" : t.status === "suspended" ? "bad" : "warn"}>{t.status}</td>
                <td className="mono">{t.vm_ip ?? "none"}</td>
                <td className="muted">{t.last_login ?? "none"}</td>
                <td><Btn onClick={() => open(t.id)}>edit</Btn></td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Section title="create tenant">
        <Field label="username"><input value={newT.username} onChange={(e) => setNewT({ ...newT, username: e.target.value })} /></Field>
        <Field label="password"><input value={newT.password} onChange={(e) => setNewT({ ...newT, password: e.target.value })} /></Field>
        <Field label="display name"><input value={newT.display_name} onChange={(e) => setNewT({ ...newT, display_name: e.target.value })} /></Field>
        <Field label="category">
          <select value={newT.category_id} onChange={(e) => setNewT({ ...newT, category_id: e.target.value })}>
            <option value="">(first category)</option>
            {cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
        </Field>
        <Actions><Btn disabled={busy} onClick={create}><Plus /> create</Btn></Actions>
      </Section>

      {sel ? (
        <Section title={`tenant: ${sel.username}`}>
          <Field label="display name">
            <input value={edit.display_name} onChange={(e) => setEdit({ ...edit, display_name: e.target.value })} />
          </Field>
          <Field label="category">
            <select value={edit.category_id} onChange={(e) => setEdit({ ...edit, category_id: e.target.value })}>
              <option value="">(unchanged)</option>
              {cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </Field>
          <Field label="status">
            <select value={edit.status} onChange={(e) => setEdit({ ...edit, status: e.target.value })}>
              <option value="">(unchanged)</option>
              {["active", "suspended", "asleep"].map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          </Field>
          <Actions>
            <Btn disabled={busy} onClick={() => run(() => api.tenantUpdate(sel.id, edit), "saved")}><Save /> save</Btn>
            <Btn disabled={busy} onClick={() => run(() => api.tenantSuspend(sel.id), "suspended")}><Pause /> suspend</Btn>
            <Btn disabled={busy} onClick={() => run(() => api.tenantUnsuspend(sel.id), "unsuspended")}><Play /> unsuspend</Btn>
            <Btn disabled={busy} onClick={() => run(() => api.workspaceSync(sel.id), "SYSTEM.md synced")}><CloudUpload /> sync prompt</Btn>
            <Btn disabled={busy} onClick={() => run(() => api.binaryPush(sel.id), "binaries push queued")}><Rocket /> push programs</Btn>
            <Btn disabled={busy} className="bad" onClick={() => {
              if (confirm(`delete ${sel.username}? VM sleeps + IP is freed`))
                run(() => api.tenantDelete(sel.id), "deleted").then(() => setSel(null));
            }}><Trash2 /> delete</Btn>
          </Actions>

          <div className="rowline"><b>tm disk</b><span>
            {disk
              ? disk.exists
                ? `${fmtBytes(disk.size_bytes)} (${disk.size_gb} GB, used ${fmtBytes(disk.used_bytes)})`
                : "no disk yet"
              : "none"}
          </span></div>
          <div className="rowline"><b>note</b><span className="muted">
            resizing disabled: disks share the host pool
          </span></div>
        </Section>
      ) : null}

      <Msg m={m} />
    </Page>
  );
}
