import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { SkinCatalogPage, SkinDescriptor } from "../bindings/contracts";
import { browseSkinCatalog, installCatalogSkin, isTauri, listInstalledSkins, onSkinBrowserOpened, selectInstalledSkin, setPanelVisible } from "../lib/backend";
import { chooseAndImportSkin } from "../lib/skin-store";
import { acceptSnapshot, useAppSnapshot } from "../lib/store";
import { PanelChrome } from "./PanelChrome";

const PAGE_SIZE = 24;

export function SkinBrowserPanel() {
  const { t } = useTranslation();
  const snapshot = useAppSnapshot();
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [page, setPage] = useState<SkinCatalogPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installedMd5, setInstalledMd5] = useState<string | null>(null);
  const [localSkins, setLocalSkins] = useState<SkinDescriptor[]>([]);
  const refreshLocalSkins = useCallback(async () => {
    if (!isTauri()) return;
    setLocalSkins(await listInstalledSkins());
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onSkinBrowserOpened(() => {
      void refreshLocalSkins().catch((reason: unknown) => setError(String(reason)));
    }).then((dispose) => {
      if (disposed) dispose(); else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshLocalSkins]);

  useEffect(() => {
    let disposed = false;
    void browseSkinCatalog(query || null, offset, PAGE_SIZE)
      .then((result) => {
        if (!disposed) setPage(result);
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          setPage(null);
          setError(String(reason));
        }
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [query, offset]);

  useEffect(() => {
    if (!isTauri()) return;
    void listInstalledSkins()
      .then(setLocalSkins)
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

  const beginBrowse = (nextQuery: string, nextOffset: number) => {
    if (nextQuery === query && nextOffset === offset) return;
    setLoading(true);
    setError(null);
    setPage(null);
    setQuery(nextQuery);
    setOffset(nextOffset);
  };

  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    beginBrowse(draftQuery.trim(), 0);
  };

  const install = async (md5: string, name: string) => {
    setInstalling(md5);
    setError(null);
    try {
      await installCatalogSkin(md5, name);
      setInstalledMd5(md5);
      await refreshLocalSkins();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setInstalling(null);
    }
  };

  const items = page?.items ?? [];
  const total = page?.totalCount === null || page?.totalCount === undefined
    ? ""
    : t("skinTotal", { count: page.totalCount });

  return (
    <PanelChrome
      className="skin-browser"
      title={t("skinBrowser")}
      controls={<button className="micro-button" type="button" aria-label={t("close")} onClick={() => void setPanelVisible("skins", false)}>×</button>}
    >
      <div className="skin-browser-toolbar">
        <span>{t("installedSkins")}</span>
        <button
          type="button"
          className={snapshot.settings.selectedSkin === null ? "active" : ""}
          onClick={() => void selectInstalledSkin(null).then(acceptSnapshot).catch((reason: unknown) => setError(String(reason)))}
        >Model 275</button>
        {localSkins.map((skin) => (
          <button
            type="button"
            className={snapshot.settings.selectedSkin === skin.id ? "active" : ""}
            title={skin.name}
            key={skin.id}
            onClick={() => void selectInstalledSkin(skin.id).then(acceptSnapshot).catch((reason: unknown) => setError(String(reason)))}
          >{skin.name}</button>
        ))}
        <button type="button" onClick={() => void chooseAndImportSkin().then(refreshLocalSkins).catch((reason: unknown) => setError(String(reason)))}>{t("importSkin")}</button>
      </div>

      <form className="skin-search" onSubmit={submitSearch}>
        <label htmlFor="skin-query">{t("skinSearch")}</label>
        <input
          id="skin-query"
          value={draftQuery}
          maxLength={100}
          onChange={(event) => setDraftQuery(event.currentTarget.value)}
        />
        <button type="submit">{t("search")}</button>
        {query && <button type="button" onClick={() => { setDraftQuery(""); beginBrowse("", 0); }}>{t("clearSearch")}</button>}
      </form>

      <div className="skin-browser-content">
        {error && <div className="skin-browser-error" role="alert">{error}</div>}
        {loading ? (
          <div className="skin-browser-status" aria-live="polite">{t("loading")}</div>
        ) : items.length === 0 ? (
          <div className="skin-browser-status">{t("noSkins")}</div>
        ) : (
          <div className="skin-grid">
            {items.map((skin) => (
              <article className="skin-card" key={skin.md5}>
                <div className="skin-preview">
                  <img src={skin.screenshotUrl} alt="" loading="lazy" decoding="async" />
                </div>
                <h2 title={skin.name}>{skin.name}</h2>
                <button
                  type="button"
                  disabled={installing !== null || installedMd5 === skin.md5}
                  onClick={() => void install(skin.md5, skin.name)}
                >
                  {installing === skin.md5 ? t("installing") : installedMd5 === skin.md5 ? t("installed") : t("install")}
                </button>
              </article>
            ))}
          </div>
        )}
      </div>

      <footer className="skin-browser-footer">
        <span>{items.length > 0 ? t("skinResults", { start: offset + 1, end: offset + items.length, total }) : t("skinLicenseNotice")}</span>
        <nav aria-label={t("skinBrowser")}>
          <button type="button" disabled={loading || offset === 0} onClick={() => beginBrowse(query, Math.max(0, offset - PAGE_SIZE))}>{t("previousPage")}</button>
          <button type="button" disabled={loading || !page?.hasMore} onClick={() => beginBrowse(query, offset + PAGE_SIZE)}>{t("nextPage")}</button>
        </nav>
      </footer>
    </PanelChrome>
  );
}
