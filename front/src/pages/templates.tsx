import React from "react";
import { api } from "../api";
import { Plus, Save, Trash2 } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

export function AdminTemplates() {
  const [rows, setRows] = React.useState<any[]>([]);
  const [sel, setSel] = React.useState<any | null>(null);
  const [edit, setEdit] = React.useState<any>({});
  const [n, setN] = React.useState({
    name: "", kernel_path: "", initrd_path: "", vcpu_count: 2,
    memory_mb: 2048, description: "", rootfs_path: "", requires_gpu: false,
  });
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      setRows((await api.templatesList()).data.templates ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);
  React.useEffect(() => { load(); }, [load]);

  async function open(id: string) {
    setM(null);
    try {
      const t = (await api.templateGet(id)).data.template;
      setSel(t);
      setEdit({
        name: t.name ?? "", description: t.description ?? "",
        kernel_path: t.kernel_path ?? "", initrd_path: t.initrd_path ?? "",
        rootfs_path: t.rootfs_path ?? "", vcpu_count: t.vcpu_count ?? 2,
        memory_mb: t.memory_mb ?? 2048,
        requires_gpu: !!t.requires_gpu, status: t.status ?? "active",
      });
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function run(fn: () => Promise<any>, ok: string) {
    setB(true); setM(null);
    try { await fn(); setM({ t: "ok", s: ok }); await load(); if (sel) await open(sel.id); }
    catch (e) { setM({ t: "bad", s: errMsg(e) }); }
    finally { setB(false); }
  }

  return (
    <Page title="VM TEMPLATES">
      <Section title="all templates" note={`${rows.length} rows`}>
        <table>
          <thead>
            <tr><th>name</th><th>vcpu</th><th>mem</th><th>gpu</th><th>initrd</th><th>status</th><th></th></tr>
          </thead>
          <tbody>
            {rows.map((t) => (
              <tr key={t.id}>
                <td className="mono">{t.name}</td>
                <td className="num">{t.vcpu_count}</td>
                <td className="num">{t.memory_mb} MB</td>
                <td>{t.requires_gpu ? "yes" : "none"}</td>
                <td className="muted mono">{t.initrd_path || t.rootfs_path || "none"}</td>
                <td className={t.status === "active" ? "ok" : "muted"}>{t.status}</td>
                <td><Btn onClick={() => open(t.id)}>edit</Btn></td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Section title="create template">
        <Field label="name"><input value={n.name} onChange={(e) => setN({ ...n, name: e.target.value })} /></Field>
        <Field label="kernel path"><input value={n.kernel_path} onChange={(e) => setN({ ...n, kernel_path: e.target.value })} /></Field>
        <Field label="initrd path"><input value={n.initrd_path} onChange={(e) => setN({ ...n, initrd_path: e.target.value })} /></Field>
        <Field label="rootfs path"><input value={n.rootfs_path} onChange={(e) => setN({ ...n, rootfs_path: e.target.value })} /></Field>
        <Field label="vcpu"><input type="number" value={n.vcpu_count} onChange={(e) => setN({ ...n, vcpu_count: +e.target.value })} /></Field>
        <Field label="memory mb"><input type="number" value={n.memory_mb} onChange={(e) => setN({ ...n, memory_mb: +e.target.value })} /></Field>
        <Field label="description"><input value={n.description} onChange={(e) => setN({ ...n, description: e.target.value })} /></Field>
        <Field label="requires gpu">
          <select value={n.requires_gpu ? "1" : "0"} onChange={(e) => setN({ ...n, requires_gpu: e.target.value === "1" })}>
            <option value="0">no</option><option value="1">yes</option>
          </select>
        </Field>
        <Actions><Btn disabled={busy} onClick={() => {
          run(() => api.templateCreate({
            name: n.name, kernel_path: n.kernel_path, initrd_path: n.initrd_path,
            vcpu_count: n.vcpu_count, memory_mb: n.memory_mb,
            description: n.description || undefined,
            rootfs_path: n.rootfs_path || undefined,
            requires_gpu: n.requires_gpu,
          }), "template created").then(() => setN({
            name: "", kernel_path: "", initrd_path: "", vcpu_count: 2,
            memory_mb: 2048, description: "", rootfs_path: "", requires_gpu: false,
          }));
        }}><Plus /> create</Btn></Actions>
      </Section>

      {sel ? (
        <Section title={`template: ${sel.name}`}>
          <Field label="name"><input value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} /></Field>
          <Field label="description"><input value={edit.description} onChange={(e) => setEdit({ ...edit, description: e.target.value })} /></Field>
          <Field label="kernel path"><input value={edit.kernel_path} onChange={(e) => setEdit({ ...edit, kernel_path: e.target.value })} /></Field>
          <Field label="initrd path"><input value={edit.initrd_path} onChange={(e) => setEdit({ ...edit, initrd_path: e.target.value })} /></Field>
          <Field label="rootfs path"><input value={edit.rootfs_path} onChange={(e) => setEdit({ ...edit, rootfs_path: e.target.value })} /></Field>
          <Field label="vcpu"><input type="number" value={edit.vcpu_count} onChange={(e) => setEdit({ ...edit, vcpu_count: +e.target.value })} /></Field>
          <Field label="memory mb"><input type="number" value={edit.memory_mb} onChange={(e) => setEdit({ ...edit, memory_mb: +e.target.value })} /></Field>
          <Field label="gpu">
            <select value={edit.requires_gpu ? "1" : "0"} onChange={(e) => setEdit({ ...edit, requires_gpu: e.target.value === "1" })}>
              <option value="0">no</option><option value="1">yes</option>
            </select>
          </Field>
          <Field label="status">
            <select value={edit.status} onChange={(e) => setEdit({ ...edit, status: e.target.value })}>
              <option value="active">active</option><option value="retired">retired</option>
            </select>
          </Field>
          <Actions>
            <Btn disabled={busy} onClick={() => run(() => api.templateUpdate(sel.id, edit), "saved")}><Save /> save</Btn>
            <Btn disabled={busy} className="bad" onClick={() => {
              if (confirm(`delete template ${sel.name}?`))
                run(() => api.templateDelete(sel.id), "deleted").then(() => setSel(null));
            }}><Trash2 /> delete</Btn>
          </Actions>
        </Section>
      ) : null}

      <Msg m={m} />
    </Page>
  );
}
