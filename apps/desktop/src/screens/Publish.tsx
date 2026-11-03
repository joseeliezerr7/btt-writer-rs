// Revisar y publicar: validación de chunks, colaboradores y subida/exportación.
import { useEffect, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { api, type ChunkValidation, type ProjectDetail } from "../api";
import { useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, Spinner, Tabs, useFeedback } from "../components/ui";
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
        <h1>{t("publish.title")}</h1>
        <span className="muted">{detail ? `${detail.manifest.project.name || detail.manifest.project.id.toUpperCase()} · ${detail.manifest.target_language.name}` : ""}</span>
      </div>
      <div className="content">
        <div className="container" style={{ maxWidth: 860 }}>
          <Tabs value={tab} onChange={setTab} tabs={[{ id: "project", label: t("publish.project") }, { id: "contributors", label: t("publish.contributors") }, { id: "upload", label: t("publish.upload") }]} />
          <div style={{ marginTop: "1rem" }}>
            {!detail || !validation ? (
              <Spinner />
            ) : tab === "project" ? (
              <div className="col">
                <div className="card pad">
                  <div className="row wrap" style={{ gap: "1rem" }}>
                    <span className="badge ok">{t("publish.valid")}: {valid.length}</span>
                    <span className="badge warn">{t("publish.invalid")}: {invalid.length}</span>
                    <span className="badge">{t("issue.empty")} {empty.length}</span>
                  </div>
                </div>
                {invalid.length === 0 ? (
                  <p className="muted">{t("publish.all_good")}</p>
                ) : (
                  <div className="card list">
                    {invalid.map((v) => (
                      <div key={`${v.chapter}-${v.chunk}`} className="list-item" onClick={() => navigate({ name: "translate", id, chapter: v.chapter, chunk: v.chunk, mode: "review" })}>
                        <div className="grow">
                          <strong>{t("translate.chapter")} {v.chapter.replace(/^0+/, "")}:{v.chunk.replace(/^0+/, "")}</strong>
                          <div className="small muted">{v.issues.map((i) => issueText(t, i)).join(" ")}</div>
                        </div>
                        <Button size="sm">{t("publish.review")}</Button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ) : tab === "contributors" ? (
              <div className="card pad">
                <p className="muted small">{t("profile.name_public")}</p>
                <ContributorsEditor id={id} translators={detail.manifest.translators} onChange={load} />
                <div className="row" style={{ justifyContent: "flex-end", marginTop: "1rem" }}>
                  <Button variant="primary" onClick={() => setTab("upload")}>{t("common.continue")}</Button>
                </div>
              </div>
            ) : (
              <div className="card pad col">
                <p>{t("export.title")}</p>
                <Button variant="primary" onClick={() => setShowExport(true)}>{t("details.backup")}</Button>
              </div>
            )}
          </div>
        </div>
      </div>
      {showExport && detail && <ExportDialog project={detail.item} onClose={() => setShowExport(false)} onReview={(files) => navigate({ name: "translate", id, mode: "review", chapter: files[0]?.split("/")[0], chunk: files[0]?.split("/")[1]?.replace(".txt", "") })} />}
    </div>
  );
}
