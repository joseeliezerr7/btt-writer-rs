// Revisar y publicar: validación de chunks, colaboradores y subida/exportación.
import { useEffect, useState } from "react";
import { AlertTriangle, ArrowLeft, CheckCircle2, CircleDashed, UploadCloud, Users } from "lucide-react";
import { api, type ChunkValidation, type ProjectDetail } from "../api";
import { useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, EmptyState, IconBadge, Spinner, Stat, Tabs, useFeedback } from "../components/ui";
import { ContributorsEditor, ExportDialog, issueText } from "../components/dialogs";

export default function PublishScreen({ id }: { id: string }) {
  const { t } = useT();
  const { navigate } = useApp();
  const fb = useFeedback();
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [validation, setValidation] = useState<ChunkValidation[] | null>(null);
  const [tab, setTab] = useState<"project" | "contributors" | "upload">("project");
  const [showExport, setShowExport] = useState(false);

  const load = async () => {
    try {
      const [d, v] = await Promise.all([api.getProject(id), api.validateProject(id)]);
      setDetail(d);
      setValidation(v);
    } catch (e) {
      fb.error(e);
    }
  };
  useEffect(() => {
    load();
  }, [id]);

  const invalid = (validation ?? []).filter((v) => v.issues.length > 0 && !(v.issues.length === 1 && v.issues[0].kind === "empty"));
  const empty = (validation ?? []).filter((v) => v.issues.length === 1 && v.issues[0].kind === "empty");
  const valid = (validation ?? []).filter((v) => v.issues.length === 0);

  return (
    <div className="screen">
      <div className="topbar">
        <Button variant="ghost" icon onClick={() => navigate({ name: "translate", id })} title={t("common.back")}>
          <ArrowLeft size={18} />
        </Button>
        <div>
          <h1>{t("publish.title")}</h1>
          {detail && <div className="small muted">{detail.manifest.project.name || detail.manifest.project.id.toUpperCase()} · {detail.manifest.target_language.name}</div>}
        </div>
      </div>
      <div className="content">
        <div className="container col" style={{ maxWidth: 880, gap: "1.2rem" }}>
          <Tabs segmented value={tab} onChange={setTab} tabs={[{ id: "project", label: t("publish.project") }, { id: "contributors", label: t("publish.contributors") }, { id: "upload", label: t("publish.upload") }]} />
          {!detail || !validation ? (
            <Spinner />
          ) : tab === "project" ? (
            <>
              <div className="stats">
                <Stat icon={<CheckCircle2 size={20} />} tone="success" value={valid.length} label={t("publish.valid")} />
                <Stat icon={<AlertTriangle size={20} />} tone={invalid.length ? "warning" : "neutral"} value={invalid.length} label={t("publish.invalid")} />
                <Stat icon={<CircleDashed size={20} />} tone="neutral" value={empty.length} label={t("issue.empty")} />
              </div>
              {invalid.length === 0 ? (
                <div className="card">
                  <EmptyState tone="success" icon={<CheckCircle2 size={28} />} title={t("publish.all_good")}>
                    <Button variant="primary" onClick={() => setTab("contributors")}>{t("common.continue")}</Button>
                  </EmptyState>
                </div>
              ) : (
                <div className="card list">
                  <div className="card-title"><AlertTriangle size={16} style={{ color: "var(--warning)" }} /> {t("publish.pending")} <span className="badge warn">{invalid.length}</span></div>
                  {invalid.map((v) => (
                    <div key={`${v.chapter}-${v.chunk}`} className="list-item" onClick={() => navigate({ name: "translate", id, chapter: v.chapter, chunk: v.chunk, mode: "review" })}>
                      <IconBadge tone="warning"><AlertTriangle size={16} /></IconBadge>
                      <div className="grow">
                        <strong>
                          {v.chapter === "front"
                            ? t("translate.book_title")
                            : v.chunk === "title" || v.chunk === "reference"
                              ? `${t("translate.chapter")} ${v.chapter.replace(/^0+/, "")} · ${t(v.chunk === "title" ? "translate.chapter_title" : "translate.chapter_reference")}`
                              : `${t("translate.chapter")} ${v.chapter.replace(/^0+/, "")}:${v.chunk.replace(/^0+/, "")}`}
                        </strong>
                        <div className="small muted">{v.issues.map((i) => issueText(t, i)).join(" ")}</div>
                      </div>
                      <Button size="sm">{t("publish.review")}</Button>
                    </div>
                  ))}
                </div>
              )}
            </>
          ) : tab === "contributors" ? (
            <div className="card pad">
              <div className="row" style={{ gap: ".8rem", marginBottom: ".8rem" }}>
                <IconBadge><Users size={18} /></IconBadge>
                <div>
                  <h3 style={{ margin: 0 }}>{t("publish.contributors")}</h3>
                  <div className="small muted">{t("profile.name_public")}</div>
                </div>
              </div>
              <ContributorsEditor id={id} translators={detail.manifest.translators} onChange={load} />
              <div className="row" style={{ justifyContent: "flex-end", marginTop: "1rem" }}>
                <Button variant="primary" onClick={() => setTab("upload")}>{t("common.continue")}</Button>
              </div>
            </div>
          ) : (
            <div className="card">
              <EmptyState icon={<UploadCloud size={28} />} title={t("export.title")} description={t("export.upload_desc")}>
                <Button variant="primary" onClick={() => setShowExport(true)}><UploadCloud size={16} /> {t("details.backup")}</Button>
              </EmptyState>
            </div>
          )}
        </div>
      </div>
      {showExport && detail && <ExportDialog project={detail.item} onClose={() => setShowExport(false)} onReview={(files) => navigate({ name: "translate", id, mode: "review", chapter: files[0]?.split("/")[0], chunk: files[0]?.split("/")[1]?.replace(".txt", "") })} />}
    </div>
  );
}
