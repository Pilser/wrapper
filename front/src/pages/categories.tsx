import React from "react";
import { api } from "../api";
import { Plus, Save, Trash2, Eraser } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

export function AdminCategories() {
  const [rows, setRows] = React.useState<any[]>([]);
  const [tpls, setTpls] = React.useState<any[]>([]);
  const [sel, setSel] = React.useState<string | null>(null);
  const [detail, setDetail] = React.useState<any>(null);
  const [edit, setEdit] = React.useState<any>({});
  const [tools, setTools] = React.useState<{ tools: string[]; restricted: boolean }>({ tools: [], restricted: false });
  const [catalog, setCatalog] = React.useState<any[]>([]);
  const [toolText, setToolText] = React.useState("");
  const [configs, setConfigs] = React.useState<any[]>([]);
  const [newCfg, setNewCfg] = React.useState({ config_type: "file", name: "", path: "", content: "", priority: 0 });
  const [n, setN] = React.useState({ name: "", vm_template_id: "", description: "", max_tenants: "" });
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      setRows((await api.categoriesList()).data.categories ?? []);
      setTpls((await api.templatesList()).data.templates ?? []);
      try { setCatalog((await api.toolsCatalog()).data.tools ?? []); } catch { /* non-fatal */ }
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);
  React.useEffect(() => { load(); }, [load]);

  async function open(id: string) {
    setM(null);
    setSel(id);
    setDetail(null);
    setConfigs([]);
    try {
      const c = (await api.categoryGet(id)).data.category;
      setDetail(c);
      setEdit({ name: c.name ?? "", description: c.description ?? "", vm_template_id: c.vm_template_id ?? "", max_tenants: c.max_tenants ?? "", status: c.status ?? "active" });
    } catch (e) { setM({ t: "bad", s: errMsg(e) }); }
    try {
      const t = (await api.categoryTools(id)).data;
      setTools(t);
      setToolText((t.tools ?? []).join("\n"));
    } catch { /* non-fatal */ }
    try {
      setConfigs((await api.categoryConfigs(id)).data.configs ?? []);
    } catch { /* non-fatal */ }
  }

  async function run(fn: () => Promise<any>, ok: string) {
    setB(true); setM(null);
    try { await fn(); setM({ t: "ok", s: ok }); await load(); if (sel) await open(sel); }
    catch (e) { setM({ t: "bad", s: errMsg(e) }); }
    finally { setB(false); }
  }

  return (
    <Page title="CATEGORIES" sub="machine groups for kinds of work">
      <Section title="all categories" note={`${rows.length} rows`}>
        <table>
          <thead><tr><th>name</th><th>template</th><th>max tenants</th><th>status</th><th>tools</th><th>configs</th><th></th></tr></thead>
          <tbody>
            {rows.map((c) => (
              <tr key={c.id}>
                <td className="mono">{c.name}</td>
                <td className="muted">{tpls.find((t) => t.id === c.vm_template_id)?.name ?? c.vm_template_id}</td>
                <td className="num">{c.max_tenants ?? "none"}</td>
                <td className={c.status === "Active" || c.status === "active" ? "ok" : "muted"}>
                  {String(c.status).toLowerCase()}
                </td>
                <td>{c.tools?.restricted ? <span className="warn">restricted</span> : <span className="muted">all</span>}</td>
                <td className="num">{c.configs ?? "none"}</td>
                <td><Btn onClick={() => open(c.id)}>edit</Btn></td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Section title="create category">
        <Field label="name"><input value={n.name} onChange={(e) => setN({ ...n, name: e.target.value })} /></Field>
        <Field label="vm template">
          <select value={n.vm_template_id} onChange={(e) => setN({ ...n, vm_template_id: e.target.value })}>
            <option value="">(select)</option>
            {tpls.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </Field>
        <Field label="description"><input value={n.description} onChange={(e) => setN({ ...n, description: e.target.value })} /></Field>
        <Field label="max tenants"><input type="number" value={n.max_tenants} onChange={(e) => setN({ ...n, max_tenants: e.target.value })} /></Field>
        <Actions><Btn disabled={busy} onClick={() => {
          run(() => api.categoryCreate({
            name: n.name, vm_template_id: n.vm_template_id,
            description: n.description || undefined,
            max_tenants: n.max_tenants ? +n.max_tenants : undefined,
          }), "category created").then(() => setN({ name: "", vm_template_id: "", description: "", max_tenants: "" }));
        }}><Plus /> create</Btn></Actions>
      </Section>

      {sel && detail ? (
        <>
          <Section title={`category: ${detail.name}`}>
            <Field label="name"><input value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} /></Field>
            <Field label="description"><input value={edit.description} onChange={(e) => setEdit({ ...edit, description: e.target.value })} /></Field>
            <Field label="vm template">
              <select value={edit.vm_template_id} onChange={(e) => setEdit({ ...edit, vm_template_id: e.target.value })}>
                <option value="">(unchanged)</option>
                {tpls.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
              </select>
            </Field>
            <Field label="max tenants"><input type="number" value={edit.max_tenants} onChange={(e) => setEdit({ ...edit, max_tenants: e.target.value })} /></Field>
            <Actions>
              <Btn disabled={busy} onClick={() => run(() => api.categoryUpdate(sel, edit), "saved")}><Save /> save</Btn>
              <Btn disabled={busy} className="bad" onClick={() => {
                if (confirm(`delete category ${detail.name}?`))
                  run(() => api.categoryDelete(sel), "deleted").then(() => { setSel(null); setDetail(null); });
              }}><Trash2 /> delete</Btn>
            </Actions>
          </Section>

          <Section title="tool grants">
            <Field label="restricted">
              <select value={tools.restricted ? "1" : "0"} disabled>
                <option value="0">no, every tool allowed</option>
                <option value="1">yes, grant list applies</option>
              </select>
            </Field>
            <Field label="grants">
              <textarea value={toolText} onChange={(e) => setToolText(e.target.value)} placeholder={"portfolio.show\nportfolio.list_goals"} />
            </Field>
            <p className="muted">
              catalog: {catalog.length} tools.{" "}
              {catalog.slice(0, 8).map((t: any) => t.name).join(", ")}
              {catalog.length > 8 ? ", …" : ""}
            </p>
            <Actions>
              <Btn disabled={busy} onClick={() => run(() =>
                api.categoryToolsSet(sel, toolText.split("\n").map((s) => s.trim()).filter(Boolean)),
                "tool grants replaced"
              )}><Save /> save grants</Btn>
              <Btn disabled={busy} onClick={() => run(() => api.categoryToolsSet(sel, []), "restriction cleared")}><Eraser /> clear restriction</Btn>
            </Actions>
          </Section>

          <Section title="deployment configs">
            <table>
              <thead><tr><th>type</th><th>name</th><th>path</th><th>prio</th><th>preview</th><th></th></tr></thead>
              <tbody>
                {configs.map((d) => (
                  <tr key={d.id}>
                    <td className="mono">{d.config_type}</td>
                    <td>{d.name}</td>
                    <td className="mono muted">{d.path}</td>
                    <td className="num">{d.priority}</td>
                    <td className="muted">{(d.content_preview ?? "").slice(0, 60) || "none"}</td>
                    <td><Btn className="bad" onClick={() =>
                      run(() => api.categoryConfigDelete(sel, d.id), "config deleted")
                    }><Trash2 /></Btn></td>
                  </tr>
                ))}
              </tbody>
            </table>
            <Field label="type"><input value={newCfg.config_type} onChange={(e) => setNewCfg({ ...newCfg, config_type: e.target.value })} /></Field>
            <Field label="name"><input value={newCfg.name} onChange={(e) => setNewCfg({ ...newCfg, name: e.target.value })} /></Field>
            <Field label="path"><input value={newCfg.path} onChange={(e) => setNewCfg({ ...newCfg, path: e.target.value })} /></Field>
            <Field label="content"><textarea value={newCfg.content} onChange={(e) => setNewCfg({ ...newCfg, content: e.target.value })} /></Field>
            <Field label="priority"><input type="number" value={newCfg.priority} onChange={(e) => setNewCfg({ ...newCfg, priority: +e.target.value })} /></Field>
            <Actions><Btn disabled={busy} onClick={() => run(() =>
              api.categoryConfigCreate(sel, { ...newCfg, content: newCfg.content || undefined }),
              "config stored"
            )}><Plus /> add config</Btn></Actions>
          </Section>
        </>
      ) : null}

      <Msg m={m} />
    </Page>
  );
}
