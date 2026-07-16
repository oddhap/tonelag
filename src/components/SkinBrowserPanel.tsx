import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { SkinCatalogPage } from "../bindings/contracts";
import { browseSkinCatalog, installCatalogSkin, isTauri, onSkinBrowserOpened, setPanelVisible } from "../lib/backend";
import { chooseAndImportSkin } from "../lib/skin-store";

const PAGE_SIZE = 24;

export function SkinBrowserPanel() {
  const { t } = useTranslation();
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [page, setPage] = useState<SkinCatalogPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<string | null>(null);
  const [active, setActive] = useState(!isTauri());

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onSkinBrowserOpened(() => setActive(true)).then((dispose) => {
      if (disposed) dispose(); else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!active) return;
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
  }, [active, query, offset]);

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
      setInstalled(md5);
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
    <section className="skin-browser">
      <header className="skin-browser-header">
        <div>
          <h1>{t("skinBrowser")}</h1>
          <p>{t("skinCatalogAttribution")}</p>
        </div>
        <div className="skin-browser-actions">
          <button type="button" onClick={() => void chooseAndImportSkin().catch((reason: unknown) => setError(String(reason)))}>{t("importSkin")}</button>
          <button type="button" onClick={() => void setPanelVisible("skins", false)}>{t("close")}</button>
        </div>
      </header>

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
                  disabled={installing !== null || installed === skin.md5}
                  onClick={() => void install(skin.md5, skin.name)}
                >
                  {installing === skin.md5 ? t("installing") : installed === skin.md5 ? t("installed") : t("install")}
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
    </section>
  );
}
