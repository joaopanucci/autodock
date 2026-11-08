import { Badge } from "./ui";
import { useI18n } from "../i18n";
import type { JavaMap } from "../types";

/** What AutoDock found in a Java folder, whatever its layout (no pom.xml needed). */
export function JavaMapPanel({ map }: { map: JavaMap }) {
  const { t } = useI18n();
  const shown = (dir: string) => (dir === "" ? "." : dir);
  return (
    <div className="java-map" data-testid="java-map">
      <p className="section-description">{t("java.map.body")}</p>

      <div className="expo-project-title">
        <strong>{map.name}</strong>
        <Badge tone={map.layout === "maven" ? "green" : "amber"}>{t(`java.map.layout.${map.layout}`)}</Badge>
        {map.javaLevel && (
          <Badge tone="neutral">
            {t("java.project.java", { level: map.javaLevel.declared })}
          </Badge>
        )}
        <Badge tone="neutral">{t("java.map.counts", { java: map.counts.java, jars: map.counts.jars })}</Badge>
      </div>
      {map.javaLevel && (
        <small className="field-hint">{t("java.map.levelFrom", { file: map.javaLevel.source })}</small>
      )}

      {map.buildSystems.length > 0 && (
        <>
          <p className="field-label">{t("java.map.builds")}</p>
          <ul className="expo-notes">
            {map.buildSystems.map((system) => (
              <li key={system.kind}>
                <strong>{t(`java.map.build.${system.kind}`)}</strong> · <code>{system.files.join(", ")}</code>
              </li>
            ))}
          </ul>
        </>
      )}

      {map.sourceRoots.length > 0 && (
        <>
          <p className="field-label">{t("java.map.roots")}</p>
          <div className="java-map-scroll">
            <table className="java-map-table">
              <thead>
                <tr>
                  <th>{t("java.map.th.folder")}</th>
                  <th>{t("java.map.th.kind")}</th>
                  <th>{t("java.map.th.files")}</th>
                  <th>{t("java.map.th.package")}</th>
                </tr>
              </thead>
              <tbody>
                {map.sourceRoots.map((root) => (
                  <tr key={root.dir}>
                    <td>
                      <code>{shown(root.dir)}</code>
                    </td>
                    <td>{t(`java.map.kind.${root.kind}`)}</td>
                    <td>{root.files}</td>
                    <td>{root.topPackage ? <code>{root.topPackage}</code> : "–"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {map.mainClasses.length > 0 && (
        <>
          <p className="field-label">{t("java.map.mains")}</p>
          <ul className="expo-notes">
            {map.mainClasses.slice(0, 10).map((main) => (
              <li key={main.file}>
                <code>{main.class}</code>
              </li>
            ))}
          </ul>
        </>
      )}

      {map.webApps.length > 0 && (
        <>
          <p className="field-label">{t("java.map.web")}</p>
          <ul className="expo-notes">
            {map.webApps.map((app) => (
              <li key={app}>
                <code>{shown(app)}</code>
              </li>
            ))}
          </ul>
        </>
      )}

      {map.libraryCount > 0 && (
        <details className="java-map-details">
          <summary>{t("java.map.libraries", { count: map.libraryCount })}</summary>
          <ul className="expo-notes">
            {map.libraries.map((library) => (
              <li key={library.path}>
                <code>{library.path}</code>
                {library.version ? ` · ${library.artifact} ${library.version}` : ""}
              </li>
            ))}
          </ul>
          {map.libraryCount > map.libraries.length && (
            <small className="field-hint">{t("java.map.more", { count: map.libraryCount - map.libraries.length })}</small>
          )}
        </details>
      )}

      {map.scatteredCount > 0 && (
        <details className="java-map-details">
          <summary>{t("java.map.scattered", { count: map.scatteredCount })}</summary>
          <ul className="expo-notes">
            {map.scatteredFiles.map((file) => (
              <li key={file}>
                <code>{file}</code>
              </li>
            ))}
          </ul>
        </details>
      )}

      {map.notes.length > 0 && (
        <>
          <p className="field-label">{t("java.map.notes")}</p>
          <ul className="expo-notes">
            {map.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
