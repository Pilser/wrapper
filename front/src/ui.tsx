import React from "react";
import { ApiError } from "./api";

export function Section(props: {
  title: string;
  note?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="section">
      <div className="hd">
        <span>{props.title}</span>
        {props.note ? <span className="note">{props.note}</span> : null}
      </div>
      <div className="bd">{props.children}</div>
    </div>
  );
}

export function Page(props: { title: string; sub?: string; children: React.ReactNode }) {
  return (
    <div className="page">
      <h1>
        {props.title}{" "}
        {props.sub ? <span className="sub">{props.sub}</span> : null}
      </h1>
      {props.children}
    </div>
  );
}

export function Field(props: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="field">
      <label>{props.label}</label>
      {props.children}
    </div>
  );
}

export function Actions(props: { children: React.ReactNode }) {
  return <div className="actions">{props.children}</div>;
}

export function Btn(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button {...props} className={"btn " + (props.className ?? "")} />;
}

export function Msg(props: { m: { t: "ok" | "bad" | "info"; s: string } | null }) {
  if (!props.m) return null;
  return (
    <div className={"msgline " + (props.m.t === "info" ? "" : props.m.t)}>
      {props.m.s}
    </div>
  );
}

export function errMsg(e: unknown): string {
  if (e instanceof ApiError) return `${e.message}${e.data && typeof e.data === "object" && e.data.detail ? ` (${e.data.detail})` : ""}`;
  if (e instanceof Error) return e.message;
  return String(e);
}

export function RowLine(props: {
  items: [string, React.ReactNode][];
}) {
  return (
    <div>
      {props.items.map(([k, v]) => (
        <div className="rowline" key={k}>
          <b>{k}</b>
          <span>{v}</span>
        </div>
      ))}
    </div>
  );
}

export function useNow(intervalMs = 5000): number {
  const [n, setN] = React.useState(Date.now());
  React.useEffect(() => {
    const t = setInterval(() => setN(Date.now()), intervalMs);
    return () => clearInterval(t);
  }, [intervalMs]);
  return n;
}
